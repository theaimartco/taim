mod gradient;
mod liquid;
mod solid;

use gradient::Gradient;
pub use liquid::Liquid;
use solid::Solid;

use crate::core::material::SceneLight;
use crate::core::{Background, Rectangle, Transformation};
use crate::graphics;
use crate::graphics::color;

use bytemuck::{Pod, Zeroable};

use std::mem;

const INITIAL_INSTANCES: usize = 2_000;

/// The properties of a quad.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct Quad {
    /// The position of the [`Quad`].
    pub position: [f32; 2],

    /// The size of the [`Quad`].
    pub size: [f32; 2],

    /// The border color of the [`Quad`], in __linear RGB__.
    pub border_color: color::Packed,

    /// The border radii of the [`Quad`].
    pub border_radius: [f32; 4],

    /// The border width of the [`Quad`].
    pub border_width: f32,

    /// The shadow color of the [`Quad`].
    pub shadow_color: color::Packed,

    /// The shadow offset of the [`Quad`].
    pub shadow_offset: [f32; 2],

    /// The shadow blur radius of the [`Quad`].
    pub shadow_blur_radius: f32,

    /// Whether the [`Quad`] should be snapped to the pixel grid.
    pub snap: u32,

    /// Material parameters, packed as
    /// `(roughness, thickness_mm, ior, dispersion)`.
    ///
    /// Packed rather than a struct because this is an instance attribute and
    /// a `vec4<f32>` is one vertex location. `transmission` is a fifth
    /// scalar rather than a member here for the same reason: a `vec4` is one
    /// location, and a fifth component would need a second.
    pub material: [f32; 4],

    /// How much light the quad transmits, `0.0..=1.0`.
    ///
    /// `0.0` -- every quad that has not asked for a material -- is opaque
    /// and never samples a backdrop. Above zero the fragment stage refracts
    /// what is behind the quad through it, which is the part a blurred
    /// rectangle cannot do.
    pub transmission: f32,
}

#[derive(Debug, Clone)]
pub struct Pipeline {
    solid: solid::Pipeline,
    gradient: gradient::Pipeline,
    liquid: liquid::Pipeline,
    constant_layout: wgpu::BindGroupLayout,
    backdrop_layout: wgpu::BindGroupLayout,
    backdrop_sampler: wgpu::Sampler,
    /// Bound at group 1 by every quad draw that does *not* transmit.
    ///
    /// A pipeline's bind-group layout set is fixed when it is created, so a
    /// transmissive quad and an opaque one share it and something has to
    /// occupy the slot either way. This is a 1x1 transparent texture that no
    /// opaque fragment ever samples -- the shader's `transmission <= 0.0`
    /// early-out returns before any texture read.
    empty_backdrop: wgpu::BindGroup,
}

#[derive(Default)]
pub struct State {
    layers: Vec<Layer>,
    prepare_layer: usize,
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn prepare(
        &mut self,
        pipeline: &Pipeline,
        device: &wgpu::Device,
        belt: &mut wgpu::util::StagingBelt,
        encoder: &mut wgpu::CommandEncoder,
        quads: &Batch,
        transformation: Transformation,
        scale: f32,
        light: SceneLight,
    ) {
        if self.layers.len() <= self.prepare_layer {
            self.layers
                .push(Layer::new(device, &pipeline.constant_layout));
        }

        let layer = &mut self.layers[self.prepare_layer];
        layer.prepare(device, encoder, belt, quads, transformation, scale, light);

        self.prepare_layer += 1;
    }

    /// Draws `quads`, sampling `backdrop` for any that transmit.
    ///
    /// `backdrop` is `None` when nothing in this batch transmits, which is
    /// nearly every batch; the pipeline's own empty texture fills the slot,
    /// and no fragment ever samples it.
    pub fn render<'a>(
        &'a self,
        pipeline: &'a Pipeline,
        layer: usize,
        bounds: Rectangle<u32>,
        quads: &Batch,
        backdrop: Option<&'a wgpu::BindGroup>,
        render_pass: &mut wgpu::RenderPass<'a>,
        metrics: &crate::metrics::Counters,
    ) {
        if let Some(layer) = self.layers.get(layer) {
            render_pass.set_scissor_rect(bounds.x, bounds.y, bounds.width, bounds.height);

            let backdrop = backdrop.unwrap_or(&pipeline.empty_backdrop);

            let mut solid_offset = 0;
            let mut gradient_offset = 0;
            let mut liquid_offset = 0;
            let mut previous_kind = None;

            for (kind, count) in &quads.order {
                if previous_kind != Some(*kind) {
                    metrics.increment(crate::metrics::Metric::PipelineSwitch);
                    previous_kind = Some(*kind);
                }

                metrics.increment(crate::metrics::Metric::Batch);
                metrics.increment(crate::metrics::Metric::Draw);

                match kind {
                    Kind::Solid => {
                        pipeline.solid.render(
                            render_pass,
                            &layer.constants,
                            backdrop,
                            &layer.solid,
                            solid_offset..(solid_offset + count),
                        );

                        solid_offset += count;
                    }
                    Kind::Gradient => {
                        pipeline.gradient.render(
                            render_pass,
                            &layer.constants,
                            backdrop,
                            &layer.gradient,
                            gradient_offset..(gradient_offset + count),
                        );

                        gradient_offset += count;
                    }
                    Kind::Liquid => {
                        pipeline.liquid.render(
                            render_pass,
                            &layer.constants,
                            backdrop,
                            &layer.liquid,
                            liquid_offset..(liquid_offset + count),
                        );

                        liquid_offset += count;
                    }
                }
            }
        }
    }

    pub fn trim(&mut self) {
        self.prepare_layer = 0;
    }
}

impl Pipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Pipeline {
        let constant_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("taim_wgpu::quad uniforms layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                // The fragment stage reads the scene light from here, so this
                // is no longer vertex-only.
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(
                        mem::size_of::<Uniforms>() as wgpu::BufferAddress
                    ),
                },
                count: None,
            }],
        });

        // Group 1: what is behind a transmissive quad. This takes the last
        // slot at the `max_bind_groups: 2` limit this crate requests of every
        // adapter (see `Engine::new` and `window::compositor`), so a further
        // quad-pipeline resource would mean raising that limit -- which costs
        // portability on weaker adapters.
        let backdrop_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("taim_wgpu::quad backdrop layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let backdrop_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("taim_wgpu::quad backdrop sampler"),
            // Clamped, so a refraction offset that reaches past the frame
            // edge repeats the edge pixel rather than wrapping the opposite
            // side of the screen into the glass.
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            // Frosted glass reads the backdrop's blurred levels, blending
            // between them.
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });

        let empty_backdrop = {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("taim_wgpu::quad empty backdrop"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("taim_wgpu::quad empty backdrop bind group"),
                layout: &backdrop_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&backdrop_sampler),
                    },
                ],
            })
        };

        Self {
            solid: solid::Pipeline::new(device, format, &constant_layout, &backdrop_layout),
            gradient: gradient::Pipeline::new(device, format, &constant_layout, &backdrop_layout),
            liquid: liquid::Pipeline::new(device, format, &constant_layout, &backdrop_layout),
            constant_layout,
            backdrop_layout,
            backdrop_sampler,
            empty_backdrop,
        }
    }

    /// Builds a bind group for `view`, to be bound at group 1 by quads that
    /// transmit.
    pub fn backdrop_bind_group(
        &self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("taim_wgpu::quad backdrop bind group"),
            layout: &self.backdrop_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.backdrop_sampler),
                },
            ],
        })
    }
}

#[derive(Debug)]
pub struct Layer {
    constants: wgpu::BindGroup,
    constants_buffer: wgpu::Buffer,
    solid: solid::Layer,
    gradient: gradient::Layer,
    liquid: liquid::Layer,
}

impl Layer {
    pub fn new(device: &wgpu::Device, constant_layout: &wgpu::BindGroupLayout) -> Self {
        let constants_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taim_wgpu::quad uniforms buffer"),
            size: mem::size_of::<Uniforms>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let constants = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("taim_wgpu::quad uniforms bind group"),
            layout: constant_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: constants_buffer.as_entire_binding(),
            }],
        });

        Self {
            constants,
            constants_buffer,
            solid: solid::Layer::new(device),
            gradient: gradient::Layer::new(device),
            liquid: liquid::Layer::new(device),
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        quads: &Batch,
        transformation: Transformation,
        scale: f32,
        light: SceneLight,
    ) {
        self.update(encoder, belt, transformation, scale, light);

        if !quads.solids.is_empty() {
            self.solid.prepare(device, encoder, belt, &quads.solids);
        }

        if !quads.gradients.is_empty() {
            self.gradient
                .prepare(device, encoder, belt, &quads.gradients);
        }

        if !quads.liquids.is_empty() {
            self.liquid.prepare(device, encoder, belt, &quads.liquids);
        }
    }

    pub fn update(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        transformation: Transformation,
        scale: f32,
        light: SceneLight,
    ) {
        let uniforms = Uniforms::new(transformation, scale, light);
        let bytes = bytemuck::bytes_of(&uniforms);

        belt.write_buffer(
            encoder,
            &self.constants_buffer,
            0,
            (bytes.len() as u64).try_into().expect("Sized uniforms"),
        )
        .copy_from_slice(bytes);
    }
}

/// A group of [`Quad`]s rendered together.
#[derive(Default, Debug)]
pub struct Batch {
    /// The solid quads of the [`Layer`].
    solids: Vec<Solid>,

    /// The gradient quads of the [`Layer`].
    gradients: Vec<Gradient>,

    /// The liquid glass quads of the [`Layer`].
    liquids: Vec<Liquid>,

    /// The quad order of the [`Layer`].
    order: Order,

    /// Whether at least one quad samples the backdrop.
    ///
    /// This is maintained while recording so the display path can answer in
    /// O(1) instead of rescanning every solid and gradient instance before
    /// each layer is rendered.
    transmits: bool,
}

/// The quad order of a [`Layer`]; stored as a tuple of the quad type & its count.
type Order = Vec<(Kind, usize)>;

impl Batch {
    /// Returns true if there are no quads of any type in [`Quads`].
    pub fn is_empty(&self) -> bool {
        self.solids.is_empty() && self.gradients.is_empty() && self.liquids.is_empty()
    }

    /// Returns whether any quad in this batch transmits, and therefore needs
    /// what is behind it captured before it draws.
    ///
    /// A backdrop capture costs a full-frame copy, so a batch of ordinary
    /// opaque quads -- which is nearly every batch -- must never trigger one.
    pub fn needs_backdrop(&self) -> bool {
        self.transmits
    }

    /// Returns the heap bytes reserved by this batch.
    pub fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.solids.capacity() * std::mem::size_of::<Solid>()
            + self.gradients.capacity() * std::mem::size_of::<Gradient>()
            + self.liquids.capacity() * std::mem::size_of::<Liquid>()
            + self.order.capacity() * std::mem::size_of::<(Kind, usize)>()
    }

    /// Adds a [`Quad`] with the provided `Background` type to the quad [`Layer`].
    ///
    /// `luminance` scales the light the fill emits; see
    /// [`taim_core::renderer::Quad::luminance`]. Gradients are not scaled:
    /// their stops are packed into a shared representation whose components
    /// are read by the gradient shader, and brightening one is a change to
    /// that shader rather than to this call.
    pub fn add(&mut self, quad: Quad, background: &Background, luminance: f32) {
        self.transmits |= quad.transmission > 0.0;

        let kind = match background {
            Background::Color(color) => {
                self.solids.push(Solid {
                    color: scale_light(color::pack(*color), luminance),
                    quad,
                });

                Kind::Solid
            }
            Background::Gradient(gradient) => {
                self.gradients.push(Gradient {
                    gradient: graphics::gradient::pack(
                        gradient,
                        Rectangle::new(quad.position.into(), quad.size.into()),
                    ),
                    quad,
                });

                Kind::Gradient
            }
        };

        self.push_order(kind);
    }

    /// Adds a liquid glass quad: one that a press lights and magnifies and
    /// that melts into its neighbours.
    pub fn add_liquid(&mut self, liquid: Liquid) {
        self.transmits |= liquid.quad.transmission > 0.0;
        self.liquids.push(liquid);
        self.push_order(Kind::Liquid);
    }

    fn push_order(&mut self, kind: Kind) {
        match self.order.last_mut() {
            Some((last_kind, count)) if kind == *last_kind => {
                *count = count.saturating_add(1);
            }
            _ => {
                self.order.push((kind, 1));
            }
        }
    }

    pub fn clear(&mut self) {
        self.solids.clear();
        self.gradients.clear();
        self.liquids.clear();
        self.order.clear();
        self.transmits = false;
    }

    pub fn append(&mut self, batch: &mut Batch) {
        self.transmits |= batch.transmits;
        batch.transmits = false;

        self.solids.append(&mut batch.solids);
        self.gradients.append(&mut batch.gradients);
        self.liquids.append(&mut batch.liquids);

        // Preserve painter order while coalescing the only run pair that can
        // become adjacent across the batch boundary. Draining keeps the
        // source allocation reusable by the layer pool.
        let mut incoming = batch.order.drain(..);

        if let Some((kind, count)) = incoming.next() {
            match self.order.last_mut() {
                Some((last_kind, last_count)) if *last_kind == kind => {
                    *last_count = last_count.saturating_add(count);
                }
                _ => self.order.push((kind, count)),
            }

            self.order.extend(incoming);
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// The kind of a quad.
enum Kind {
    /// A solid quad
    Solid,
    /// A gradient quad
    Gradient,
    /// A liquid glass quad
    Liquid,
}

fn color_target_state(format: wgpu::TextureFormat) -> [Option<wgpu::ColorTargetState>; 1] {
    [Some(wgpu::ColorTargetState {
        format,
        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
    })]
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
struct Uniforms {
    transform: [f32; 16],
    scale: f32,
    /// Uniforms align to their largest member. `mat4x4<f32>` aligns to 16, so
    /// `scale` is followed by twelve bytes of padding before anything that
    /// also aligns to 16.
    _padding: [f32; 3],
    /// The scene's key light direction, normalised, in `xyz`.
    ///
    /// A `vec4` rather than a `vec3` because WGSL aligns `vec3<f32>` to 16
    /// bytes anyway: declaring three floats and writing three floats produces
    /// a struct the shader reads as 96 bytes and the host writes as 80, which
    /// fails pipeline validation rather than merely reading the wrong values.
    ///
    /// One light for every surface in the interface. A per-application light
    /// produces surfaces that disagree about where the window is, which reads
    /// as inconsistency rather than as lighting.
    light: [f32; 4],
}

impl Uniforms {
    fn new(transformation: Transformation, scale: f32, light: SceneLight) -> Uniforms {
        Self {
            transform: *transformation.as_ref(),
            scale,
            _padding: [0.0; 3],
            light: {
                let [x, y, z] = light.unit_direction();

                [x, y, z, 0.0]
            },
        }
    }
}

impl Default for Uniforms {
    fn default() -> Self {
        Self {
            transform: *Transformation::IDENTITY.as_ref(),
            scale: 1.0,
            _padding: [0.0; 3],
            light: {
                let [x, y, z] = SceneLight::KEY.unit_direction();

                [x, y, z, 0.0]
            },
        }
    }
}

/// Applies a luminance multiplier to a packed color.
///
/// A no-op at `1.0`, which is every quad the framework draws today, and a
/// no-op under `web-colors` where the pack is sRGB-encoded rather than linear
/// and scaling its components would change hue rather than brightness.
fn scale_light(packed: color::Packed, luminance: f32) -> color::Packed {
    if !color::GAMMA_CORRECTION || (luminance - 1.0).abs() < f32::EPSILON {
        return packed;
    }

    packed.scale_light(luminance)
}

#[cfg(test)]
mod batching_tests {
    use super::*;
    use crate::batch::{self, Batchable};
    use crate::core::{Background, Color};

    impl Batchable for Kind {
        type Key = Kind;

        fn batch_key(&self) -> Kind {
            *self
        }
    }

    fn quad() -> Quad {
        Quad {
            position: [0.0, 0.0],
            size: [1.0, 1.0],
            border_color: color::pack(Color::TRANSPARENT),
            border_radius: [0.0; 4],
            border_width: 0.0,
            shadow_color: color::pack(Color::TRANSPARENT),
            shadow_offset: [0.0, 0.0],
            shadow_blur_radius: 0.0,
            snap: 0,
            material: [1.0, 0.0, 1.0, 0.0],
            transmission: 0.0,
        }
    }

    /// A luminance multiplier must reach the packed fill.
    ///
    /// Proved here rather than through the integration suite because that
    /// suite cannot turn `web-colors` off -- the root crate lists itself as
    /// a dev-dependency with default features, and `web-colors` is one of
    /// them -- so it can only ever check that the multiplier is *ignored*.
    /// This crate's own tests build on the linear path, where it applies.
    #[test]
    fn a_luminance_multiplier_scales_the_packed_fill() {
        if !color::GAMMA_CORRECTION {
            eprintln!("skipped: compositing is not linear in this build");
            return;
        }

        let mut plain = Batch::default();
        plain.add(quad(), &Background::Color(Color::WHITE), 1.0);

        let mut emitting = Batch::default();
        emitting.add(quad(), &Background::Color(Color::WHITE), 4.926);

        let ordinary = plain.solids[0].color.components();
        let bright = emitting.solids[0].color.components();

        for channel in 0..3 {
            assert!(
                (bright[channel] / ordinary[channel] - 4.926).abs() < 1e-3,
                "channel {channel}: {} against {}",
                bright[channel],
                ordinary[channel]
            );
        }

        // Alpha is coverage, not light, and must not move.
        assert!((bright[3] - ordinary[3]).abs() < 1e-6);
    }

    /// The default must be bit-identical to having no multiplier at all.
    #[test]
    fn the_default_multiplier_changes_nothing() {
        let mut batch = Batch::default();
        batch.add(
            quad(),
            &Background::Color(Color::from_rgb(0.3, 0.6, 0.9)),
            1.0,
        );

        assert_eq!(
            batch.solids[0].color.components(),
            color::pack(Color::from_rgb(0.3, 0.6, 0.9)).components()
        );
    }

    /// Proves `Batch::add`'s incremental `order` accumulation is
    /// exactly equivalent to running `batch::group` over the same
    /// sequence of `Kind`s after the fact -- the ordering-correctness
    /// oracle `phase.md`'s Phase 5 step 3 asks for, applied to the
    /// batching this crate's quad rendering already depends on being
    /// correct (see `wgpu/src/batch.rs`'s module documentation for why
    /// this crate does not migrate `Batch` itself onto `batch::group`).
    fn assert_incremental_order_matches_grouping(kinds: &[Kind]) {
        let mut batch = Batch::default();

        for kind in kinds {
            let background = match kind {
                Kind::Solid => Background::Color(Color::BLACK),
                Kind::Gradient => Background::Gradient(crate::core::Gradient::Linear(
                    crate::core::gradient::Linear::new(0.0),
                )),
                Kind::Liquid => {
                    batch.add_liquid(liquid());
                    continue;
                }
            };

            batch.add(quad(), &background, 1.0);
        }

        let expected_runs = batch::group(kinds);
        let actual_order: Vec<(Kind, usize)> = batch.order.clone();

        let actual_runs: Vec<batch::Run<Kind>> = {
            let mut start = 0;
            actual_order
                .iter()
                .map(|(kind, count)| {
                    let run = batch::Run {
                        key: *kind,
                        start,
                        end: start + count,
                    };
                    start += count;
                    run
                })
                .collect()
        };

        assert_eq!(
            actual_runs, expected_runs,
            "Batch::add's incremental order for {kinds:?} must match batch::group's result"
        );
    }

    fn liquid() -> Liquid {
        let mut pane = quad();
        pane.transmission = 0.8;
        Liquid::new(
            color::pack(Color::WHITE),
            pane,
            &crate::core::material::Liquid::default(),
            crate::core::Transformation::IDENTITY,
            1.0,
        )
    }

    #[test]
    fn liquid_quads_keep_painter_order_among_the_others() {
        assert_incremental_order_matches_grouping(&[
            Kind::Solid,
            Kind::Liquid,
            Kind::Liquid,
            Kind::Gradient,
            Kind::Liquid,
            Kind::Solid,
        ]);
    }

    #[test]
    fn a_transmissive_liquid_quad_asks_for_a_backdrop() {
        let mut batch = Batch::default();
        batch.add_liquid(liquid());
        assert!(batch.needs_backdrop());
        assert!(!batch.is_empty());
    }

    #[test]
    fn a_liquid_press_and_neighbours_follow_the_transformation() {
        use crate::core::material::{LiquidNeighbour, LiquidTouch};
        use crate::core::{Point, Rectangle, Size, Transformation};

        let shape = crate::core::material::Liquid {
            touch: Some(LiquidTouch {
                position: Point::new(10.0, 20.0),
                strength: 2.0,
                radius: 30.0,
            }),
            neighbours: [
                Some(LiquidNeighbour {
                    bounds: Rectangle::new(Point::new(5.0, 6.0), Size::new(40.0, 20.0)),
                    radius: 10.0,
                }),
                None,
            ],
            blend: 12.0,
            legibility: 0.5,
        };
        let packed = Liquid::new(
            color::pack(Color::WHITE),
            quad(),
            &shape,
            Transformation::translate(100.0, 0.0),
            1.0,
        );

        assert_eq!(
            packed.touch,
            [110.0, 20.0, 1.0, 30.0],
            "strength is clamped"
        );
        assert_eq!(packed.neighbour_a, [105.0, 6.0, 40.0, 20.0]);
        assert_eq!(packed.neighbour_b, [0.0; 4], "no second neighbour");
        assert_eq!(packed.shape, [10.0, 0.0, 12.0, 0.5]);
    }

    #[test]
    fn empty_input_produces_no_runs() {
        assert_incremental_order_matches_grouping(&[]);
    }

    #[test]
    fn all_solid_produces_one_run() {
        assert_incremental_order_matches_grouping(&[Kind::Solid, Kind::Solid, Kind::Solid]);
    }

    #[test]
    fn alternating_kinds_produce_a_run_per_switch_in_order() {
        assert_incremental_order_matches_grouping(&[
            Kind::Solid,
            Kind::Solid,
            Kind::Gradient,
            Kind::Solid,
            Kind::Gradient,
            Kind::Gradient,
            Kind::Gradient,
        ]);
    }

    #[test]
    fn a_single_gradient_among_solids_does_not_merge_with_a_later_solid_run() {
        // The same "non-adjacent same-key items must not merge" property
        // batch::group's own tests pin, exercised through the real
        // Batch::add path this test module's other cases prove agrees
        // with batch::group.
        assert_incremental_order_matches_grouping(&[Kind::Solid, Kind::Gradient, Kind::Solid]);
    }

    #[test]
    fn backdrop_need_is_constant_time_state_and_clear_resets_it() {
        let mut transmitting = quad();
        transmitting.transmission = 0.75;

        let mut batch = Batch::default();
        batch.add(transmitting, &Background::Color(Color::BLACK), 1.0);
        assert!(batch.needs_backdrop());

        batch.clear();
        assert!(!batch.needs_backdrop());
    }

    #[test]
    fn append_merges_the_boundary_run_without_reordering_instances() {
        let mut first = Batch::default();
        first.add(quad(), &Background::Color(Color::BLACK), 1.0);

        let mut second = Batch::default();
        second.add(quad(), &Background::Color(Color::WHITE), 1.0);
        second.add(
            quad(),
            &Background::Gradient(crate::core::Gradient::Linear(
                crate::core::gradient::Linear::new(0.0),
            )),
            1.0,
        );

        first.append(&mut second);

        assert_eq!(first.order, vec![(Kind::Solid, 2), (Kind::Gradient, 1)]);
        assert_eq!(first.solids.len(), 2);
        assert_eq!(first.gradients.len(), 1);
        assert!(second.is_empty());
        assert!(second.order.is_empty());
    }
}

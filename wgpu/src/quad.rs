mod gradient;
mod solid;

use gradient::Gradient;
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
    /// a `vec4<f32>` is one vertex location; `transmission` is absent because
    /// nothing samples a backdrop yet.
    pub material: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct Pipeline {
    solid: solid::Pipeline,
    gradient: gradient::Pipeline,
    constant_layout: wgpu::BindGroupLayout,
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

    pub fn render<'a>(
        &'a self,
        pipeline: &'a Pipeline,
        layer: usize,
        bounds: Rectangle<u32>,
        quads: &Batch,
        render_pass: &mut wgpu::RenderPass<'a>,
        metrics: &crate::metrics::Counters,
    ) {
        if let Some(layer) = self.layers.get(layer) {
            render_pass.set_scissor_rect(bounds.x, bounds.y, bounds.width, bounds.height);

            let mut solid_offset = 0;
            let mut gradient_offset = 0;
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
                            &layer.solid,
                            solid_offset..(solid_offset + count),
                        );

                        solid_offset += count;
                    }
                    Kind::Gradient => {
                        pipeline.gradient.render(
                            render_pass,
                            &layer.constants,
                            &layer.gradient,
                            gradient_offset..(gradient_offset + count),
                        );

                        gradient_offset += count;
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
            label: Some("aura_wgpu::quad uniforms layout"),
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

        Self {
            solid: solid::Pipeline::new(device, format, &constant_layout),
            gradient: gradient::Pipeline::new(device, format, &constant_layout),
            constant_layout,
        }
    }
}

#[derive(Debug)]
pub struct Layer {
    constants: wgpu::BindGroup,
    constants_buffer: wgpu::Buffer,
    solid: solid::Layer,
    gradient: gradient::Layer,
}

impl Layer {
    pub fn new(device: &wgpu::Device, constant_layout: &wgpu::BindGroupLayout) -> Self {
        let constants_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("aura_wgpu::quad uniforms buffer"),
            size: mem::size_of::<Uniforms>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let constants = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("aura_wgpu::quad uniforms bind group"),
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

    /// The quad order of the [`Layer`].
    order: Order,
}

/// The quad order of a [`Layer`]; stored as a tuple of the quad type & its count.
type Order = Vec<(Kind, usize)>;

impl Batch {
    /// Returns true if there are no quads of any type in [`Quads`].
    pub fn is_empty(&self) -> bool {
        self.solids.is_empty() && self.gradients.is_empty()
    }

    /// Adds a [`Quad`] with the provided `Background` type to the quad [`Layer`].
    ///
    /// `luminance` scales the light the fill emits; see
    /// [`aura_core::renderer::Quad::luminance`]. Gradients are not scaled:
    /// their stops are packed into a shared representation whose components
    /// are read by the gradient shader, and brightening one is a change to
    /// that shader rather than to this call.
    pub fn add(&mut self, quad: Quad, background: &Background, luminance: f32) {
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

        match self.order.last_mut() {
            Some((last_kind, count)) if kind == *last_kind => {
                *count += 1;
            }
            _ => {
                self.order.push((kind, 1));
            }
        }
    }

    pub fn clear(&mut self) {
        self.solids.clear();
        self.gradients.clear();
        self.order.clear();
    }

    pub fn append(&mut self, batch: &mut Batch) {
        self.solids.append(&mut batch.solids);
        self.gradients.append(&mut batch.gradients);
        self.order.append(&mut batch.order);
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// The kind of a quad.
enum Kind {
    /// A solid quad
    Solid,
    /// A gradient quad
    Gradient,
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
}

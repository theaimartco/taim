//! The GPU side of the effect graph.
//!
//! # What this replaced
//!
//! An empty struct. `prepare` and `render` were `// Stub` and did nothing, so
//! blur, backdrop blur, colour matrix, saturation and noise -- nine modules
//! of bounds arithmetic, tier gating and pooling, all of it tested -- drew
//! exactly nothing. `blur_layers` and `effect_ladder` were the *cheapest*
//! scenes in the benchmark suite, which is what a cost model with no
//! rasteriser costs (`Capstone.md` F-07).
//!
//! # How an effect is applied
//!
//! Effects are post-process: the layer beneath has already been drawn to the
//! frame when they run. So each effect
//!
//! 1. copies its region of the frame into an offscreen texture,
//! 2. runs one or more fragment passes over that texture, ping-ponging
//!    between two targets when a pass cannot read and write the same one,
//! 3. blits the result back over the region.
//!
//! The region comes from `Effect::bounds`, which already accounts for a
//! blur's sampling overhang and clips to the surface -- so a blur over a card
//! touches that card's pixels and its penumbra, not the whole screen.
//!
//! # Why blur is two passes
//!
//! A 2D Gaussian of radius r costs `(2r+1)^2` taps in one pass and
//! `2*(2r+1)` in two. At r = 16 that is 1089 against 66. The kernel is
//! separable, so the two are equivalent.

use std::borrow::Cow;

use crate::core::Rectangle;
use crate::effects::{Effect, Graph};

/// One effect, resolved to the region it will touch.
#[derive(Debug, Clone, Copy)]
struct Queued {
    effect: Effect,
    /// In physical texels, clipped to the surface.
    region: Rectangle<u32>,
    /// The layer that submitted it.
    ///
    /// A backdrop effect must run at *its own* layer -- before that layer
    /// draws and after every layer beneath it has. Without this, the first
    /// layer with any backdrop effect would run all of them, frosting a page
    /// that had not been drawn yet.
    layer: usize,
}

/// The uniform block shared by every effect pass.
///
/// One layout for all of them: a blur ignores the matrix rows and a colour
/// matrix ignores the radius, which costs 80 bytes per pass and saves a
/// second bind group layout, a second buffer, and the bugs that come with
/// keeping two of anything in step.
#[derive(Debug, Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
struct Uniforms {
    region: [f32; 4],
    step: [f32; 4],
    params: [f32; 4],
    matrix_r: [f32; 4],
    matrix_g: [f32; 4],
    matrix_b: [f32; 4],
    matrix_a: [f32; 4],
    bias: [f32; 4],
}

impl Uniforms {
    /// The stride between two uniform slots in the shared buffer.
    ///
    /// Uniform bindings must start at a device-specific alignment, which is
    /// 256 bytes on most hardware and never less than this struct.
    const STRIDE: u64 = 256;

    /// Uniform slots reserved per queued effect.
    ///
    /// Three, because the passes for one effect must not share a slot: every
    /// `write_buffer` for a frame lands before any pass runs, so two passes
    /// pointed at one slot both read whatever was written last. A blur whose
    /// horizontal and vertical passes shared a slot blurred *vertically
    /// twice* -- the vertical edges stayed hard, and it looked entirely
    /// plausible until someone rendered it and looked.
    const SLOTS_PER_EFFECT: u64 = 3;

    fn new(region: Rectangle<u32>, target: (u32, u32)) -> Self {
        Self {
            region: [
                region.x as f32,
                region.y as f32,
                region.width as f32,
                region.height as f32,
            ],
            step: [
                1.0 / target.0.max(1) as f32,
                1.0 / target.1.max(1) as f32,
                0.0,
                0.0,
            ],
            params: [0.0; 4],
            matrix_r: [1.0, 0.0, 0.0, 0.0],
            matrix_g: [0.0, 1.0, 0.0, 0.0],
            matrix_b: [0.0, 0.0, 1.0, 0.0],
            matrix_a: [0.0, 0.0, 0.0, 1.0],
            bias: [0.0; 4],
        }
    }
}

/// A pair of offscreen targets the passes ping-pong between.
struct Targets {
    width: u32,
    height: u32,
    views: [wgpu::TextureView; 2],
    // Held so the views stay valid.
    _textures: [wgpu::Texture; 2],
}

/// Which fragment entry point a pass uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Blur,
    ColorMatrix,
    Noise,
    Blit,
    Mask,
}

/// The mask atlas.
///
/// One layer per mask rather than a packed 2D atlas: a mask is addressed in
/// its own 0..1 space by [`Kind::Mask`], so packing would mean carrying each
/// slot's rectangle through the uniform block to unpack it again. Layers cost
/// a fixed footprint per mask and keep the shader's addressing trivial.
#[derive(Clone)]
struct Masks {
    texture: wgpu::Texture,
    /// One view per slot; index is the slot handed to the caller.
    views: Vec<wgpu::TextureView>,
    layout: wgpu::BindGroupLayout,
}

impl Masks {
    /// The edge length of every mask layer.
    ///
    /// Fixed, and sampled with linear filtering, so a caller's mask is scaled
    /// to the region it clips rather than the region being constrained to the
    /// mask's resolution.
    const SIZE: u32 = 256;

    /// How many masks the atlas holds.
    ///
    /// A hard ceiling rather than a growing allocation: `phase.md` requires
    /// that no effect can trigger unbounded texture allocation, and a mask
    /// arrives from application code.
    const SLOTS: u32 = 16;

    fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("taim_wgpu.effect.mask_atlas"),
            size: wgpu::Extent3d {
                width: Self::SIZE,
                height: Self::SIZE,
                depth_or_array_layers: Self::SLOTS,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let views = (0..Self::SLOTS)
            .map(|slot| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("taim_wgpu.effect.mask_slot"),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: slot,
                    array_layer_count: Some(1),
                    ..wgpu::TextureViewDescriptor::default()
                })
            })
            .collect();

        // Source *and* mask, because the portability floor allows only two
        // bind groups and group 0 is the uniforms.
        let entry = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("taim_wgpu.effect.mask_layout"),
            entries: &[
                entry,
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    ..entry
                },
            ],
        });

        Self {
            texture,
            views,
            layout,
        }
    }
}

pub struct Pipeline {
    format: wgpu::TextureFormat,
    sampler: wgpu::Sampler,
    uniform_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    blur: wgpu::RenderPipeline,
    color_matrix: wgpu::RenderPipeline,
    noise: wgpu::RenderPipeline,
    blit: wgpu::RenderPipeline,
    mask: wgpu::RenderPipeline,
    /// The mask atlas, and the slots handed out of it.
    masks: Masks,
    uniforms: wgpu::Buffer,
    /// How many uniform slots `uniforms` currently holds.
    capacity: u64,
    targets: Option<Targets>,
    /// Byte accounting for the intermediates in `targets`.
    ///
    /// The budget comes from the active `quality::Tier`, so thermal and
    /// battery pressure reach the renderer as a smaller ceiling here. Before
    /// this existed the pool had no production caller at all and
    /// `Policy::texture_budget_bytes` was read by nothing.
    pool: crate::effects::pool::Pool,
    /// Effects that read what is *behind* a layer, applied before the layer
    /// draws over them.
    backdrop: Vec<Queued>,
    /// Effects that read the layer itself, applied after it draws.
    foreground: Vec<Queued>,
}

impl Clone for Pipeline {
    /// The engine derives `Clone`; a pipeline's GPU objects are all handles.
    ///
    /// The per-frame work queue is deliberately *not* carried over: a clone
    /// is a second renderer, and inheriting another's half-built frame would
    /// apply its effects twice.
    fn clone(&self) -> Self {
        Self {
            format: self.format,
            sampler: self.sampler.clone(),
            uniform_layout: self.uniform_layout.clone(),
            texture_layout: self.texture_layout.clone(),
            blur: self.blur.clone(),
            color_matrix: self.color_matrix.clone(),
            noise: self.noise.clone(),
            blit: self.blit.clone(),
            mask: self.mask.clone(),
            masks: self.masks.clone(),
            // A clone is a second renderer with its own intermediates, so it
            // starts with an empty ledger at the same budget.
            pool: crate::effects::pool::Pool::new(self.pool.budget_bytes()),
            uniforms: self.uniforms.clone(),
            capacity: self.capacity,
            targets: None,
            backdrop: Vec::new(),
            foreground: Vec::new(),
        }
    }
}

impl Pipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("taim_wgpu.effect.sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            // Linear, so a downsampled blur does not band.
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });

        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("taim_wgpu.effect.uniform_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<Uniforms>() as u64
                        ),
                    },
                    count: None,
                },
            ],
        });

        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("taim_wgpu.effect.texture_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("taim_wgpu.effect.pipeline_layout"),
            bind_group_layouts: &[Some(&uniform_layout), Some(&texture_layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("taim_wgpu.effect.shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shader/effect.wgsl"))),
        });

        // `blend` matters most for the write-back. An effect's result
        // *replaces* its region: a mask that erased a pixel, or a blur whose
        // edge went transparent, must not blend over the original and leave
        // it showing through. Intermediate passes clear their target first,
        // so their blend mode never comes into play.
        let build_with = |label: &str, entry: &str, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };

        let build = |label: &str, entry: &str| {
            build_with(
                label,
                entry,
                Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            )
        };

        let masks = Masks::new(device);

        // Its own pipeline layout, carrying the mask atlas as group 2, so no
        // other pass pays for a binding it never reads.
        let mask_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("taim_wgpu.effect.mask_pipeline_layout"),
            bind_group_layouts: &[Some(&uniform_layout), Some(&masks.layout)],
            immediate_size: 0,
        });

        let mask = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("taim_wgpu.effect.mask"),
            layout: Some(&mask_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_mask"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taim_wgpu.effect.uniforms"),
            size: Uniforms::STRIDE * 24,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            format,
            sampler,
            uniform_layout,
            texture_layout,
            blur: build("taim_wgpu.effect.blur", "fs_blur"),
            color_matrix: build("taim_wgpu.effect.color_matrix", "fs_color_matrix"),
            noise: build("taim_wgpu.effect.noise", "fs_noise"),
            // Replace, not blend: see `build_with`.
            blit: build_with("taim_wgpu.effect.blit", "fs_blit", None),
            mask,
            masks,
            uniforms,
            capacity: 24,
            targets: None,
            pool: crate::effects::pool::Pool::new(
                crate::graphics::quality::Tier::default()
                    .policy()
                    .texture_budget_bytes,
            ),
            backdrop: Vec::new(),
            foreground: Vec::new(),
        }
    }

    /// Records the effects in `graph` for this frame.
    ///
    /// `source` is the region the layer occupies and `surface` the whole
    /// frame, both in physical texels; each effect's own bounds are derived
    /// from them so a blur's overhang is included and nothing samples outside
    /// the surface.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        layer: usize,
        graph: &Graph,
        source: Rectangle,
        surface: Rectangle,
        tier: crate::graphics::quality::Tier,
        target: (u32, u32),
    ) {
        // Before anything else, and before any early return: the ledger
        // follows the tier even on a frame that queues no effects, so
        // telemetry never reports a budget the renderer is no longer using.
        self.sync_budget(tier);

        for effect in graph.effects() {
            let Some(bounds) = effect.bounds(source, surface, tier) else {
                continue;
            };

            let region = Rectangle {
                x: bounds.x.floor().max(0.0) as u32,
                y: bounds.y.floor().max(0.0) as u32,
                width: bounds.width.ceil().max(0.0) as u32,
                height: bounds.height.ceil().max(0.0) as u32,
            };

            // A region with no area has nothing to sample and nothing to
            // write; queueing it would cost two passes to produce no pixels.
            if region.width == 0 || region.height == 0 {
                continue;
            }

            let queued = Queued {
                effect: *effect,
                region,
                layer,
            };

            // A backdrop effect reads what is *behind* its layer, so it has
            // to run before that layer draws over it. Everything else reads
            // the layer and runs after. Splitting here rather than at render
            // time keeps the ordering decision in one place.
            if matches!(effect, Effect::BackdropBlur(_)) {
                self.backdrop.push(queued);
            } else {
                self.foreground.push(queued);
            }
        }

        if self.backdrop.is_empty() && self.foreground.is_empty() {
            return;
        }

        // Ask the budget before allocating. Under pressure this returns a
        // smaller size, and when even the floor will not fit it returns
        // `None` and the queued effects are dropped -- which is the point:
        // `Policy::texture_budget_bytes` now decides what the renderer does.
        let Some(granted) = self.admit_targets(tier, target) else {
            log::debug!(
                "effect intermediates for {}x{} do not fit the {} byte budget at {tier:?}; \
                 dropping {} queued effect(s) this frame",
                target.0,
                target.1,
                tier.policy().texture_budget_bytes,
                self.backdrop.len() + self.foreground.len(),
            );

            self.backdrop.clear();
            self.foreground.clear();

            return;
        };

        self.ensure_targets(device, granted);
        self.ensure_uniform_capacity(device);
    }

    /// Points the ledger at the tier's budget, discarding intermediates that
    /// no longer fit.
    ///
    /// Called once per frame by the renderer, unconditionally.
    ///
    /// Called on every `prepare`, including frames that queue no effects: a
    /// device that has just dropped to `Recovery` under thermal pressure
    /// should be reporting `Recovery`'s budget immediately, not on whenever
    /// the next blur happens to appear.
    pub fn sync_budget(&mut self, tier: crate::graphics::quality::Tier) {
        let budget = tier.policy().texture_budget_bytes;

        if self.pool.budget_bytes() == budget {
            return;
        }

        self.pool = crate::effects::pool::Pool::new(budget);

        // The old ledger was only ever tracking the current pair, so dropping
        // both together leaks nothing; they are re-admitted on the next frame
        // that needs them, at whatever size the new budget allows.
        self.targets = None;
    }

    /// Returns the intermediate size the budget allows, at or below
    /// `requested`, or `None` when even the smallest useful pair will not fit.
    ///
    /// Two intermediates are needed, because every effect here ping-pongs
    /// between a pair. Halving both dimensions quarters the cost, so the
    /// search converges in a handful of steps.
    fn admit_targets(
        &mut self,
        tier: crate::graphics::quality::Tier,
        requested: (u32, u32),
    ) -> Option<(u32, u32)> {
        use crate::effects::pool::Descriptor;

        let _ = tier;

        // Release whatever the current pair costs before re-admitting, or a
        // steady-state frame would charge itself again every time.
        if let Some(existing) = &self.targets {
            let descriptor = Descriptor::new(existing.width, existing.height, self.format);

            self.pool.release(descriptor);
            self.pool.release(descriptor);
        }

        let (mut width, mut height) = (requested.0.max(1), requested.1.max(1));

        loop {
            let descriptor = Descriptor::new(width, height, self.format);

            if self.pool.acquire(descriptor) {
                if self.pool.acquire(descriptor) {
                    return Some((width, height));
                }

                // Only one of the pair fitted, which is no use.
                self.pool.release(descriptor);
            }

            // A 64px floor: below this an effect is not a reduced-quality
            // version of itself, it is a different image, and skipping it
            // outright is the more honest degradation.
            if width <= 64 || height <= 64 {
                return None;
            }

            width = (width / 2).max(1);
            height = (height / 2).max(1);
        }
    }

    /// Bytes of effect intermediates currently accounted, and the budget
    /// they are accounted against.
    ///
    /// Telemetry, in the same spirit as the renderer's retained-layer
    /// statistics: it is what makes "the budget changed what was rendered" an
    /// assertion rather than a claim.
    pub fn memory(&self) -> (u64, u64) {
        (self.pool.footprint_bytes(), self.pool.budget_bytes())
    }

    /// The size the intermediates were actually allocated at, if any.
    ///
    /// Smaller than the requested size when the budget forced a downscale.
    pub fn target_size(&self) -> Option<(u32, u32)> {
        self.targets
            .as_ref()
            .map(|targets| (targets.width, targets.height))
    }

    /// Whether `layer` has backdrop effects waiting.
    pub fn has_backdrop(&self, layer: usize) -> bool {
        self.backdrop.iter().any(|queued| queued.layer == layer)
    }

    /// Applies the effects that read what is behind a layer.
    ///
    /// Must be called *before* the layer's own primitives are recorded: a
    /// backdrop blur frosts the page behind a sheet, and running it after the
    /// sheet has drawn would blur the sheet instead.
    #[allow(clippy::too_many_arguments)]
    pub fn render_backdrop(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::Texture,
        target: (u32, u32),
        layer: usize,
    ) {
        let mut mine = Vec::new();

        self.backdrop.retain(|queued| {
            if queued.layer == layer {
                mine.push(*queued);
                false
            } else {
                true
            }
        });

        self.apply(device, queue, encoder, frame, target, &mine);
    }

    /// Applies the effects that read the layer itself, then clears the queue.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::Texture,
        target: (u32, u32),
    ) {
        let queued = std::mem::take(&mut self.foreground);

        self.apply(device, queue, encoder, frame, target, &queued);
    }

    /// Runs `queued` against `frame`.
    #[allow(clippy::too_many_arguments)]
    fn apply(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::Texture,
        target: (u32, u32),
        queued: &[Queued],
    ) {
        if queued.is_empty() {
            return;
        }

        let Some(targets) = &self.targets else {
            return;
        };

        let frame_view = frame.create_view(&wgpu::TextureViewDescriptor::default());

        for (index, item) in queued.iter().enumerate() {
            let base = index as u64 * Uniforms::SLOTS_PER_EFFECT * Uniforms::STRIDE;

            // Copy the region out of the frame. `copy_texture_to_texture`
            // rather than a sampled pass: no filtering, no format juggling,
            // and it is the one operation guaranteed not to read and write
            // the same texture.
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: frame,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: item.region.x,
                        y: item.region.y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &targets._textures[0],
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: item.region.x,
                        y: item.region.y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: item
                        .region
                        .width
                        .min(target.0.saturating_sub(item.region.x)),
                    height: item
                        .region
                        .height
                        .min(target.1.saturating_sub(item.region.y)),
                    depth_or_array_layers: 1,
                },
            );

            let mut uniforms = Uniforms::new(item.region, target);
            let mut source = 0usize;

            match item.effect {
                Effect::Blur(blur) => {
                    uniforms.params[0] = blur.radius;

                    // Horizontal into target 1, vertical back into target 0,
                    // each with its own uniform slot.
                    for (pass, direction) in [(0usize, [1.0, 0.0]), (1, [0.0, 1.0])] {
                        let slot = base + pass as u64 * Uniforms::STRIDE;

                        uniforms.step[2] = direction[0];
                        uniforms.step[3] = direction[1];

                        queue.write_buffer(&self.uniforms, slot, bytemuck::bytes_of(&uniforms));

                        let destination = 1 - source;

                        self.pass(
                            device,
                            encoder,
                            Kind::Blur,
                            &targets.views[source],
                            &targets.views[destination],
                            slot,
                            true,
                            None,
                        );

                        source = destination;
                    }
                }
                Effect::ColorMatrix(matrix) => {
                    write_matrix(&mut uniforms, &matrix.values);
                    queue.write_buffer(&self.uniforms, base, bytemuck::bytes_of(&uniforms));

                    self.pass(
                        device,
                        encoder,
                        Kind::ColorMatrix,
                        &targets.views[0],
                        &targets.views[1],
                        base,
                        true,
                        None,
                    );

                    source = 1;
                }
                Effect::Saturation(saturation) => {
                    write_matrix(&mut uniforms, &saturation_matrix(saturation.0));
                    queue.write_buffer(&self.uniforms, base, bytemuck::bytes_of(&uniforms));

                    self.pass(
                        device,
                        encoder,
                        Kind::ColorMatrix,
                        &targets.views[0],
                        &targets.views[1],
                        base,
                        true,
                        None,
                    );

                    source = 1;
                }
                // Identical machinery to a foreground blur; the difference
                // is entirely *when* it runs, which `prepare` already decided
                // by routing it into the backdrop queue.
                Effect::BackdropBlur(backdrop) => {
                    uniforms.params[0] = backdrop.radius;

                    for (pass, direction) in [(0usize, [1.0, 0.0]), (1, [0.0, 1.0])] {
                        let slot = base + pass as u64 * Uniforms::STRIDE;

                        uniforms.step[2] = direction[0];
                        uniforms.step[3] = direction[1];

                        queue.write_buffer(&self.uniforms, slot, bytemuck::bytes_of(&uniforms));

                        let destination = 1 - source;

                        self.pass(
                            device,
                            encoder,
                            Kind::Blur,
                            &targets.views[source],
                            &targets.views[destination],
                            slot,
                            true,
                            None,
                        );

                        source = destination;
                    }
                }
                Effect::Noise(noise) => {
                    // Fixed amplitude: `Noise` carries only a seed, and grain
                    // a user can see *as* grain is a bug in a UI. This is
                    // enough to break up the banding a wide gradient or a
                    // heavy blur produces, and no more.
                    uniforms.params[1] = 0.02;
                    uniforms.params[2] = f32::from_bits(noise.seed);

                    queue.write_buffer(&self.uniforms, base, bytemuck::bytes_of(&uniforms));

                    self.pass(
                        device,
                        encoder,
                        Kind::Noise,
                        &targets.views[0],
                        &targets.views[1],
                        base,
                        true,
                        None,
                    );

                    source = 1;
                }
                Effect::Mask(mask) => {
                    // A slot nobody uploaded to would clip everything to
                    // whatever the atlas happened to contain. Skipping is the
                    // only safe reading of an out-of-range slot.
                    if mask.atlas_slot >= Masks::SLOTS {
                        continue;
                    }

                    queue.write_buffer(&self.uniforms, base, bytemuck::bytes_of(&uniforms));

                    self.pass(
                        device,
                        encoder,
                        Kind::Mask,
                        &targets.views[0],
                        &targets.views[1],
                        base,
                        true,
                        Some(mask.atlas_slot),
                    );

                    source = 1;
                }
                // A shadow is drawn by the quad pipeline, which knows the
                // shape casting it. Re-deriving one here from a rasterised
                // region would give every widget a rectangle's shadow,
                // whatever its actual outline.
                Effect::Shadow(_) => {
                    continue;
                }
            }

            // Put the result back on the frame, from a slot of its own.
            let blit = base + 2 * Uniforms::STRIDE;

            uniforms.step[2] = 0.0;
            uniforms.step[3] = 0.0;
            queue.write_buffer(&self.uniforms, blit, bytemuck::bytes_of(&uniforms));

            self.pass(
                device,
                encoder,
                Kind::Blit,
                &targets.views[source],
                &frame_view,
                blit,
                false,
                None,
            );
        }
    }

    /// Runs one fragment pass from `source` into `destination`.
    #[allow(clippy::too_many_arguments)]
    fn pass(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        kind: Kind,
        source: &wgpu::TextureView,
        destination: &wgpu::TextureView,
        slot: u64,
        clear: bool,
        mask: Option<u32>,
    ) {
        let uniform_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("taim_wgpu.effect.uniform_bind_group"),
            layout: &self.uniform_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.uniforms,
                        offset: 0,
                        size: wgpu::BufferSize::new(std::mem::size_of::<Uniforms>() as u64),
                    }),
                },
            ],
        });

        // The mask pipeline binds source *and* mask in group 1, so it needs
        // its own layout; every other pass binds the source alone.
        let texture_group = match mask {
            Some(slot) => device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("taim_wgpu.effect.mask_bind_group"),
                layout: &self.masks.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(source),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(
                            &self.masks.views[slot as usize],
                        ),
                    },
                ],
            }),
            None => device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("taim_wgpu.effect.texture_bind_group"),
                layout: &self.texture_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                }],
            }),
        };

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("taim_wgpu.effect.pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: destination,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: if clear {
                        wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(match kind {
            Kind::Blur => &self.blur,
            Kind::ColorMatrix => &self.color_matrix,
            Kind::Noise => &self.noise,
            Kind::Blit => &self.blit,
            Kind::Mask => &self.mask,
        });

        pass.set_bind_group(0, &uniform_group, &[slot as u32]);
        pass.set_bind_group(1, &texture_group, &[]);

        pass.draw(0..6, 0..1);
    }

    /// Allocates the ping-pong targets, reusing them when the size is unchanged.
    fn ensure_targets(&mut self, device: &wgpu::Device, (width, height): (u32, u32)) {
        if let Some(targets) = &self.targets
            && targets.width == width
            && targets.height == height
        {
            return;
        }

        let make = |label: &str| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: width.max(1),
                    height: height.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        };

        let textures = [
            make("taim_wgpu.effect.target.0"),
            make("taim_wgpu.effect.target.1"),
        ];
        let views = [
            textures[0].create_view(&wgpu::TextureViewDescriptor::default()),
            textures[1].create_view(&wgpu::TextureViewDescriptor::default()),
        ];

        self.targets = Some(Targets {
            width,
            height,
            views,
            _textures: textures,
        });
    }

    /// Grows the uniform buffer when a frame queues more effects than it holds.
    fn ensure_uniform_capacity(&mut self, device: &wgpu::Device) {
        let needed =
            (self.backdrop.len() + self.foreground.len()) as u64 * Uniforms::SLOTS_PER_EFFECT;

        if needed <= self.capacity {
            return;
        }

        let capacity = needed.next_power_of_two();

        self.uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taim_wgpu.effect.uniforms"),
            size: Uniforms::STRIDE * capacity,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.capacity = capacity;
    }

    /// Uploads a coverage mask and returns the slot addressing it.
    ///
    /// `coverage` is one byte per pixel, row-major, `Masks::SIZE` square: 255
    /// keeps a pixel, 0 erases it, and the values between antialias the edge.
    /// The mask is sampled across whatever region the effect covers, so it
    /// describes a *shape*, not a resolution.
    ///
    /// Returns `None` for an out-of-range slot or a wrongly sized mask.
    pub fn upload_mask(&self, queue: &wgpu::Queue, slot: u32, coverage: &[u8]) -> Option<u32> {
        let expected = (Masks::SIZE * Masks::SIZE) as usize;

        if slot >= Masks::SLOTS || coverage.len() != expected {
            return None;
        }

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.masks.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: slot,
                },
                aspect: wgpu::TextureAspect::All,
            },
            coverage,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(Masks::SIZE),
                rows_per_image: Some(Masks::SIZE),
            },
            wgpu::Extent3d {
                width: Masks::SIZE,
                height: Masks::SIZE,
                depth_or_array_layers: 1,
            },
        );

        Some(slot)
    }

    /// The edge length every uploaded mask must have.
    pub const fn mask_size() -> u32 {
        Masks::SIZE
    }

    /// How many mask slots the atlas holds.
    pub const fn mask_slots() -> u32 {
        Masks::SLOTS
    }

    /// Releases the offscreen targets.
    ///
    /// Called when the renderer trims; a window that has stopped using
    /// effects should not hold two surface-sized textures indefinitely.
    pub fn trim(&mut self) {
        self.targets = None;
        self.backdrop.clear();
        self.foreground.clear();
    }
}

/// Writes a 4x5 colour matrix into the uniform block.
///
/// Stored row-major as `[r0..r4, g0..g4, b0..b4, a0..a4]`, where the fifth
/// entry of each row is the bias -- the conventional layout, matching
/// `ColorMatrix::IDENTITY`.
fn write_matrix(uniforms: &mut Uniforms, values: &[f32; 20]) {
    uniforms.matrix_r = [values[0], values[1], values[2], values[3]];
    uniforms.matrix_g = [values[5], values[6], values[7], values[8]];
    uniforms.matrix_b = [values[10], values[11], values[12], values[13]];
    uniforms.matrix_a = [values[15], values[16], values[17], values[18]];
    uniforms.bias = [values[4], values[9], values[14], values[19]];
}

/// The colour matrix for a saturation of `amount`.
///
/// `0.0` is greyscale, `1.0` the identity, above `1.0` oversaturated. The
/// luminance weights are Rec. 709, so desaturating keeps perceived
/// brightness rather than averaging the channels.
fn saturation_matrix(amount: f32) -> [f32; 20] {
    const R: f32 = 0.2126;
    const G: f32 = 0.7152;
    const B: f32 = 0.0722;

    let inverse = 1.0 - amount;

    [
        R * inverse + amount,
        G * inverse,
        B * inverse,
        0.0,
        0.0,
        R * inverse,
        G * inverse + amount,
        B * inverse,
        0.0,
        0.0,
        R * inverse,
        G * inverse,
        B * inverse + amount,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
    ]
}

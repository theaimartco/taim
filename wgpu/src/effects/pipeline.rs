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
    Blit,
}

pub struct Pipeline {
    format: wgpu::TextureFormat,
    sampler: wgpu::Sampler,
    uniform_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    blur: wgpu::RenderPipeline,
    color_matrix: wgpu::RenderPipeline,
    blit: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    /// How many uniform slots `uniforms` currently holds.
    capacity: u64,
    targets: Option<Targets>,
    queued: Vec<Queued>,
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
            blit: self.blit.clone(),
            uniforms: self.uniforms.clone(),
            capacity: self.capacity,
            targets: None,
            queued: Vec::new(),
        }
    }
}

impl Pipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("aura_wgpu.effect.sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            // Linear, so a downsampled blur does not band.
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });

        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("aura_wgpu.effect.uniform_layout"),
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
            label: Some("aura_wgpu.effect.texture_layout"),
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
            label: Some("aura_wgpu.effect.pipeline_layout"),
            bind_group_layouts: &[Some(&uniform_layout), Some(&texture_layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("aura_wgpu.effect.shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shader/effect.wgsl"))),
        });

        let build = |label: &str, entry: &str| {
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
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
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

        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("aura_wgpu.effect.uniforms"),
            size: Uniforms::STRIDE * 24,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            format,
            sampler,
            uniform_layout,
            texture_layout,
            blur: build("aura_wgpu.effect.blur", "fs_blur"),
            color_matrix: build("aura_wgpu.effect.color_matrix", "fs_color_matrix"),
            blit: build("aura_wgpu.effect.blit", "fs_blit"),
            uniforms,
            capacity: 24,
            targets: None,
            queued: Vec::new(),
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
        graph: &Graph,
        source: Rectangle,
        surface: Rectangle,
        tier: crate::graphics::quality::Tier,
        target: (u32, u32),
    ) {
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

            self.queued.push(Queued {
                effect: *effect,
                region,
            });
        }

        if self.queued.is_empty() {
            return;
        }

        self.ensure_targets(device, target);
        self.ensure_uniform_capacity(device);
    }

    /// Applies every queued effect to `frame`, then clears the queue.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::Texture,
        target: (u32, u32),
    ) {
        if self.queued.is_empty() {
            return;
        }

        let Some(targets) = &self.targets else {
            self.queued.clear();
            return;
        };

        let frame_view = frame.create_view(&wgpu::TextureViewDescriptor::default());
        let queued = std::mem::take(&mut self.queued);

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
                    );

                    source = 1;
                }
                // Shadow bounds are already handled by the quad pipeline's
                // own shadow, mask is a clip rather than a colour operation,
                // and noise needs a source of randomness this pass does not
                // carry. Each is queued and skipped rather than silently
                // dropped at `prepare`, so `Graph` stays the one place that
                // decides what an effect costs.
                Effect::Shadow(_)
                | Effect::Noise(_)
                | Effect::Mask(_)
                | Effect::BackdropBlur(_) => {
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
    ) {
        let uniform_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("aura_wgpu.effect.uniform_bind_group"),
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

        let texture_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("aura_wgpu.effect.texture_bind_group"),
            layout: &self.texture_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(source),
            }],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("aura_wgpu.effect.pass"),
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
            Kind::Blit => &self.blit,
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
            make("aura_wgpu.effect.target.0"),
            make("aura_wgpu.effect.target.1"),
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
        let needed = self.queued.len() as u64 * Uniforms::SLOTS_PER_EFFECT;

        if needed <= self.capacity {
            return;
        }

        let capacity = needed.next_power_of_two();

        self.uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("aura_wgpu.effect.uniforms"),
            size: Uniforms::STRIDE * capacity,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.capacity = capacity;
    }

    /// Releases the offscreen targets.
    ///
    /// Called when the renderer trims; a window that has stopped using
    /// effects should not hold two surface-sized textures indefinitely.
    pub fn trim(&mut self) {
        self.targets = None;
        self.queued.clear();
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

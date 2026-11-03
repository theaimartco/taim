//! A mip chain over the glass backdrop, so frosted glass can read a wide,
//! smooth blur with a handful of samples.
//!
//! After the frame is copied into the backdrop texture, each level is drawn
//! from the one above it with a four-tap tent filter (a 4x4 box, bilinearly
//! sampled), which is smooth enough that sampling between levels shows no
//! blocks. A frosted fragment then samples the level that matches its blur
//! radius instead of spending dozens of taps on the full-resolution frame,
//! which is what made per-pixel blurs grainy.

/// How many levels below full resolution are kept. Six halvings cover a
/// blur of well over a hundred pixels, more than any interface needs.
const MAX_LEVELS: u32 = 7;

/// The number of mip levels for a backdrop of `size`.
pub(crate) fn levels(size: wgpu::Extent3d) -> u32 {
    let longest = size.width.max(size.height).max(1);
    (32 - longest.leading_zeros()).clamp(1, MAX_LEVELS)
}

const SHADER: &str = r"
struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> Out {
    // One triangle covering the target.
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: Out;
    out.uv = uv;
    out.position = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    return out;
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var linear: sampler;

@fragment
fn fs_main(input: Out) -> @location(0) vec4<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(source));
    let half_texel = texel * 0.5;

    // Four bilinear taps at the corners of the 2x2 block this target texel
    // covers, each averaging four source texels: a 4x4 tent.
    let a = textureSampleLevel(source, linear, input.uv + vec2<f32>(-half_texel.x, -half_texel.y), 0.0);
    let b = textureSampleLevel(source, linear, input.uv + vec2<f32>(half_texel.x, -half_texel.y), 0.0);
    let c = textureSampleLevel(source, linear, input.uv + vec2<f32>(-half_texel.x, half_texel.y), 0.0);
    let d = textureSampleLevel(source, linear, input.uv + vec2<f32>(half_texel.x, half_texel.y), 0.0);

    return (a + b + c + d) * 0.25;
}
";

/// The pipeline and per-level bindings that fill a backdrop's mip chain.
pub(crate) struct Mips {
    pipeline: wgpu::RenderPipeline,
    /// `(source view of level n - 1, target view of level n)` for each
    /// level below the top.
    steps: Vec<(wgpu::BindGroup, wgpu::TextureView)>,
}

impl Mips {
    pub fn new(
        device: &wgpu::Device,
        texture: &wgpu::Texture,
        format: wgpu::TextureFormat,
        levels: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("taim_wgpu::backdrop mips shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(SHADER)),
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("taim_wgpu::backdrop mips layout"),
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("taim_wgpu::backdrop mips pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("taim_wgpu::backdrop mips"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("taim_wgpu::backdrop mips sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });

        let level_view = |level: u32| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("taim_wgpu::backdrop mip level"),
                base_mip_level: level,
                mip_level_count: Some(1),
                ..wgpu::TextureViewDescriptor::default()
            })
        };

        let steps = (1..levels)
            .map(|level| {
                let source = level_view(level - 1);
                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("taim_wgpu::backdrop mip source"),
                    layout: &layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&source),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                    ],
                });
                (bind_group, level_view(level))
            })
            .collect();

        Self { pipeline, steps }
    }

    /// Fills every level below the top from the one above it. Call after
    /// the top level has received the frame.
    pub fn generate(&self, encoder: &mut wgpu::CommandEncoder) {
        for (source, target) in &self.steps {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("taim_wgpu::backdrop mip pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, source, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::levels;

    fn size(width: u32, height: u32) -> wgpu::Extent3d {
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        }
    }

    #[test]
    fn levels_halve_down_to_a_cap() {
        assert_eq!(levels(size(1, 1)), 1);
        assert_eq!(levels(size(2, 1)), 2);
        assert_eq!(levels(size(480, 360)), 7);
        assert_eq!(levels(size(3840, 2160)), 7, "capped");
    }
}

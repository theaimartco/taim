//! Deferred G-Buffer pipeline for UI materials (Pillar III).
//!
//! Owns the 3-target G-buffer attachments:
//! - RT0 (`Rgba16Float`): Emissive Radiance (.rgb) + Analytic Coverage (.a)
//! - RT1 (`Rgba16Float`): Octahedral Normal (.rg) + Roughness (.b) + Thickness mm (.a)
//! - RT2 (`R8Uint`): Material Table Index (.r)
//!
//! Evaluates the deferred lighting pass with `light.wgsl`.

use crate::core::material::SceneLight;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LightUniforms {
    pub light_dir: [f32; 3],
    pub pixels_per_mm: f32,
    pub light_color: [f32; 3],
    pub light_intensity: f32,
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub pad0: f32,
    pub pad1: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuMaterial {
    pub roughness: f32,
    pub thickness_mm: f32,
    pub ior: f32,
    pub dispersion: f32,
    pub transmission: f32,
    pub pad0: f32,
    pub pad1: f32,
    pub pad2: f32,
}

/// A deferred G-buffer managing 3 surface attachments and the physical lighting pass.
pub struct GBuffer {
    pub width: u32,
    pub height: u32,

    pub rt0: wgpu::Texture,
    pub rt0_view: wgpu::TextureView,

    pub rt1: wgpu::Texture,
    pub rt1_view: wgpu::TextureView,

    pub rt2: wgpu::Texture,
    pub rt2_view: wgpu::TextureView,

    uniform_buf: wgpu::Buffer,
    material_table_buf: wgpu::Buffer,
    sampler: wgpu::Sampler,

    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}

impl GBuffer {
    /// Shader source for the deferred lighting pass.
    pub const SHADER: &'static str = include_str!("shader/light.wgsl");

    /// Creates a new [`GBuffer`] with dimensions `(width, height)` for target `format`.
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let width = width.max(1);
        let height = height.max(1);

        let rt0 = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("taim_gbuffer::rt0"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let rt0_view = rt0.create_view(&wgpu::TextureViewDescriptor::default());

        let rt1 = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("taim_gbuffer::rt1"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let rt1_view = rt1.create_view(&wgpu::TextureViewDescriptor::default());

        let rt2 = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("taim_gbuffer::rt2"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Uint,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let rt2_view = rt2.create_view(&wgpu::TextureViewDescriptor::default());

        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taim_gbuffer::uniforms"),
            size: std::mem::size_of::<LightUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let default_materials = [GpuMaterial {
            roughness: 0.1,
            thickness_mm: 8.0,
            ior: 1.52,
            dispersion: 0.04,
            transmission: 0.85,
            pad0: 0.0,
            pad1: 0.0,
            pad2: 0.0,
        }; 32];
        let material_table_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taim_gbuffer::materials"),
            size: (default_materials.len() * std::mem::size_of::<GpuMaterial>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("taim_gbuffer::sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("taim_gbuffer::layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("taim_gbuffer::lighting_shader"),
            source: wgpu::ShaderSource::Wgsl(Self::SHADER.into()),
        });

        let pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("taim_gbuffer::pipeline_layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("taim_gbuffer::pipeline"),
            layout: Some(&pipe_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            width,
            height,
            rt0,
            rt0_view,
            rt1,
            rt1_view,
            rt2,
            rt2_view,
            uniform_buf,
            material_table_buf,
            sampler,
            layout,
            pipeline,
        }
    }

    /// Dispatches the deferred lighting render pass.
    pub fn render_lighting(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        backdrop_view: &wgpu::TextureView,
        light: SceneLight,
        pixels_per_mm: f32,
    ) {
        let light_dir = light.unit_direction();
        let uniforms = LightUniforms {
            light_dir,
            pixels_per_mm,
            light_color: [
                light.radiance.red,
                light.radiance.green,
                light.radiance.blue,
            ],
            light_intensity: light.radiance.alpha.max(1.0),
            viewport_width: self.width as f32,
            viewport_height: self.height as f32,
            pad0: 0.0,
            pad1: 0.0,
        };
        queue.write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&uniforms));

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("taim_gbuffer::lighting_bind_group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.material_table_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.rt0_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&self.rt1_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&self.rt2_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(backdrop_view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("taim_gbuffer::lighting_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

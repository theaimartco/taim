use super::Cache;
use super::batch::{Instance, Uniforms};

use crate::graphics::Shell;

use std::mem;

/// Immutable image pipeline state shared by all image layers.
#[derive(Debug, Clone)]
pub struct Pipeline {
    pub(super) raw: wgpu::RenderPipeline,
    backend: wgpu::Backend,
    nearest_sampler: wgpu::Sampler,
    linear_sampler: wgpu::Sampler,
    texture_layout: wgpu::BindGroupLayout,
    pub(super) constant_layout: wgpu::BindGroupLayout,
}

impl Pipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, backend: wgpu::Backend) -> Self {
        let nearest_sampler = sampler(device, wgpu::FilterMode::Nearest, "nearest");
        let linear_sampler = sampler(device, wgpu::FilterMode::Linear, "linear");
        let constant_layout = constants_layout(device);
        let texture_layout = texture_layout(device);
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("taim.image.pipeline.layout"),
            bind_group_layouts: &[Some(&constant_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("taim.image.shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(concat!(
                include_str!("../shader/vertex.wgsl"),
                "\n",
                include_str!("../shader/color.wgsl"),
                "\n",
                include_str!("../shader/image.wgsl"),
            ))),
        });
        let raw = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("taim.image.pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array!(
                        0 => Float32x2, // center
                        1 => Float32x4, // clip bounds
                        2 => Float32x4, // border radius
                        3 => Float32x4, // tile
                        4 => Float32x2, // rotation cosine + sine
                        5 => Float32,   // opacity
                        6 => Float32x2, // atlas position
                        7 => Float32x2, // atlas scale
                        8 => Sint32,    // texture-array layer
                    ),
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Cw,
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            raw,
            backend,
            nearest_sampler,
            linear_sampler,
            texture_layout,
            constant_layout,
        }
    }

    pub fn create_cache(&self, device: &wgpu::Device, queue: &wgpu::Queue, shell: &Shell) -> Cache {
        Cache::new(
            device,
            queue,
            self.backend,
            self.texture_layout.clone(),
            shell,
        )
    }

    pub(super) fn create_state_layer(&self, device: &wgpu::Device) -> super::batch::Layer {
        super::batch::Layer::new(
            device,
            &self.constant_layout,
            &self.nearest_sampler,
            &self.linear_sampler,
        )
    }
}

fn sampler(device: &wgpu::Device, filter: wgpu::FilterMode, name: &str) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some(if name == "nearest" {
            "taim.image.sampler.nearest"
        } else {
            "taim.image.sampler.linear"
        }),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: filter,
        min_filter: filter,
        mipmap_filter: match filter {
            wgpu::FilterMode::Nearest => wgpu::MipmapFilterMode::Nearest,
            wgpu::FilterMode::Linear => wgpu::MipmapFilterMode::Linear,
        },
        ..wgpu::SamplerDescriptor::default()
    })
}

fn constants_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("taim.image.constants.layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(mem::size_of::<Uniforms>() as u64),
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
    })
}

fn texture_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("taim.image.texture.layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2Array,
                multisampled: false,
            },
            count: None,
        }],
    })
}

use super::Uniforms;
use crate::graphics::Antialiasing;
use crate::graphics::mesh;

use std::sync::atomic::{AtomicU64, Ordering};

/// Solid, gradient, and optional multisample triangle pipelines.
#[derive(Debug, Clone)]
pub struct Pipeline {
    pub(super) id: u64,
    pub(super) msaa: Option<super::msaa::Pipeline>,
    pub(super) solid: solid::Pipeline,
    pub(super) gradient: gradient::Pipeline,
}

impl Pipeline {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        antialiasing: Option<Antialiasing>,
    ) -> Self {
        static NEXT_PIPELINE_ID: AtomicU64 = AtomicU64::new(1);

        Self {
            id: NEXT_PIPELINE_ID.fetch_add(1, Ordering::Relaxed),
            msaa: antialiasing.map(|quality| super::msaa::Pipeline::new(device, format, quality)),
            solid: solid::Pipeline::new(device, format, antialiasing),
            gradient: gradient::Pipeline::new(device, format, antialiasing),
        }
    }
}

pub(super) fn color_target(format: wgpu::TextureFormat) -> wgpu::ColorTargetState {
    wgpu::ColorTargetState {
        format,
        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
    }
}

fn primitive() -> wgpu::PrimitiveState {
    wgpu::PrimitiveState {
        topology: wgpu::PrimitiveTopology::TriangleList,
        front_face: wgpu::FrontFace::Cw,
        ..wgpu::PrimitiveState::default()
    }
}

fn multisample(antialiasing: Option<Antialiasing>) -> wgpu::MultisampleState {
    wgpu::MultisampleState {
        count: antialiasing.map(Antialiasing::sample_count).unwrap_or(1),
        ..wgpu::MultisampleState::default()
    }
}

fn constants_layout(device: &wgpu::Device, label: &'static str) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[Uniforms::layout_entry()],
    })
}

fn constants(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
    label: &'static str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer,
                offset: 0,
                size: Uniforms::binding_size(),
            }),
        }],
    })
}

pub(super) mod solid {
    use super::*;
    use crate::Buffer;

    #[derive(Debug, Clone)]
    pub struct Pipeline {
        pub raw: wgpu::RenderPipeline,
        pub constants_layout: wgpu::BindGroupLayout,
    }

    #[derive(Debug)]
    pub struct Buffers {
        pub vertices: Buffer<mesh::SolidVertex2D>,
        pub uniforms: Buffer<Uniforms>,
        pub constants: wgpu::BindGroup,
    }

    impl Buffers {
        pub fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
            let vertices = Buffer::new(
                device,
                "taim.triangle.solid.vertices",
                super::super::draw::INITIAL_VERTEX_COUNT,
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            );
            let uniforms = Buffer::new(
                device,
                "taim.triangle.solid.uniforms",
                1,
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            );
            let constants = super::constants(
                device,
                layout,
                &uniforms.raw,
                "taim.triangle.solid.constants",
            );
            Self {
                vertices,
                uniforms,
                constants,
            }
        }

        pub fn rebind(&mut self, device: &wgpu::Device, layout: &wgpu::BindGroupLayout) {
            self.constants = super::constants(
                device,
                layout,
                &self.uniforms.raw,
                "taim.triangle.solid.constants",
            );
        }
    }

    impl Pipeline {
        pub(super) fn new(
            device: &wgpu::Device,
            format: wgpu::TextureFormat,
            antialiasing: Option<Antialiasing>,
        ) -> Self {
            let constants_layout =
                super::constants_layout(device, "taim.triangle.solid.constants_layout");
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("taim.triangle.solid.layout"),
                bind_group_layouts: &[Some(&constants_layout)],
                immediate_size: 0,
            });
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("taim.triangle.solid.shader"),
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(concat!(
                    include_str!("../shader/triangle.wgsl"),
                    "\n",
                    include_str!("../shader/triangle/solid.wgsl"),
                    "\n",
                    include_str!("../shader/color.wgsl"),
                ))),
            });
            let raw = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("taim.triangle.solid.pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("solid_vs_main"),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<mesh::SolidVertex2D>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array!(
                            0 => Float32x2,
                            1 => Float32x4,
                        ),
                    }],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("solid_fs_main"),
                    targets: &[Some(super::color_target(format))],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: super::primitive(),
                depth_stencil: None,
                multisample: super::multisample(antialiasing),
                multiview_mask: None,
                cache: None,
            });
            Self {
                raw,
                constants_layout,
            }
        }
    }
}

pub(super) mod gradient {
    use super::*;
    use crate::Buffer;

    #[derive(Debug, Clone)]
    pub struct Pipeline {
        pub raw: wgpu::RenderPipeline,
        pub constants_layout: wgpu::BindGroupLayout,
    }

    #[derive(Debug)]
    pub struct Buffers {
        pub vertices: Buffer<mesh::GradientVertex2D>,
        pub uniforms: Buffer<Uniforms>,
        pub constants: wgpu::BindGroup,
    }

    impl Buffers {
        pub fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
            let vertices = Buffer::new(
                device,
                "taim.triangle.gradient.vertices",
                super::super::draw::INITIAL_VERTEX_COUNT,
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            );
            let uniforms = Buffer::new(
                device,
                "taim.triangle.gradient.uniforms",
                1,
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            );
            let constants = super::constants(
                device,
                layout,
                &uniforms.raw,
                "taim.triangle.gradient.constants",
            );
            Self {
                vertices,
                uniforms,
                constants,
            }
        }

        pub fn rebind(&mut self, device: &wgpu::Device, layout: &wgpu::BindGroupLayout) {
            self.constants = super::constants(
                device,
                layout,
                &self.uniforms.raw,
                "taim.triangle.gradient.constants",
            );
        }
    }

    impl Pipeline {
        pub(super) fn new(
            device: &wgpu::Device,
            format: wgpu::TextureFormat,
            antialiasing: Option<Antialiasing>,
        ) -> Self {
            let constants_layout =
                super::constants_layout(device, "taim.triangle.gradient.constants_layout");
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("taim.triangle.gradient.layout"),
                bind_group_layouts: &[Some(&constants_layout)],
                immediate_size: 0,
            });
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("taim.triangle.gradient.shader"),
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(concat!(
                    include_str!("../shader/triangle.wgsl"),
                    "\n",
                    include_str!("../shader/gradient.wgsl"),
                    "\n",
                    include_str!("../shader/triangle/gradient.wgsl"),
                    "\n",
                    include_str!("../shader/color.wgsl"),
                    "\n",
                    include_str!("../shader/color/linear_rgb.wgsl"),
                ))),
            });
            let raw = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("taim.triangle.gradient.pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("gradient_vs_main"),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<mesh::GradientVertex2D>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array!(
                            0 => Float32x2,
                            1 => Uint32x4,
                            2 => Uint32x4,
                            3 => Uint32x4,
                            4 => Uint32x4,
                            5 => Uint32x4,
                            6 => Float32x4,
                        ),
                    }],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("gradient_fs_main"),
                    targets: &[Some(super::color_target(format))],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: super::primitive(),
                depth_stencil: None,
                multisample: super::multisample(antialiasing),
                multiview_mask: None,
                cache: None,
            });
            Self {
                raw,
                constants_layout,
            }
        }
    }
}

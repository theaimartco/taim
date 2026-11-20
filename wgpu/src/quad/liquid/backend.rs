use super::Liquid;
use crate::Buffer;
use crate::quad;

use std::ops::Range;

#[derive(Debug)]
pub struct Layer {
    instances: Buffer<Liquid>,
}

impl Layer {
    pub fn new(device: &wgpu::Device) -> Self {
        let instances = Buffer::new(
            device,
            "taim_wgpu.quad.liquid.buffer",
            quad::INITIAL_INSTANCES,
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        );

        Self { instances }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        instances: &[Liquid],
    ) {
        if instances.is_empty() {
            return;
        }

        let _ = self.instances.resize(device, instances.len());
        let _ = self.instances.write(encoder, belt, 0, instances);
    }
}

#[derive(Debug, Clone)]
pub struct Pipeline {
    pipeline: wgpu::RenderPipeline,
}

impl Pipeline {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        constants_layout: &wgpu::BindGroupLayout,
        backdrop_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("taim_wgpu.quad.liquid.pipeline"),
            bind_group_layouts: &[Some(constants_layout), Some(backdrop_layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("taim_wgpu.quad.liquid.shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(concat!(
                include_str!("../../shader/color.wgsl"),
                "\n",
                include_str!("../../shader/material.wgsl"),
                "\n",
                include_str!("../../shader/quad.wgsl"),
                "\n",
                include_str!("../../shader/quad/backdrop.wgsl"),
                "\n",
                include_str!("../../shader/vertex.wgsl"),
                "\n",
                include_str!("../../shader/quad/geometry.wgsl"),
                "\n",
                include_str!("../../shader/quad/liquid.wgsl"),
            ))),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("taim_wgpu.quad.liquid.pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("liquid_vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Liquid>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array!(
                        // Color
                        0 => Float32x4,
                        // Position
                        1 => Float32x2,
                        // Size
                        2 => Float32x2,
                        // Border color
                        3 => Float32x4,
                        // Border radius
                        4 => Float32x4,
                        // Border width
                        5 => Float32,
                        // Shadow color
                        6 => Float32x4,
                        // Shadow offset
                        7 => Float32x2,
                        // Shadow blur radius
                        8 => Float32,
                        // Snap
                        9 => Uint32,
                        // Material: roughness, thickness, ior, dispersion
                        10 => Float32x4,
                        // Transmission
                        11 => Float32,
                        // Touch: x, y, strength, radius
                        12 => Float32x4,
                        // Neighbour A: x, y, width, height
                        13 => Float32x4,
                        // Neighbour B: x, y, width, height
                        14 => Float32x4,
                        // Shape: radius A, radius B, blend, legibility
                        15 => Float32x4,
                    ),
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("liquid_fs_main"),
                targets: &quad::color_target_state(format),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Cw,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        Self { pipeline }
    }

    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        constants: &'a wgpu::BindGroup,
        backdrop: &'a wgpu::BindGroup,
        layer: &'a Layer,
        range: Range<usize>,
    ) {
        if range.is_empty() {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, constants, &[]);
        render_pass.set_bind_group(1, backdrop, &[]);
        render_pass.set_vertex_buffer(0, layer.instances.slice(..));

        render_pass.draw(0..6, range.start as u32..range.end as u32);
    }
}

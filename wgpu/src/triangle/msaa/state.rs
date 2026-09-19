use super::{Pipeline, targets};
use crate::core::{Size, Transformation};

#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub(super) struct Ratio {
    u: f32,
    v: f32,
    _padding: [f32; 2],
}

/// Renderer-local MSAA uniforms and target pool.
pub struct State {
    ratio_buffer: wgpu::Buffer,
    constants: wgpu::BindGroup,
    last_ratio: Option<Ratio>,
    targets: targets::Pool,
}

impl State {
    pub fn new(device: &wgpu::Device, pipeline: &Pipeline) -> Self {
        let ratio_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taim.triangle.msaa.ratio"),
            size: std::mem::size_of::<Ratio>() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });
        let constants = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("taim.triangle.msaa.constants"),
            layout: &pipeline.constant_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&pipeline.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ratio_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            ratio_buffer,
            constants,
            last_ratio: None,
            targets: targets::Pool::default(),
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        pipeline: &Pipeline,
        requested: Size<u32>,
    ) -> Transformation {
        let maximum = device.limits().max_texture_dimension_2d;
        let requested = Size::new(
            requested.width.max(1).min(maximum),
            requested.height.max(1).min(maximum),
        );
        let capacity = self.targets.ensure(device, pipeline, requested);
        let ratio = Ratio {
            u: requested.width as f32 / capacity.width as f32,
            v: requested.height as f32 / capacity.height as f32,
            _padding: [0.0; 2],
        };
        if self.last_ratio != Some(ratio) {
            let bytes = bytemuck::bytes_of(&ratio);
            belt.write_buffer(
                encoder,
                &self.ratio_buffer,
                0,
                wgpu::BufferSize::new(bytes.len() as u64).expect("non-empty MSAA ratio"),
            )
            .copy_from_slice(bytes);
            self.last_ratio = Some(ratio);
        }
        Transformation::orthographic(capacity.width, capacity.height)
    }

    pub fn render_pass<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
    ) -> wgpu::RenderPass<'a> {
        self.targets.render_pass(encoder)
    }

    pub fn render(
        &self,
        pipeline: &Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("taim.triangle.msaa.resolve_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
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
        pass.set_pipeline(&pipeline.raw);
        pass.set_bind_group(0, &self.constants, &[]);
        pass.set_bind_group(1, self.targets.texture_binding(), &[]);
        pass.draw(0..6, 0..1);
    }
}

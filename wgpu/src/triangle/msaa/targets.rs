use super::Pipeline;
use crate::core::Size;

const SHRINK_AFTER_FRAMES: u16 = 120;

#[derive(Debug)]
struct Targets {
    attachment: wgpu::TextureView,
    resolve: wgpu::TextureView,
    texture_binding: wgpu::BindGroup,
    capacity: Size<u32>,
}

impl Targets {
    fn allocate(device: &wgpu::Device, pipeline: &Pipeline, capacity: Size<u32>) -> Self {
        let extent = wgpu::Extent3d {
            width: capacity.width,
            height: capacity.height,
            depth_or_array_layers: 1,
        };
        let attachment = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("taim.triangle.msaa.attachment"),
            size: extent,
            mip_level_count: 1,
            sample_count: pipeline.sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: pipeline.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let resolve = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("taim.triangle.msaa.resolve"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: pipeline.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let attachment = attachment.create_view(&wgpu::TextureViewDescriptor {
            label: Some("taim.triangle.msaa.attachment_view"),
            ..wgpu::TextureViewDescriptor::default()
        });
        let resolve = resolve.create_view(&wgpu::TextureViewDescriptor {
            label: Some("taim.triangle.msaa.resolve_view"),
            ..wgpu::TextureViewDescriptor::default()
        });
        let texture_binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("taim.triangle.msaa.texture_binding"),
            layout: &pipeline.texture_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&resolve),
            }],
        });

        Self {
            attachment,
            resolve,
            texture_binding,
            capacity,
        }
    }
}

/// Geometrically growing target pool with delayed shrink hysteresis.
#[derive(Debug, Default)]
pub(super) struct Pool {
    targets: Option<Targets>,
    underused_frames: u16,
}

impl Pool {
    pub fn ensure(
        &mut self,
        device: &wgpu::Device,
        pipeline: &Pipeline,
        requested: Size<u32>,
    ) -> Size<u32> {
        let maximum = device.limits().max_texture_dimension_2d;
        let requested = Size::new(
            requested.width.max(1).min(maximum),
            requested.height.max(1).min(maximum),
        );
        let Some(current) = self.targets.as_ref().map(|targets| targets.capacity) else {
            self.targets = Some(Targets::allocate(device, pipeline, requested));
            return requested;
        };

        if requested.width > current.width || requested.height > current.height {
            let capacity = Size::new(
                grow(current.width, requested.width, maximum),
                grow(current.height, requested.height, maximum),
            );
            self.targets = Some(Targets::allocate(device, pipeline, capacity));
            self.underused_frames = 0;
            return capacity;
        }

        let heavily_underused = requested.width.saturating_mul(2) < current.width
            || requested.height.saturating_mul(2) < current.height;
        self.underused_frames = if heavily_underused {
            self.underused_frames.saturating_add(1)
        } else {
            0
        };
        if self.underused_frames >= SHRINK_AFTER_FRAMES {
            let capacity = Size::new(
                headroom(requested.width, maximum),
                headroom(requested.height, maximum),
            );
            self.targets = Some(Targets::allocate(device, pipeline, capacity));
            self.underused_frames = 0;
            capacity
        } else {
            current
        }
    }

    pub fn render_pass<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
    ) -> wgpu::RenderPass<'a> {
        let targets = self.targets.as_ref().expect("MSAA prepare precedes render");
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("taim.triangle.msaa.geometry_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &targets.attachment,
                depth_slice: None,
                resolve_target: Some(&targets.resolve),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        })
    }

    pub fn texture_binding(&self) -> &wgpu::BindGroup {
        &self
            .targets
            .as_ref()
            .expect("MSAA prepare precedes render")
            .texture_binding
    }
}

fn grow(current: u32, required: u32, maximum: u32) -> u32 {
    current
        .saturating_add(current.div_ceil(2))
        .max(required)
        .min(maximum)
}

fn headroom(required: u32, maximum: u32) -> u32 {
    required
        .saturating_add(required.div_ceil(4))
        .max(1)
        .min(maximum)
}

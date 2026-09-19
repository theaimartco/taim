use super::Layer;
use crate::graphics::color;

use std::sync::Arc;

const RGBA_BYTES: u64 = 4;

/// Physical texture-array storage with geometric, budgeted growth.
#[derive(Debug)]
pub(super) struct TextureStore {
    side: u32,
    backend: wgpu::Backend,
    capacity: u32,
    maximum_layers: u32,
    texture: wgpu::Texture,
    bind_group: Arc<wgpu::BindGroup>,
    layout: wgpu::BindGroupLayout,
}

impl TextureStore {
    pub fn new(
        device: &wgpu::Device,
        backend: wgpu::Backend,
        layout: wgpu::BindGroupLayout,
        side: u32,
        preferred_initial_layers: u32,
        byte_budget: u64,
    ) -> Self {
        let bytes_per_layer = u64::from(side) * u64::from(side) * RGBA_BYTES;
        let budget_layers = u32::try_from(byte_budget / bytes_per_layer)
            .unwrap_or(u32::MAX)
            .max(1);
        let maximum_layers = device
            .limits()
            .max_texture_array_layers
            .min(budget_layers)
            .max(1);
        let capacity = preferred_initial_layers.min(maximum_layers).max(1);
        let texture = create_texture(device, side, capacity);
        let bind_group = create_bind_group(device, &layout, &texture);

        Self {
            side,
            backend,
            capacity,
            maximum_layers,
            texture,
            bind_group,
            layout,
        }
    }

    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    pub fn bind_group(&self) -> &Arc<wgpu::BindGroup> {
        &self.bind_group
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    pub fn maximum_layers(&self) -> u32 {
        self.maximum_layers
    }

    pub fn storage_bytes(&self) -> u64 {
        u64::from(self.side) * u64::from(self.side) * RGBA_BYTES * u64::from(self.capacity)
    }

    pub fn ensure_capacity(
        &mut self,
        required_layers: usize,
        logical_layers: &[Layer],
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
    ) -> bool {
        let Ok(required) = u32::try_from(required_layers) else {
            return false;
        };
        if required <= self.capacity {
            return true;
        }
        if required > self.maximum_layers {
            return false;
        }

        let geometric = self.capacity.saturating_add(self.capacity.div_ceil(2));
        let mut target = geometric.max(required).min(self.maximum_layers);

        // GLES may infer cube textures at exact multiples of six despite the
        // explicit D2-array view. Reserving one extra layer avoids that path.
        if self.backend == wgpu::Backend::Gl
            && target.is_multiple_of(6)
            && target < self.maximum_layers
        {
            target += 1;
        }

        let replacement = create_texture(device, self.side, target);
        self.copy_live_runs(&replacement, logical_layers, encoder);
        let replacement_bind_group = create_bind_group(device, &self.layout, &replacement);

        self.texture = replacement;
        self.bind_group = replacement_bind_group;
        self.capacity = target;
        true
    }

    fn copy_live_runs(
        &self,
        replacement: &wgpu::Texture,
        logical_layers: &[Layer],
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let old_layers = logical_layers.len().min(self.capacity as usize);
        let mut cursor = 0;

        while cursor < old_layers {
            while cursor < old_layers && logical_layers[cursor].is_empty() {
                cursor += 1;
            }
            let start = cursor;
            while cursor < old_layers && !logical_layers[cursor].is_empty() {
                cursor += 1;
            }
            if start == cursor {
                continue;
            }

            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: start as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: replacement,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: start as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: self.side,
                    height: self.side,
                    depth_or_array_layers: (cursor - start) as u32,
                },
            );
        }
    }
}

fn create_texture(device: &wgpu::Device, side: u32, layers: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("taim.image.atlas.texture"),
        size: wgpu::Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: layers,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: if color::GAMMA_CORRECTION {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        },
        usage: wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

fn create_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    texture: &wgpu::Texture,
) -> Arc<wgpu::BindGroup> {
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("taim.image.atlas.view"),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..wgpu::TextureViewDescriptor::default()
    });

    Arc::new(device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("taim.image.atlas.binding"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&view),
        }],
    }))
}

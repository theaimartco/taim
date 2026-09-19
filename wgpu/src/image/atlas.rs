//! TAIM's bounded, recyclable GPU image atlas.
//!
//! Allocation and GPU storage are deliberately separate: packing may add a
//! logical layer, while texture storage grows geometrically and copies live
//! layer runs in batches. This avoids rebuilding the whole texture array for
//! every extra layer without allowing image storage to grow without a budget.

pub mod entry;

mod allocation;
mod layer;
mod packing;
mod storage;
mod upload;

pub use allocation::Allocation;
pub use entry::Entry;
pub use layer::Layer;
pub(crate) use taim_atlas as allocator;

use packing::Packing;
use storage::TextureStore;

use std::sync::Arc;

/// Default side length of each image-atlas layer.
pub const DEFAULT_SIZE: u32 = 2048;
/// Upper bound for an image-atlas layer side.
pub const MAX_SIZE: u32 = 2048;
/// Per-atlas GPU storage ceiling before allocations fail gracefully.
pub const MAX_STORAGE_BYTES: u64 = 256 * 1024 * 1024;

/// Device-side image storage with recyclable logical layers.
#[derive(Debug)]
pub struct Atlas {
    packing: Packing,
    storage: TextureStore,
}

impl Atlas {
    pub fn new(
        device: &wgpu::Device,
        backend: wgpu::Backend,
        texture_layout: wgpu::BindGroupLayout,
    ) -> Self {
        Self::with_size(device, backend, texture_layout, DEFAULT_SIZE)
    }

    pub fn with_size(
        device: &wgpu::Device,
        backend: wgpu::Backend,
        texture_layout: wgpu::BindGroupLayout,
        requested_size: u32,
    ) -> Self {
        let side = requested_size
            .max(1)
            .min(MAX_SIZE)
            .min(device.limits().max_texture_dimension_2d);
        let preferred_initial_layers = if backend == wgpu::Backend::Gl { 2 } else { 1 };
        let storage = TextureStore::new(
            device,
            backend,
            texture_layout,
            side,
            preferred_initial_layers,
            MAX_STORAGE_BYTES,
        );
        let packing = Packing::new(side, storage.capacity(), storage.maximum_layers());

        Self { packing, storage }
    }

    pub fn bind_group(&self) -> &Arc<wgpu::BindGroup> {
        self.storage.bind_group()
    }

    /// Returns currently reserved GPU bytes, including spare growth capacity.
    pub fn storage_bytes(&self) -> u64 {
        self.storage.storage_bytes()
    }

    /// Returns the number of physical texture-array layers currently reserved.
    pub fn layer_capacity(&self) -> u32 {
        self.storage.capacity()
    }

    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Option<Entry> {
        if !upload::valid_rgba(width, height, pixels) {
            return None;
        }

        let entry = self.packing.allocate(width, height)?;
        if !self.storage.ensure_capacity(
            self.packing.layer_count(),
            self.packing.layers(),
            device,
            encoder,
        ) {
            self.packing.remove(&entry);
            return None;
        }

        let staged = match &entry {
            Entry::Contiguous(allocation) => upload::stage_rgba(
                self.storage.texture(),
                encoder,
                belt,
                pixels,
                width,
                0,
                allocation,
            ),
            Entry::Fragmented { fragments, .. } => fragments.iter().all(|fragment| {
                let (x, y) = fragment.position;
                let offset = u64::from(y)
                    .checked_mul(u64::from(width))
                    .and_then(|value| value.checked_add(u64::from(x)))
                    .and_then(|value| value.checked_mul(upload::RGBA_BYTES as u64))
                    .and_then(|value| usize::try_from(value).ok());

                offset.is_some_and(|offset| {
                    upload::stage_rgba(
                        self.storage.texture(),
                        encoder,
                        belt,
                        pixels,
                        width,
                        offset,
                        &fragment.allocation,
                    )
                })
            }),
        };

        if !staged {
            self.packing.remove(&entry);
            return None;
        }

        log::debug!(
            "TAIM image atlas: logical_layers={}, capacity={}, busy={}, allocations={}, bytes={}",
            self.packing.layer_count(),
            self.layer_capacity(),
            self.packing.busy_layers(),
            self.packing.allocations(),
            self.storage_bytes(),
        );

        Some(entry)
    }

    pub fn remove(&mut self, entry: &Entry) {
        self.packing.remove(entry);
    }
}

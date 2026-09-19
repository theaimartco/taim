use super::COLOR_MODE;
use crate::core::Size;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock, Weak};
use taim_text_gpu as gpu;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct Id(u64);

pub struct Viewport(pub(super) gpu::Viewport);

impl Viewport {
    pub fn update(&mut self, queue: &wgpu::Queue, resolution: Size<u32>) {
        self.0.update(
            queue,
            gpu::Resolution {
                width: resolution.width,
                height: resolution.height,
            },
        );
    }
}

#[derive(Clone)]
pub struct Pipeline {
    pub(super) id: Id,
    pub(super) format: wgpu::TextureFormat,
    pub(super) cache: gpu::Cache,
    pub(super) atlas: Arc<RwLock<gpu::TextAtlas>>,
    lifetime: Arc<()>,
}

impl Pipeline {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);

        let cache = gpu::Cache::new(device);
        let atlas = gpu::TextAtlas::with_color_mode(device, queue, &cache, format, COLOR_MODE);

        Self {
            id: Id(NEXT_ID.fetch_add(1, Ordering::Relaxed)),
            format,
            cache,
            atlas: Arc::new(RwLock::new(atlas)),
            lifetime: Arc::new(()),
        }
    }

    pub fn create_viewport(&self, device: &wgpu::Device) -> Viewport {
        Viewport(gpu::Viewport::new(device, &self.cache))
    }

    pub fn trim(&self) {
        self.atlas
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .trim();
    }

    pub(super) fn lifetime(&self) -> Weak<()> {
        Arc::downgrade(&self.lifetime)
    }
}

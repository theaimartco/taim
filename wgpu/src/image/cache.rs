use crate::core;
#[cfg(feature = "svg")]
use crate::core::Size;
use crate::graphics::Shell;
use crate::image::atlas::{self, Atlas};

#[cfg(all(feature = "image", not(target_arch = "wasm32")))]
#[path = "cache/worker.rs"]
mod worker;

#[cfg(all(feature = "image", not(target_arch = "wasm32")))]
#[path = "cache/requests.rs"]
mod requests;

#[cfg(feature = "image")]
#[path = "cache/raster.rs"]
mod raster;

#[cfg(feature = "image")]
use smallvec::SmallVec;
#[cfg(feature = "svg")]
use std::sync::Arc;
#[cfg(all(feature = "image", not(target_arch = "wasm32")))]
use worker::Worker;

#[cfg(feature = "image")]
type Callback = Box<dyn FnOnce(Result<core::image::Allocation, core::image::Error>) + Send>;

#[cfg(feature = "image")]
type CallbackList = SmallVec<[Callback; 1]>;

/// Device-side image storage shared by raster and vector primitives.
pub struct Cache {
    atlas: Atlas,
    #[cfg(feature = "image")]
    raster: RasterStore,
    #[cfg(feature = "svg")]
    vector: crate::image::vector::Cache,
    #[cfg(all(feature = "image", not(target_arch = "wasm32")))]
    worker: Worker,
}

#[cfg(feature = "image")]
struct RasterStore {
    resident: crate::image::raster::Cache,
    #[cfg(not(target_arch = "wasm32"))]
    requests: requests::Queue,
    blocking_uploads: wgpu::util::StagingBelt,
}

impl Cache {
    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        backend: wgpu::Backend,
        layout: wgpu::BindGroupLayout,
        _shell: &Shell,
    ) -> Self {
        #[cfg(all(feature = "image", not(target_arch = "wasm32")))]
        let worker = Worker::new(device, _queue, backend, layout.clone(), _shell);

        Self {
            atlas: Atlas::new(device, backend, layout),
            #[cfg(feature = "image")]
            raster: RasterStore {
                resident: crate::image::raster::Cache::default(),
                #[cfg(not(target_arch = "wasm32"))]
                requests: requests::Queue::default(),
                blocking_uploads: wgpu::util::StagingBelt::new(device.clone(), 2 * 1024 * 1024),
            },
            #[cfg(feature = "svg")]
            vector: crate::image::vector::Cache::default(),
            #[cfg(all(feature = "image", not(target_arch = "wasm32")))]
            worker,
        }
    }

    #[cfg(feature = "svg")]
    pub fn measure_svg(&mut self, handle: &core::svg::Handle) -> Size<u32> {
        self.vector.load(handle).viewport_dimensions()
    }

    #[cfg(feature = "svg")]
    pub fn upload_vector(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        handle: &core::svg::Handle,
        color: Option<core::Color>,
        size: Size<u32>,
    ) -> Option<(&atlas::Entry, &Arc<wgpu::BindGroup>)> {
        self.vector
            .upload(device, encoder, belt, handle, color, size, &mut self.atlas)
            .map(|entry| (entry, self.atlas.bind_group()))
    }

    pub fn trim(&mut self) {
        #[cfg(feature = "image")]
        {
            self.receive();
            #[cfg(not(target_arch = "wasm32"))]
            self.trim_requests();
            self.raster.resident.trim(&mut self.atlas, drop);
        }

        #[cfg(feature = "svg")]
        self.vector.trim(&mut self.atlas);
    }
}

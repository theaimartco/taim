#[cfg(target_arch = "wasm32")]
use super::CallbackList;
use super::{Cache, Callback, atlas};
use crate::core::{self, Size};
use crate::graphics::Shell;

use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use super::worker;

impl Cache {
    pub fn allocate_image(
        &mut self,
        handle: &core::image::Handle,
        callback: impl FnOnce(Result<core::image::Allocation, core::image::Error>) + Send + 'static,
    ) {
        self.receive();
        let callback: Callback = Box::new(callback);

        #[cfg(not(target_arch = "wasm32"))]
        {
            let callback = match self.raster.requests.attach_callback(handle.id(), callback) {
                Ok(()) => return,
                Err(callback) => callback,
            };
            return self.allocate_image_now(handle, callback);
        }

        #[cfg(target_arch = "wasm32")]
        self.allocate_image_now(handle, callback);
    }

    fn allocate_image_now(&mut self, handle: &core::image::Handle, callback: Callback) {
        use crate::image::raster::Memory;

        match self.raster.resident.get_mut(handle) {
            Some(Memory::Device {
                entry, allocation, ..
            }) => {
                if let Some(allocation) = allocation
                    .as_ref()
                    .and_then(core::image::Allocation::upgrade)
                {
                    callback(Ok(allocation));
                    return;
                }

                #[allow(unsafe_code)]
                let fresh = unsafe { core::image::allocate(handle, entry.size()) };
                *allocation = Some(fresh.downgrade());
                callback(Ok(fresh));
            }
            Some(Memory::Error(error)) => callback(Err(error.clone())),
            Some(Memory::Host(_image)) => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let pixels = worker::Pixels {
                        width: _image.width(),
                        height: _image.height(),
                        rgba: _image.clone().into_raw(),
                    };
                    self.queue_upload(handle, pixels, Some(callback), Shell::tick);
                }

                #[cfg(target_arch = "wasm32")]
                callback(Err(core::image::Error::Unsupported));
            }
            None => {
                #[cfg(not(target_arch = "wasm32"))]
                self.queue_decode(handle, Some(callback), Shell::tick);

                #[cfg(target_arch = "wasm32")]
                callback(Err(core::image::Error::Unsupported));
            }
        }
    }

    pub fn load_image(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        handle: &core::image::Handle,
    ) -> Result<core::image::Allocation, core::image::Error> {
        use crate::image::raster::Memory;

        self.receive();

        #[cfg(not(target_arch = "wasm32"))]
        let callbacks = self.raster.requests.take_callbacks(handle.id());
        #[cfg(target_arch = "wasm32")]
        let callbacks = CallbackList::new();

        let result = match self.raster.resident.get_or_load(handle) {
            Memory::Device {
                entry, allocation, ..
            } => {
                if let Some(allocation) = allocation
                    .as_ref()
                    .and_then(core::image::Allocation::upgrade)
                {
                    Ok(allocation)
                } else {
                    #[allow(unsafe_code)]
                    let fresh = unsafe { core::image::allocate(handle, entry.size()) };
                    *allocation = Some(fresh.downgrade());
                    Ok(fresh)
                }
            }
            Memory::Error(error) => Err(error.clone()),
            Memory::Host(image) => {
                let size = Size::new(image.width(), image.height());
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("taim.image.blocking_upload"),
                });
                let entry = self.atlas.upload(
                    device,
                    &mut encoder,
                    &mut self.raster.blocking_uploads,
                    size.width,
                    size.height,
                    image,
                );

                self.raster.blocking_uploads.finish();
                let submission = queue.submit([encoder.finish()]);
                self.raster.blocking_uploads.recall();

                if let Some(entry) = entry {
                    let _ = device.poll(wgpu::PollType::Wait {
                        submission_index: Some(submission),
                        timeout: None,
                    });

                    #[allow(unsafe_code)]
                    let allocation = unsafe { core::image::allocate(handle, size) };
                    self.raster.resident.insert(
                        handle,
                        Memory::Device {
                            entry,
                            bind_group: None,
                            allocation: Some(allocation.downgrade()),
                        },
                    );
                    Ok(allocation)
                } else {
                    Err(core::image::Error::OutOfMemory)
                }
            }
        };

        for callback in callbacks {
            callback(result.clone());
        }
        result
    }

    pub fn measure_image(&mut self, handle: &core::image::Handle) -> Option<Size<u32>> {
        self.receive();
        self.request_decode(handle, Shell::invalidate_layout);
        self.raster
            .resident
            .get_mut(handle)
            .map(|memory| memory.dimensions())
    }

    pub fn upload_raster(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        handle: &core::image::Handle,
    ) -> Option<(&atlas::Entry, &Arc<wgpu::BindGroup>)> {
        use crate::image::raster::Memory;

        const INLINE_UPLOAD_LIMIT: usize = 2 * 1024 * 1024;

        self.receive();
        self.request_decode(handle, Shell::request_redraw);

        if matches!(
            self.raster.resident.get_mut(handle),
            Some(Memory::Device { .. })
        ) {
            return self.device_image(handle);
        }

        let image = self
            .raster
            .resident
            .get_mut(handle)
            .and_then(|memory| memory.host())?;

        if image.len() <= INLINE_UPLOAD_LIMIT || cfg!(target_arch = "wasm32") {
            let entry =
                self.atlas
                    .upload(device, encoder, belt, image.width(), image.height(), &image)?;
            self.raster.resident.insert(
                handle,
                Memory::Device {
                    entry,
                    bind_group: None,
                    allocation: None,
                },
            );
            return self.device_image(handle);
        }

        #[cfg(not(target_arch = "wasm32"))]
        if !self.raster.requests.touch(handle.id()) {
            self.queue_upload(
                handle,
                worker::Pixels {
                    width: image.width(),
                    height: image.height(),
                    rgba: image.into_raw(),
                },
                None,
                Shell::request_redraw,
            );
        }

        None
    }

    #[cfg(target_arch = "wasm32")]
    pub fn receive(&mut self) {}

    #[cfg(target_arch = "wasm32")]
    fn request_decode(&mut self, handle: &core::image::Handle, _wake: fn(&Shell)) {
        use crate::image::raster::Memory;

        if !self.raster.resident.contains(handle) {
            self.raster.resident.insert(handle, Memory::load(handle));
        }
    }

    fn device_image(
        &mut self,
        handle: &core::image::Handle,
    ) -> Option<(&atlas::Entry, &Arc<wgpu::BindGroup>)> {
        let crate::image::raster::Memory::Device {
            entry, bind_group, ..
        } = self.raster.resident.get_mut(handle)?
        else {
            return None;
        };

        Some((
            entry,
            bind_group.as_ref().unwrap_or(self.atlas.bind_group()),
        ))
    }
}

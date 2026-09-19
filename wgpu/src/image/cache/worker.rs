use crate::core::{Bytes, image};
use crate::graphics::Shell;
use crate::image::atlas::{self, Atlas};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;

const JOB_CAPACITY: usize = 8;
const COMPLETION_CAPACITY: usize = 16;
const UPLOAD_BELT_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Bytes,
}

pub enum Completion {
    Ready {
        ticket: u64,
        handle: image::Handle,
        entry: atlas::Entry,
        bind_group: Arc<wgpu::BindGroup>,
    },
    Failed {
        ticket: u64,
        handle: image::Handle,
        error: image::Error,
    },
}

enum Job {
    Decode {
        ticket: u64,
        handle: image::Handle,
        wake: fn(&Shell),
    },
    Upload {
        ticket: u64,
        handle: image::Handle,
        pixels: Pixels,
        wake: fn(&Shell),
    },
    Stop,
}

pub struct Worker {
    jobs: mpsc::SyncSender<Job>,
    completions: mpsc::Receiver<Completion>,
    stopping: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        backend: wgpu::Backend,
        texture_layout: wgpu::BindGroupLayout,
        shell: &Shell,
    ) -> Self {
        let (jobs, job_rx) = mpsc::sync_channel(JOB_CAPACITY);
        let (completion_tx, completions) = mpsc::sync_channel(COMPLETION_CAPACITY);
        let stopping = Arc::new(AtomicBool::new(false));
        let worker_stopping = stopping.clone();
        let mut engine = Engine {
            device: device.clone(),
            queue: queue.clone(),
            backend,
            texture_layout,
            shell: shell.clone(),
            upload_belt: wgpu::util::StagingBelt::new(device.clone(), UPLOAD_BELT_BYTES),
            completions: completion_tx,
        };
        let thread = thread::Builder::new()
            .name(String::from("taim-image-io"))
            .spawn(move || {
                while !worker_stopping.load(Ordering::Acquire) {
                    let Ok(job) = job_rx.recv() else {
                        break;
                    };

                    match job {
                        Job::Decode {
                            ticket,
                            handle,
                            wake,
                        } => engine.decode(ticket, handle, wake),
                        Job::Upload {
                            ticket,
                            handle,
                            pixels,
                            wake,
                        } => engine.upload(ticket, handle, pixels, wake),
                        Job::Stop => break,
                    }
                }
            })
            .expect("spawn TAIM image worker");

        Self {
            jobs,
            completions,
            stopping,
            thread: Some(thread),
        }
    }

    pub fn try_decode(&self, ticket: u64, handle: &image::Handle, wake: fn(&Shell)) -> bool {
        self.jobs
            .try_send(Job::Decode {
                ticket,
                handle: handle.clone(),
                wake,
            })
            .is_ok()
    }

    pub fn try_upload(
        &self,
        ticket: u64,
        handle: &image::Handle,
        pixels: &Pixels,
        wake: fn(&Shell),
    ) -> bool {
        self.jobs
            .try_send(Job::Upload {
                ticket,
                handle: handle.clone(),
                pixels: pixels.clone(),
                wake,
            })
            .is_ok()
    }

    pub fn try_recv(&self) -> Result<Completion, mpsc::TryRecvError> {
        self.completions.try_recv()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        let _ = self.jobs.try_send(Job::Stop);

        if let Some(handle) = self.thread.take() {
            // A bounded completion queue deliberately applies backpressure
            // during normal operation. Drain it while joining so a worker
            // already publishing its final result can never deadlock cache
            // shutdown when the UI no longer calls `receive`.
            while !handle.is_finished() {
                while self.completions.try_recv().is_ok() {}
                thread::yield_now();
            }
            let _ = handle.join();
        }
    }
}

struct Engine {
    device: wgpu::Device,
    queue: wgpu::Queue,
    backend: wgpu::Backend,
    texture_layout: wgpu::BindGroupLayout,
    shell: Shell,
    upload_belt: wgpu::util::StagingBelt,
    completions: mpsc::SyncSender<Completion>,
}

impl Engine {
    fn decode(&mut self, ticket: u64, handle: image::Handle, wake: fn(&Shell)) {
        match crate::graphics::image::load(&handle) {
            Ok(image) => self.upload(
                ticket,
                handle,
                Pixels {
                    width: image.width(),
                    height: image.height(),
                    rgba: image.into_raw(),
                },
                wake,
            ),
            Err(error) => self.fail(ticket, handle, error, wake),
        }
    }

    fn upload(&mut self, ticket: u64, handle: image::Handle, pixels: Pixels, wake: fn(&Shell)) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("taim.image.worker.upload"),
            });
        let mut atlas = Atlas::with_size(
            &self.device,
            self.backend,
            self.texture_layout.clone(),
            pixels.width.max(pixels.height),
        );
        let Some(entry) = atlas.upload(
            &self.device,
            &mut encoder,
            &mut self.upload_belt,
            pixels.width,
            pixels.height,
            &pixels.rgba,
        ) else {
            self.fail(ticket, handle, image::Error::OutOfMemory, wake);
            return;
        };

        self.upload_belt.finish();
        let submission = self.queue.submit([encoder.finish()]);
        self.upload_belt.recall();
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: None,
        });
        let _ = self.completions.send(Completion::Ready {
            ticket,
            handle,
            entry,
            bind_group: atlas.bind_group().clone(),
        });
        wake(&self.shell);
    }

    fn fail(&self, ticket: u64, handle: image::Handle, error: image::Error, wake: fn(&Shell)) {
        let _ = self.completions.send(Completion::Failed {
            ticket,
            handle,
            error,
        });
        wake(&self.shell);
    }
}

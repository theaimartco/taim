use crate::core::image;
use crate::core::Size;
use crate::graphics::image::Buffer;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

/// A request to decode an image.
#[derive(Debug, Clone)]
pub struct DecodeRequest {
    /// The handle of the image to decode.
    pub handle: image::Handle,
    /// The target size of the decoded image.
    pub target_size: Option<Size<u32>>,
}

/// The result of an image decode operation.
pub struct DecodeResult {
    /// The handle of the decoded image.
    pub handle: image::Handle,
    /// The decoded image buffer, or an error.
    pub result: Result<Buffer, image::Error>,
}

/// A handle to a submitted decode request that allows cancellation.
pub struct DecodeHandle {
    is_cancelled: Arc<AtomicBool>,
}

impl DecodeHandle {
    /// Cancels the ongoing or pending decode request.
    pub fn cancel(&self) {
        self.is_cancelled.store(true, Ordering::Release);
    }
}

impl Drop for DecodeHandle {
    fn drop(&mut self) {
        self.cancel();
    }
}

struct PendingRequest {
    request: DecodeRequest,
    is_cancelled: Arc<AtomicBool>,
}

/// A background image decoder with a bounded queue and thread pool.
pub struct Decoder {
    sender: mpsc::SyncSender<PendingRequest>,
    result_receiver: mpsc::Receiver<DecodeResult>,
    result_sender: mpsc::Sender<DecodeResult>,
    workers: Vec<thread::JoinHandle<()>>,
}

impl Decoder {
    /// Creates a new `Decoder` with the given thread count and queue size.
    pub fn new(threads: usize, queue_size: usize) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<PendingRequest>(queue_size);
        let receiver = Arc::new(Mutex::new(receiver));
        let (result_sender, result_receiver) = mpsc::channel();

        let mut workers = Vec::with_capacity(threads);
        for i in 0..threads {
            let rx = receiver.clone();
            let tx = result_sender.clone();
            
            workers.push(thread::Builder::new()
                .name(format!("aura-decode-worker-{}", i))
                .spawn(move || {
                    loop {
                        let pending = {
                            let lock = rx.lock().unwrap();
                            match lock.recv() {
                                Ok(req) => req,
                                Err(_) => break, // Channel disconnected, terminate thread
                            }
                        };

                        if pending.is_cancelled.load(Ordering::Acquire) {
                            continue;
                        }

                        let result = decode_image(&pending.request.handle, pending.request.target_size);

                        // Only send if it hasn't been cancelled during decoding
                        if !pending.is_cancelled.load(Ordering::Acquire) {
                            let _ = tx.send(DecodeResult {
                                handle: pending.request.handle,
                                result,
                            });
                        }
                    }
                })
                .expect("Failed to spawn decode worker thread"));
        }

        Self {
            sender,
            result_receiver,
            result_sender,
            workers,
        }
    }

    /// Submits a request to decode an image in the background.
    /// 
    /// Returns a `DecodeHandle` which can be used to cancel the request.
    /// If the bounded queue is full, this will drop the request and return `None`.
    pub fn submit(&self, request: DecodeRequest) -> Option<DecodeHandle> {
        let is_cancelled = Arc::new(AtomicBool::new(false));
        let pending = PendingRequest {
            request,
            is_cancelled: is_cancelled.clone(),
        };

        match self.sender.try_send(pending) {
            Ok(_) => Some(DecodeHandle { is_cancelled }),
            Err(mpsc::TrySendError::Full(_)) => {
                log::warn!("Decoder queue is full, dropping request");
                None
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                log::error!("Decoder thread pool has been disconnected");
                None
            }
        }
    }

    /// Polls for completed decode results.
    pub fn poll(&self) -> impl Iterator<Item = DecodeResult> + '_ {
        std::iter::from_fn(move || self.result_receiver.try_recv().ok())
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        // Since we are using an mpsc channel, the rx end is held by the worker threads.
        // We drop the sender end here, which will cause the worker threads to break out
        // of their loops on their next `recv` call because the channel is disconnected.
        // However, self.sender is dropped automatically here.
    }
}

fn decode_image(handle: &image::Handle, target_size: Option<Size<u32>>) -> Result<Buffer, image::Error> {
    // 1. First load the image using the existing synchronous loader
    let mut buffer = crate::graphics::image::load(handle)?;

    // 2. If a target size is requested, scale it down to save memory and atlas space
    if let Some(target) = target_size {
        let current_width = buffer.width();
        let current_height = buffer.height();

        if target.width > 0 && target.height > 0 && (current_width > target.width || current_height > target.height) {
            log::trace!(
                "Resizing image from {}x{} to {}x{}",
                current_width,
                current_height,
                target.width,
                target.height
            );
            
            let dynamic_image = ::image::DynamicImage::ImageRgba8(
                ::image::ImageBuffer::from_raw(current_width, current_height, buffer.to_vec())
                    .expect("Buffer size should match dimensions"),
            );
            let resized = dynamic_image.resize(
                target.width, 
                target.height, 
                ::image::imageops::FilterType::Triangle
            );
            
            let rgba = resized.into_rgba8();
            buffer = Buffer::from_raw(rgba.width(), rgba.height(), crate::core::Bytes::from(rgba.into_raw()))
                .expect("Failed to create Buffer from raw resized RGBA data");
        }
    }

    Ok(buffer)
}

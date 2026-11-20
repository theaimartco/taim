//! Camera permission through AVFoundation, the same on iOS and macOS.

use std::sync::Mutex;

use futures::channel::oneshot;
use objc2::msg_send;
use objc2::runtime::{AnyObject, Bool};

use super::ffi;
use super::types::{PermissionState, ServiceError};

#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {
    static AVMediaTypeVideo: *const AnyObject;
}

fn capture_permission_now() -> PermissionState {
    let Some(class) = ffi::class("AVCaptureDevice") else {
        return PermissionState::Restricted;
    };
    // SAFETY: a thread-safe class method with AVFoundation's media type.
    PermissionState::from_capture(unsafe {
        msg_send![class, authorizationStatusForMediaType: AVMediaTypeVideo]
    })
}

/// The camera permission, without asking.
pub fn camera_permission() -> PermissionState {
    capture_permission_now()
}

/// Asks for camera permission if the user has not been asked yet.
pub async fn request_camera_permission() -> Result<PermissionState, ServiceError> {
    let current = capture_permission_now();
    if current != PermissionState::NotDetermined {
        return Ok(current);
    }
    let class = ffi::class("AVCaptureDevice").ok_or(ServiceError::Unsupported)?;
    let (sender, receiver) = oneshot::channel();
    {
        let sender = Mutex::new(Some(sender));
        let block = block2::RcBlock::new(move |granted: Bool| {
            if let Some(sender) = sender.lock().ok().and_then(|mut slot| slot.take()) {
                let _ = sender.send(granted.as_bool());
            }
        });
        // SAFETY: thread-safe; AVFoundation copies the block and calls it once.
        unsafe {
            let _: () = msg_send![
                class,
                requestAccessForMediaType: AVMediaTypeVideo,
                completionHandler: &*block
            ];
        }
    }
    let granted = receiver.await.map_err(|_| ServiceError::Cancelled)?;
    Ok(if granted {
        PermissionState::Granted
    } else {
        PermissionState::Denied
    })
}

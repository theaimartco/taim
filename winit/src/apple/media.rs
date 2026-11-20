//! Photos and videos: the system photo picker (no permission needed) and
//! camera capture.
//!
//! Picked and captured files are copied into `Library/Caches/taim-media`
//! and belong to the app from then on. Camera capture needs
//! `NSCameraUsageDescription` in the Info.plist, and video also
//! `NSMicrophoneUsageDescription`.

use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::{Arc, Mutex};

use futures::channel::oneshot;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{ClassType, DeclaredClass, declare_class, msg_send, msg_send_id, mutability};
use objc2_foundation::MainThreadMarker;

use super::ffi;
use super::types::{
    CaptureKind, MediaFile, MediaKind, MediaPicker, PermissionState, ServiceError, media_types,
    type_for_extension,
};

#[link(name = "PhotosUI", kind = "framework")]
unsafe extern "C" {}

#[link(name = "UIKit", kind = "framework")]
unsafe extern "C" {
    static UIImagePickerControllerOriginalImage: *const AnyObject;
    static UIImagePickerControllerMediaURL: *const AnyObject;
    fn UIImageJPEGRepresentation(image: *const AnyObject, quality: f64) -> *mut AnyObject;
}

type Reply = oneshot::Sender<Result<Vec<MediaFile>, ServiceError>>;

/// The picker or camera on screen, at most one at a time.
struct Active {
    reply: Reply,
    _delegate: Retained<MediaDelegate>,
    capture: Option<CaptureKind>,
}

thread_local! {
    static ACTIVE: RefCell<Option<Active>> = const { RefCell::new(None) };
}

fn finish(result: Result<Vec<MediaFile>, ServiceError>) {
    let active = ACTIVE.with(|slot| slot.borrow_mut().take());
    if let Some(active) = active {
        let _ = active.reply.send(result);
    }
}

fn capture_kind() -> Option<CaptureKind> {
    ACTIVE.with(|slot| slot.borrow().as_ref().and_then(|active| active.capture))
}

/// Collects the files of a multi-item selection, which arrive one by one on
/// background queues, and answers once all have.
struct Collector {
    files: Vec<Option<MediaFile>>,
    remaining: usize,
    failed: bool,
}

fn deliver(collector: &Arc<Mutex<Collector>>) {
    let done = {
        let collector = collector
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        collector.remaining == 0
    };
    if done {
        let collector = collector.clone();
        ffi::on_main(move |_mtm| {
            let mut collector = collector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let files: Vec<MediaFile> = collector.files.drain(..).flatten().collect();
            if files.is_empty() && collector.failed {
                finish(Err(ServiceError::Failed));
            } else {
                finish(Ok(files));
            }
        });
    }
}

/// Loads one picked item's file through its `NSItemProvider`.
///
/// # Safety
/// `result` must point to a `PHPickerResult`; called on the main thread.
unsafe fn load(
    result: *const AnyObject,
    index: usize,
    kinds: &'static [&'static str],
    collector: &Arc<Mutex<Collector>>,
) {
    // SAFETY: PHPickerResult.itemProvider and NSItemProvider's documented API.
    unsafe {
        let provider: *const AnyObject = msg_send![result, itemProvider];
        let chosen = (!provider.is_null())
            .then(|| {
                kinds.iter().copied().find(|kind| {
                    msg_send![provider, hasItemConformingToTypeIdentifier: &*ffi::string(kind)]
                })
            })
            .flatten();
        let Some(kind) = chosen else {
            let mut state = collector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.remaining -= 1;
            drop(state);
            deliver(collector);
            return;
        };
        let collector = collector.clone();
        let block = block2::RcBlock::new(move |url: *mut AnyObject, error: *mut AnyObject| {
            // The file is deleted when this handler returns: copy it now.
            let file = if error.is_null() {
                ffi::file_path(url).and_then(|path| {
                    let type_identifier = type_for_extension(
                        path.extension().and_then(|extension| extension.to_str()),
                        kind,
                    );
                    ffi::adopt_file(&path, type_identifier).ok()
                })
            } else {
                None
            };
            let mut state = collector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if file.is_none() {
                state.failed = true;
            }
            if let Some(slot) = state.files.get_mut(index) {
                *slot = file;
            }
            state.remaining -= 1;
            drop(state);
            deliver(&collector);
        });
        let _: *mut AnyObject = msg_send![
            provider,
            loadFileRepresentationForTypeIdentifier: &*ffi::string(kind),
            completionHandler: &*block
        ];
    }
}

declare_class!(
    struct MediaDelegate;

    // SAFETY: NSObject has no subclassing requirements; no state, no Drop.
    unsafe impl ClassType for MediaDelegate {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "TaimMediaDelegate";
    }

    impl DeclaredClass for MediaDelegate {}

    unsafe impl MediaDelegate {
        #[method(picker:didFinishPicking:)]
        fn did_finish_picking(&self, picker: *mut AnyObject, results: *mut AnyObject) {
            // SAFETY: PhotosUI calls this on the main thread with the picker
            // and an NSArray of PHPickerResult.
            unsafe {
                if !picker.is_null() {
                    ffi::dismiss(&*picker);
                }
                let count: usize = if results.is_null() { 0 } else { msg_send![results, count] };
                if count == 0 {
                    // Cancelled: an empty selection.
                    finish(Ok(Vec::new()));
                    return;
                }
                let kinds = PICK_KINDS.with(|kinds| *kinds.borrow());
                let collector = Arc::new(Mutex::new(Collector {
                    files: vec![None; count],
                    remaining: count,
                    failed: false,
                }));
                for index in 0..count {
                    let result: *const AnyObject = msg_send![results, objectAtIndex: index];
                    load(result, index, kinds, &collector);
                }
            }
        }

        #[method(imagePickerController:didFinishPickingMediaWithInfo:)]
        fn did_capture(&self, picker: *mut AnyObject, info: *mut AnyObject) {
            // SAFETY: UIKit calls this on the main thread with the camera
            // controller and its info dictionary.
            let file = unsafe {
                if !picker.is_null() {
                    ffi::dismiss(&*picker);
                }
                captured(info, capture_kind().unwrap_or_default())
            };
            finish(file.map(|file| vec![file]));
        }

        #[method(imagePickerControllerDidCancel:)]
        fn did_cancel(&self, picker: *mut AnyObject) {
            // SAFETY: as above.
            unsafe {
                if !picker.is_null() {
                    ffi::dismiss(&*picker);
                }
            }
            finish(Ok(Vec::new()));
        }
    }
);

thread_local! {
    static PICK_KINDS: RefCell<&'static [&'static str]> = const { RefCell::new(&[]) };
}

/// Saves what the camera produced.
///
/// # Safety
/// `info` must be the camera's info dictionary; on the main thread.
unsafe fn captured(info: *const AnyObject, kind: CaptureKind) -> Result<MediaFile, ServiceError> {
    if info.is_null() {
        return Err(ServiceError::Failed);
    }
    // SAFETY: documented UIImagePickerController info keys and UIKit's JPEG
    // encoder; the NSData is autoreleased and read within this call.
    unsafe {
        match kind {
            CaptureKind::Photo => {
                let image: *const AnyObject =
                    msg_send![info, objectForKey: UIImagePickerControllerOriginalImage];
                if image.is_null() {
                    return Err(ServiceError::Failed);
                }
                let data = UIImageJPEGRepresentation(image, 0.9);
                if data.is_null() {
                    return Err(ServiceError::Failed);
                }
                let length: usize = msg_send![data, length];
                let bytes: *const c_void = msg_send![data, bytes];
                let bytes = bytes.cast::<u8>();
                if bytes.is_null() || length == 0 {
                    return Err(ServiceError::Failed);
                }
                let path = ffi::media_destination("jpg").map_err(|_| ServiceError::Failed)?;
                std::fs::write(&path, std::slice::from_raw_parts(bytes, length))
                    .map_err(|_| ServiceError::Failed)?;
                Ok(MediaFile {
                    path,
                    type_identifier: "public.jpeg".to_owned(),
                    size: length as u64,
                })
            }
            CaptureKind::Video => {
                let url: *const AnyObject =
                    msg_send![info, objectForKey: UIImagePickerControllerMediaURL];
                let path = ffi::file_path(url).ok_or(ServiceError::Failed)?;
                ffi::adopt_file(&path, "com.apple.quicktime-movie")
            }
        }
    }
}

/// Claims the single picker slot, or fails with [`ServiceError::Busy`].
fn claim(reply: Reply, capture: Option<CaptureKind>) -> Option<Retained<MediaDelegate>> {
    ACTIVE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_some() {
            let _ = reply.send(Err(ServiceError::Busy));
            return None;
        }
        // SAFETY: a plain NSObject subclass, kept alive in ACTIVE because
        // pickers hold their delegates weakly.
        let delegate: Retained<MediaDelegate> =
            unsafe { msg_send_id![MediaDelegate::alloc(), init] };
        *slot = Some(Active {
            reply,
            _delegate: delegate.clone(),
            capture,
        });
        Some(delegate)
    })
}

/// Shows the system photo picker. Returns the chosen files, or an empty
/// list if the user cancelled. iOS 14 and later.
pub async fn pick_media(picker: MediaPicker) -> Result<Vec<MediaFile>, ServiceError> {
    picker.validate()?;
    let (sender, receiver) = oneshot::channel();
    ffi::on_main(move |mtm| show_picker(mtm, picker, sender));
    receiver.await.map_err(|_| ServiceError::Cancelled)?
}

fn show_picker(mtm: MainThreadMarker, picker: MediaPicker, reply: Reply) {
    let (Some(configuration_class), Some(controller_class), Some(filter_class)) = (
        ffi::class("PHPickerConfiguration"),
        ffi::class("PHPickerViewController"),
        ffi::class("PHPickerFilter"),
    ) else {
        let _ = reply.send(Err(ServiceError::Unsupported));
        return;
    };
    let Some(delegate) = claim(reply, None) else {
        return;
    };
    PICK_KINDS.with(|kinds| *kinds.borrow_mut() = media_types(picker.kind));
    // SAFETY: PhotosUI objects created and presented on the main thread.
    let presented = unsafe {
        let configuration: Retained<AnyObject> = msg_send_id![configuration_class, new];
        let _: () = msg_send![&configuration, setSelectionLimit: isize::from(picker.limit as i16)];
        let filter: Option<Retained<AnyObject>> = match picker.kind {
            MediaKind::Images => msg_send_id![filter_class, imagesFilter],
            MediaKind::Videos => msg_send_id![filter_class, videosFilter],
            MediaKind::Any => None,
        };
        if let Some(filter) = &filter {
            let _: () = msg_send![&configuration, setFilter: &**filter];
        }
        let controller: Retained<AnyObject> = msg_send_id![
            msg_send_id![controller_class, alloc],
            initWithConfiguration: &*configuration
        ];
        let _: () = msg_send![&controller, setDelegate: &*delegate];
        ffi::present(mtm, &controller)
    };
    if !presented {
        finish(Err(ServiceError::Unavailable));
    }
}

/// Opens the camera to take one photo or video. Returns the file, or `None`
/// if the user cancelled. [`ServiceError::Unsupported`] without a camera
/// (including the Simulator); [`ServiceError::Denied`] without permission.
pub async fn capture_media(kind: CaptureKind) -> Result<Option<MediaFile>, ServiceError> {
    let (sender, receiver) = oneshot::channel();
    ffi::on_main(move |mtm| show_camera(mtm, kind, sender));
    let mut files = receiver.await.map_err(|_| ServiceError::Cancelled)??;
    Ok(files.pop())
}

fn show_camera(mtm: MainThreadMarker, kind: CaptureKind, reply: Reply) {
    let class = objc2::class!(UIImagePickerController);
    // SAFETY: class methods on UIImagePickerController; source type 1 is
    // the camera.
    let available: bool = unsafe { msg_send![class, isSourceTypeAvailable: 1isize] };
    if !available {
        let _ = reply.send(Err(ServiceError::Unsupported));
        return;
    }
    if matches!(
        super::camera::camera_permission(),
        PermissionState::Denied | PermissionState::Restricted
    ) {
        let _ = reply.send(Err(ServiceError::Denied));
        return;
    }
    let Some(delegate) = claim(reply, Some(kind)) else {
        return;
    };
    // SAFETY: a camera controller created and presented on the main thread.
    let presented = unsafe {
        let controller: Retained<AnyObject> = msg_send_id![class, new];
        let _: () = msg_send![&controller, setSourceType: 1isize];
        let types: Retained<AnyObject> = msg_send_id![
            objc2::class!(NSArray),
            arrayWithObject: &*ffi::string(match kind {
                CaptureKind::Photo => "public.image",
                CaptureKind::Video => "public.movie",
            })
        ];
        let _: () = msg_send![&controller, setMediaTypes: &*types];
        let _: () = msg_send![&controller, setDelegate: &*delegate];
        ffi::present(mtm, &controller)
    };
    if !presented {
        finish(Err(ServiceError::Unavailable));
    }
}

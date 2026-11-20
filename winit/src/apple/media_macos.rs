//! Choosing photos and videos on a Mac: the system open panel, filtered to
//! images or movies. A sandboxed app needs the
//! `com.apple.security.files.user-selected.read-only` entitlement.
//!
//! Chosen files are copied into `~/Library/Caches/<bundle id>/taim-media`
//! (inside the container when sandboxed) and belong to the app.

use std::cell::Cell;
use std::sync::Mutex;

use futures::channel::oneshot;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{msg_send, msg_send_id};

use super::ffi;
use super::types::{
    CaptureKind, MediaFile, MediaKind, MediaPicker, ServiceError, type_for_extension,
};

thread_local! {
    /// Whether a panel from this module is open.
    static OPEN: Cell<bool> = const { Cell::new(false) };
}

type Reply = oneshot::Sender<Result<Vec<MediaFile>, ServiceError>>;

/// Shows the open panel. Returns the chosen files (at most `picker.limit`),
/// or an empty list if the user cancelled.
pub async fn pick_media(picker: MediaPicker) -> Result<Vec<MediaFile>, ServiceError> {
    picker.validate()?;
    let (sender, receiver) = oneshot::channel();
    ffi::on_main(move |_mtm| show_panel(picker, sender));
    let paths = receiver.await.map_err(|_| ServiceError::Cancelled)??;
    Ok(paths)
}

/// The type identifiers the panel accepts.
fn allowed(kind: MediaKind) -> &'static [&'static str] {
    match kind {
        MediaKind::Images => &["public.image"],
        MediaKind::Videos => &["public.movie"],
        MediaKind::Any => &["public.image", "public.movie"],
    }
}

fn show_panel(picker: MediaPicker, reply: Reply) {
    if OPEN.with(Cell::get) {
        let _ = reply.send(Err(ServiceError::Busy));
        return;
    }
    OPEN.with(|open| open.set(true));
    let reply = Mutex::new(Some(reply));
    // SAFETY: an AppKit open panel configured and shown on the main thread;
    // the completion block runs there too and reads the panel's URLs.
    unsafe {
        let panel: Retained<AnyObject> = msg_send_id![objc2::class!(NSOpenPanel), openPanel];
        let _: () = msg_send![&panel, setCanChooseFiles: true];
        let _: () = msg_send![&panel, setCanChooseDirectories: false];
        let _: () = msg_send![&panel, setAllowsMultipleSelection: picker.limit > 1];
        let types: Retained<AnyObject> = msg_send_id![objc2::class!(NSMutableArray), array];
        for identifier in allowed(picker.kind) {
            let _: () = msg_send![&types, addObject: &*ffi::string(identifier)];
        }
        // Takes uniform type identifiers as well as extensions.
        let _: () = msg_send![&panel, setAllowedFileTypes: &*types];
        let held = ffi::Shared(panel.clone());
        let kind = picker.kind;
        let limit = usize::from(picker.limit);
        let block = block2::RcBlock::new(move |response: isize| {
            OPEN.with(|open| open.set(false));
            let Some(reply) = reply.lock().ok().and_then(|mut slot| slot.take()) else {
                return;
            };
            // NSModalResponseOK
            if response != 1 {
                let _ = reply.send(Ok(Vec::new()));
                return;
            }
            let urls: *const AnyObject = msg_send![&held.0, URLs];
            let count: usize = if urls.is_null() {
                0
            } else {
                msg_send![urls, count]
            };
            let paths: Vec<std::path::PathBuf> = (0..count.min(limit))
                .filter_map(|index| {
                    let url: *const AnyObject = msg_send![urls, objectAtIndex: index];
                    ffi::file_path(url)
                })
                .collect();
            // Copying a video can take a while: not on the main thread.
            let fallback = match kind {
                MediaKind::Images => "public.image",
                MediaKind::Videos => "public.movie",
                MediaKind::Any => "public.data",
            };
            let copier = std::thread::Builder::new()
                .name("taim-media-copy".into())
                .spawn(move || {
                    let mut files = Vec::with_capacity(paths.len());
                    for path in paths {
                        let identifier = type_for_extension(
                            path.extension().and_then(|extension| extension.to_str()),
                            fallback,
                        );
                        match ffi::adopt_file(&path, identifier) {
                            Ok(file) => files.push(file),
                            Err(error) => {
                                let _ = reply.send(Err(error));
                                return;
                            }
                        }
                    }
                    let _ = reply.send(Ok(files));
                });
            if copier.is_err() {
                log::error!("could not start copying the chosen media");
            }
        });
        let _: () = msg_send![&panel, beginWithCompletionHandler: &*block];
    }
}

/// Camera capture has no system UI on macOS: always
/// [`ServiceError::Unsupported`]. Camera permission is available through
/// [`camera_permission`](super::camera_permission).
#[allow(clippy::unused_async)]
pub async fn capture_media(_kind: CaptureKind) -> Result<Option<MediaFile>, ServiceError> {
    Err(ServiceError::Unsupported)
}

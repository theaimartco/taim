//! Main-thread dispatch and small Objective-C helpers the iOS services share.
//!
//! TAIM runs background tasks on their own threads, while UIKit, Core
//! Location and the presented pickers belong to the main thread. Services
//! therefore post their UIKit work to the main dispatch queue and complete
//! through channels. Never block the main thread waiting for one of them:
//! the reply needs that thread.

use std::ffi::c_void;
use std::time::Duration;

use objc2::msg_send;
#[cfg(target_os = "ios")]
use objc2::msg_send_id;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_foundation::{MainThreadMarker, NSString};

#[repr(C)]
pub(crate) struct DispatchQueue {
    _private: [u8; 0],
}

unsafe extern "C" {
    static _dispatch_main_q: DispatchQueue;
    fn dispatch_async_f(
        queue: *const DispatchQueue,
        context: *mut c_void,
        work: unsafe extern "C" fn(*mut c_void),
    );
    fn dispatch_after_f(
        when: u64,
        queue: *const DispatchQueue,
        context: *mut c_void,
        work: unsafe extern "C" fn(*mut c_void),
    );
    fn dispatch_time(when: u64, delta: i64) -> u64;
}

type Job = Box<dyn FnOnce(MainThreadMarker) + Send>;

unsafe extern "C" fn run_job(context: *mut c_void) {
    // SAFETY: `post` leaked exactly this box for exactly one call.
    let job = unsafe { Box::from_raw(context.cast::<Job>()) };
    // SAFETY: the main dispatch queue only runs on the main thread.
    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    // A panic must not unwind into libdispatch.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        objc2::rc::autoreleasepool(|_| job(mtm));
    }));
    if result.is_err() {
        log::error!("an Apple main-thread service job panicked");
        eprintln!("TAIM: an Apple main-thread service job panicked");
    }
}

fn post(delay: Option<Duration>, job: Job) {
    let context = Box::into_raw(Box::new(job)).cast::<c_void>();
    // SAFETY: the main queue is a process-lifetime static; `run_job` takes
    // ownership of `context` exactly once.
    unsafe {
        let queue = &raw const _dispatch_main_q;
        match delay {
            None => dispatch_async_f(queue, context, run_job),
            Some(delay) => {
                let nanos = i64::try_from(delay.as_nanos()).unwrap_or(i64::MAX);
                dispatch_after_f(dispatch_time(0, nanos), queue, context, run_job);
            }
        }
    }
}

/// Runs `job` on the main thread soon, never synchronously: callers may hold
/// main-thread state borrowed while they post.
pub(crate) fn on_main(job: impl FnOnce(MainThreadMarker) + Send + 'static) {
    post(None, Box::new(job));
}

/// Runs `job` on the main thread after `delay`.
pub(crate) fn after(delay: Duration, job: impl FnOnce(MainThreadMarker) + Send + 'static) {
    post(Some(delay), Box::new(job));
}

/// A class that may be missing on older systems or unlinked frameworks.
pub(crate) fn class(name: &str) -> Option<&'static AnyClass> {
    AnyClass::get(name)
}

/// An `NSString`, as an object for dynamic message sends.
pub(crate) fn string(text: &str) -> Retained<NSString> {
    NSString::from_str(text)
}

/// Reads an `NSString` held as a plain object pointer.
///
/// # Safety
/// `object` must be null or point to an `NSString`.
pub(crate) unsafe fn read_string(object: *const AnyObject) -> Option<String> {
    if object.is_null() {
        return None;
    }
    // SAFETY: the caller promises an NSString; `UTF8String` is valid while
    // the string lives, which it does for this call.
    unsafe {
        let utf8: *const std::ffi::c_char = msg_send![object, UTF8String];
        if utf8.is_null() {
            return None;
        }
        Some(
            std::ffi::CStr::from_ptr(utf8)
                .to_string_lossy()
                .into_owned(),
        )
    }
}

/// The `code` of an `NSError`, if there is one.
///
/// # Safety
/// `error` must be null or point to an `NSError`.
pub(crate) unsafe fn error_code(error: *const AnyObject) -> Option<isize> {
    // SAFETY: `code` is an NSError property.
    (!error.is_null()).then(|| unsafe { msg_send![error, code] })
}

/// The view controller system UI should be presented from: the key window's
/// root controller, or whatever it is already presenting.
#[cfg(target_os = "ios")]
pub(crate) fn presenter(_mtm: MainThreadMarker) -> Option<Retained<AnyObject>> {
    // SAFETY: on the main thread (`_mtm`), with UIKit's documented
    // selectors; every returned object is retained before use.
    unsafe {
        let application: Retained<AnyObject> =
            msg_send_id![objc2::class!(UIApplication), sharedApplication];
        let windows: Retained<AnyObject> = msg_send_id![&application, windows];
        let count: usize = msg_send![&windows, count];
        let mut chosen: Option<Retained<AnyObject>> = None;
        for index in 0..count {
            let window: Retained<AnyObject> = msg_send_id![&windows, objectAtIndex: index];
            let key: bool = msg_send![&window, isKeyWindow];
            if key || chosen.is_none() {
                chosen = Some(window);
            }
            if key {
                break;
            }
        }
        let window = chosen?;
        let mut controller: Retained<AnyObject> = msg_send_id![&window, rootViewController];
        loop {
            let presented: Option<Retained<AnyObject>> =
                msg_send_id![&controller, presentedViewController];
            match presented {
                Some(next) => {
                    let leaving: bool = msg_send![&next, isBeingDismissed];
                    if leaving {
                        break;
                    }
                    controller = next;
                }
                None => break,
            }
        }
        Some(controller)
    }
}

/// Presents `controller` from the current presenter. On iPad, popovers
/// such as the share sheet are anchored to the middle of the screen.
#[cfg(target_os = "ios")]
pub(crate) fn present(mtm: MainThreadMarker, controller: &AnyObject) -> bool {
    let Some(presenter) = presenter(mtm) else {
        return false;
    };
    // SAFETY: on the main thread with a live presenter and controller; the
    // popover controller is nil on iPhone and for non-popover styles.
    unsafe {
        let popover: Option<Retained<AnyObject>> =
            msg_send_id![controller, popoverPresentationController];
        if let Some(popover) = popover {
            let view: Retained<AnyObject> = msg_send_id![&presenter, view];
            let bounds: objc2_foundation::CGRect = msg_send![&view, bounds];
            let anchor = objc2_foundation::CGRect::new(
                objc2_foundation::CGPoint::new(
                    bounds.origin.x + bounds.size.width / 2.0,
                    bounds.origin.y + bounds.size.height / 2.0,
                ),
                objc2_foundation::CGSize::new(1.0, 1.0),
            );
            let _: () = msg_send![&popover, setSourceView: &*view];
            let _: () = msg_send![&popover, setSourceRect: anchor];
            let _: () = msg_send![&popover, setPermittedArrowDirections: 0usize];
        }
        let _: () = msg_send![
            &presenter,
            presentViewController: controller,
            animated: true,
            completion: None::<&block2::Block<dyn Fn()>>
        ];
    }
    true
}

/// Dismisses a controller this module presented.
#[cfg(target_os = "ios")]
pub(crate) fn dismiss(controller: &AnyObject) {
    // SAFETY: called on the main thread from delegate callbacks with the
    // controller UIKit passed in.
    unsafe {
        let _: () = msg_send![
            controller,
            dismissViewControllerAnimated: true,
            completion: None::<&block2::Block<dyn Fn()>>
        ];
    }
}

/// A fresh, unique file path inside `Library/Caches/taim-media`.
pub(crate) fn media_destination(extension: &str) -> Result<std::path::PathBuf, std::io::Error> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    let home = std::env::var_os("HOME")
        .ok_or_else(|| std::io::Error::other("no sandbox home directory"))?;
    // iOS apps always have their own container. A Mac app may not be
    // sandboxed, so its caches are namespaced like `platform::data_dir`.
    #[cfg(target_os = "ios")]
    let directory = std::path::PathBuf::from(home).join("Library/Caches/taim-media");
    #[cfg(target_os = "macos")]
    let directory = std::path::PathBuf::from(home)
        .join("Library/Caches")
        .join(app_namespace())
        .join("taim-media");
    std::fs::create_dir_all(&directory)?;
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let count = NEXT.fetch_add(1, Ordering::Relaxed);
    Ok(directory.join(format!("{millis}-{count}.{extension}")))
}

/// The bundle identifier, or the executable's name outside a bundle.
#[cfg(target_os = "macos")]
pub(crate) fn app_namespace() -> String {
    bundle_identifier()
        .or_else(|| {
            std::env::current_exe()
                .ok()?
                .file_stem()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "taim-app".to_owned())
}

/// The main bundle's identifier; `None` for an executable started outside
/// an app bundle, such as with `cargo run`.
pub(crate) fn bundle_identifier() -> Option<String> {
    // SAFETY: NSBundle's main bundle and its identifier are thread-safe reads.
    unsafe {
        let bundle: *const AnyObject = msg_send![objc2::class!(NSBundle), mainBundle];
        if bundle.is_null() {
            return None;
        }
        let identifier: *const AnyObject = msg_send![bundle, bundleIdentifier];
        read_string(identifier)
    }
}

/// Copies a file the system lends only for the duration of a callback.
pub(crate) fn adopt_file(
    source: &std::path::Path,
    type_identifier: &str,
) -> Result<super::MediaFile, super::ServiceError> {
    let extension =
        super::types::safe_extension(source.extension().and_then(|extension| extension.to_str()));
    let destination = media_destination(&extension).map_err(|_| super::ServiceError::Failed)?;
    let size = std::fs::copy(source, &destination).map_err(|_| super::ServiceError::Failed)?;
    Ok(super::MediaFile {
        path: destination,
        type_identifier: type_identifier.to_owned(),
        size,
    })
}

/// Reads an `NSURL`'s file-system path.
///
/// # Safety
/// `url` must be null or point to an `NSURL`.
pub(crate) unsafe fn file_path(url: *const AnyObject) -> Option<std::path::PathBuf> {
    if url.is_null() {
        return None;
    }
    // SAFETY: `path` is an NSString property of NSURL.
    unsafe {
        let path: *const AnyObject = msg_send![url, path];
        read_string(path).map(std::path::PathBuf::from)
    }
}

/// An object handle moved between threads. Used only for objects Apple
/// documents as thread-safe (`LAContext`), or that are only touched again
/// on the main thread.
pub(crate) struct Shared(pub Retained<AnyObject>);

// SAFETY: see the type's documentation; each use states which case it is.
unsafe impl Send for Shared {}
// SAFETY: as above.
unsafe impl Sync for Shared {}

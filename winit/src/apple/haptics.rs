//! Taptic Engine feedback and the share sheet.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{msg_send, msg_send_id};

use super::ffi;
use super::types::{Haptic, HapticGenerator};

/// Plays `kind` on the Taptic Engine. Returns at once; the feedback plays on
/// the main thread. Silent where the device or the user's settings have none.
pub fn haptic(kind: Haptic) {
    ffi::on_main(move |_mtm| {
        // SAFETY: on the main thread, as UIKit's feedback generators require;
        // each generator is created, prepared and fired here.
        unsafe {
            match kind.generator() {
                HapticGenerator::Impact(style) => {
                    let generator: Retained<AnyObject> = msg_send_id![
                        msg_send_id![objc2::class!(UIImpactFeedbackGenerator), alloc],
                        initWithStyle: style
                    ];
                    let _: () = msg_send![&generator, prepare];
                    let _: () = msg_send![&generator, impactOccurred];
                }
                HapticGenerator::Selection => {
                    let generator: Retained<AnyObject> =
                        msg_send_id![objc2::class!(UISelectionFeedbackGenerator), new];
                    let _: () = msg_send![&generator, prepare];
                    let _: () = msg_send![&generator, selectionChanged];
                }
                HapticGenerator::Notification(kind) => {
                    let generator: Retained<AnyObject> =
                        msg_send_id![objc2::class!(UINotificationFeedbackGenerator), new];
                    let _: () = msg_send![&generator, prepare];
                    let _: () = msg_send![&generator, notificationOccurred: kind];
                }
            }
        }
    });
}

/// Presents the system share sheet with `text`. Returns at once; the sheet
/// appears on the main thread over whatever the app is showing.
pub(crate) fn share_text(text: String) {
    ffi::on_main(move |mtm| {
        // SAFETY: on the main thread; the array and controller are retained
        // until presentation retains the controller.
        unsafe {
            let text = ffi::string(&text);
            let items: Retained<AnyObject> =
                msg_send_id![objc2::class!(NSArray), arrayWithObject: &*text];
            let controller: Option<Retained<AnyObject>> = msg_send_id![
                msg_send_id![objc2::class!(UIActivityViewController), alloc],
                initWithActivityItems: &*items,
                applicationActivities: std::ptr::null::<AnyObject>()
            ];
            let presented = controller.is_some_and(|controller| ffi::present(mtm, &controller));
            if !presented {
                log::warn!("the share sheet could not be presented: no window");
            }
        }
    });
}

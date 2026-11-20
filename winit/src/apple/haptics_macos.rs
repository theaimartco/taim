//! Force Touch trackpad feedback and the AppKit share picker.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{msg_send, msg_send_id};

use super::ffi;
use super::types::Haptic;

/// `NSHapticFeedbackPattern` for a [`Haptic`]. The trackpad has three
/// patterns; impacts map to the closest one.
fn pattern(kind: Haptic) -> isize {
    match kind {
        // alignment: snapping, selection
        Haptic::Selection | Haptic::Light | Haptic::Soft => 1,
        // levelChange: a firmer step
        Haptic::Heavy | Haptic::Rigid | Haptic::Warning | Haptic::Error => 2,
        // generic
        Haptic::Medium | Haptic::Success => 0,
    }
}

/// Plays `kind` on a Force Touch trackpad. macOS only plays it while a finger
/// rests on the trackpad, and not at all with a mouse; it never fails.
pub fn haptic(kind: Haptic) {
    ffi::on_main(move |_mtm| {
        // SAFETY: the default performer is an AppKit singleton used on the
        // main thread; performance time 1 is "now".
        unsafe {
            let performer: Option<Retained<AnyObject>> =
                msg_send_id![objc2::class!(NSHapticFeedbackManager), defaultPerformer];
            if let Some(performer) = performer {
                let _: () = msg_send![
                    &performer,
                    performFeedbackPattern: pattern(kind),
                    performanceTime: 1usize
                ];
            }
        }
    });
}

/// Shows the system share picker for `text`, anchored to the middle of the
/// key window. Returns at once; the picker appears on the main thread.
pub(crate) fn share_text(text: String) {
    ffi::on_main(move |_mtm| {
        // SAFETY: AppKit objects used on the main thread; the picker retains
        // itself while it is showing.
        unsafe {
            let application: Retained<AnyObject> =
                msg_send_id![objc2::class!(NSApplication), sharedApplication];
            let mut window: Option<Retained<AnyObject>> = msg_send_id![&application, keyWindow];
            if window.is_none() {
                window = msg_send_id![&application, mainWindow];
            }
            let Some(window) = window else {
                log::warn!("the share picker could not be shown: no window");
                return;
            };
            let view: Option<Retained<AnyObject>> = msg_send_id![&window, contentView];
            let Some(view) = view else {
                return;
            };
            let text = ffi::string(&text);
            let items: Retained<AnyObject> =
                msg_send_id![objc2::class!(NSArray), arrayWithObject: &*text];
            let picker: Retained<AnyObject> = msg_send_id![
                msg_send_id![objc2::class!(NSSharingServicePicker), alloc],
                initWithItems: &*items
            ];
            let bounds: objc2_foundation::CGRect = msg_send![&view, bounds];
            let anchor = objc2_foundation::CGRect::new(
                objc2_foundation::CGPoint::new(
                    bounds.origin.x + bounds.size.width / 2.0,
                    bounds.origin.y + bounds.size.height / 2.0,
                ),
                objc2_foundation::CGSize::new(1.0, 1.0),
            );
            // NSRectEdgeMinY
            let _: () = msg_send![
                &picker,
                showRelativeToRect: anchor,
                ofView: &*view,
                preferredEdge: 1usize
            ];
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_haptic_has_a_trackpad_pattern() {
        assert_eq!(pattern(Haptic::Selection), 1);
        assert_eq!(pattern(Haptic::Error), 2);
        assert_eq!(pattern(Haptic::Success), 0);
    }
}

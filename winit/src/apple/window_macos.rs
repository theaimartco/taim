//! What the shell reads from AppKit that winit does not report: Reduce Motion.

use objc2::msg_send;
use objc2::runtime::AnyObject;

/// Whether the user turned on System Settings › Accessibility › Display ›
/// Reduce motion.
pub(crate) fn reduce_motion() -> bool {
    // SAFETY: a thread-safe NSWorkspace property read.
    unsafe {
        let workspace: *const AnyObject = msg_send![objc2::class!(NSWorkspace), sharedWorkspace];
        !workspace.is_null() && msg_send![workspace, accessibilityDisplayShouldReduceMotion]
    }
}

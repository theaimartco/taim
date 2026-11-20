//! What the shell reads from UIKit about its window: the light or dark
//! appearance, Reduce Motion, and the safe area.

use objc2::msg_send;
use objc2::runtime::AnyObject;
use taim_core::{Point, Rectangle, Size, theme};
use winit::raw_window_handle::{HasWindowHandle as _, RawWindowHandle};

#[link(name = "UIKit", kind = "framework")]
unsafe extern "C" {
    fn UIAccessibilityIsReduceMotionEnabled() -> bool;
}

/// The window's appearance (`UIUserInterfaceStyle`), on the main thread.
pub(crate) fn appearance(window: &winit::window::Window) -> Option<theme::Mode> {
    let handle = window.window_handle().ok()?;
    let RawWindowHandle::UiKit(handle) = handle.as_raw() else {
        return None;
    };
    let view = handle.ui_view.as_ptr().cast::<AnyObject>();
    // SAFETY: winit hands out its live UIView; the shell calls this on the
    // main thread, where winit delivers events.
    let style: isize = unsafe {
        let traits: *const AnyObject = msg_send![view, traitCollection];
        if traits.is_null() {
            return None;
        }
        msg_send![traits, userInterfaceStyle]
    };
    match style {
        1 => Some(theme::Mode::Light),
        2 => Some(theme::Mode::Dark),
        _ => None,
    }
}

/// Whether the user turned on Settings › Accessibility › Motion › Reduce Motion.
pub(crate) fn reduce_motion() -> bool {
    // SAFETY: a UIKit C function without arguments.
    unsafe { UIAccessibilityIsReduceMotionEnabled() }
}

/// The part of the view outside the notch, status bar and home indicator.
pub(crate) fn safe_content(
    window: &winit::window::Window,
    physical: Size<u32>,
    scale: f32,
) -> Rectangle {
    let point = |position: winit::dpi::PhysicalPosition<i32>| Point::new(position.x, position.y);
    let size = |size: winit::dpi::PhysicalSize<u32>| Size::new(size.width, size.height);
    match (window.outer_position(), window.inner_position()) {
        (Ok(outer), Ok(inner)) => super::types::safe_area(
            physical,
            scale,
            (point(outer), size(window.outer_size())),
            (point(inner), size(window.inner_size())),
        ),
        _ => Rectangle::with_size(Size::new(
            physical.width as f32 / scale,
            physical.height as f32 / scale,
        )),
    }
}

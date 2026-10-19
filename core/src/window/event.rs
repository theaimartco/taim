use crate::time::Instant;
use crate::{Point, Size};

use std::path::PathBuf;

/// A window-related event.
#[derive(PartialEq, Clone, Debug)]
pub enum Event {
    /// A window was opened.
    Opened {
        /// The position of the opened window. This is relative to the top-left corner of the desktop
        /// the window is on, including virtual desktops. Refers to window's "outer" position,
        /// or the window area, in logical pixels.
        ///
        /// **Note**: Not available in Wayland.
        position: Option<Point>,
        /// The size of the created window. This is its "inner" size, or the size of the
        /// client area, in logical pixels.
        size: Size,
        /// The scale factor of the created window.
        scale_factor: f32,
    },

    /// A window was closed.
    Closed,

    /// A window was moved.
    Moved(Point),

    /// A window was resized.
    Resized(Size),

    /// A window changed its scale factor.
    Rescaled(f32),

    /// A window redraw was requested.
    ///
    /// The [`Instant`] contains the current time.
    RedrawRequested(Instant),

    /// The user has requested for the window to close.
    CloseRequested,

    /// A window was focused.
    Focused,

    /// A window was unfocused.
    Unfocused,

    /// A file is being hovered over the window.
    ///
    /// When the user hovers multiple files at once, this event will be emitted
    /// for each file separately.
    ///
    /// ## Platform-specific
    ///
    /// - **Wayland:** Not implemented.
    FileHovered(PathBuf),

    /// A file has been dropped into the window.
    ///
    /// When the user drops multiple files at once, this event will be emitted
    /// for each file separately.
    ///
    /// ## Platform-specific
    ///
    /// - **Wayland:** Not implemented.
    FileDropped(PathBuf),

    /// A file was hovered, but has exited the window.
    ///
    /// There will be a single `FilesHoveredLeft` event triggered even if
    /// multiple files were hovered.
    ///
    /// ## Platform-specific
    ///
    /// - **Wayland:** Not implemented.
    FilesHoveredLeft,
}


impl Event {
    /// Whether this newer event can replace the immediately preceding event
    /// inside a single TAIM window's pending input batch.
    ///
    /// Only transient geometry observations can be folded. Open/close,
    /// focus, file drop, compositor frame, and explicit close-request
    /// notifications are semantic boundaries and are never coalesced.
    /// Adjacent-only replacement is required so a move cannot cross a
    /// keyboard, pointer, or focus event that should observe the old state.
    #[must_use]
    pub fn supersedes_adjacent(&self, earlier: &Self) -> bool {
        matches!(
            (earlier, self),
            (Self::Moved(_), Self::Moved(_))
                | (Self::Resized(_), Self::Resized(_))
                | (Self::Rescaled(_), Self::Rescaled(_))
        )
    }
}

#[cfg(test)]
mod taim_window_event_tests {
    use super::*;

    #[test]
    fn newest_geometry_replaces_only_matching_adjacent_geometry() {
        let old = Event::Moved(Point::new(1.0, 2.0));
        let newest = Event::Moved(Point::new(3.0, 4.0));
        assert!(newest.supersedes_adjacent(&old));
        assert!(!Event::Resized(Size::new(50.0, 30.0)).supersedes_adjacent(&old));

        let old = Event::Resized(Size::new(10.0, 10.0));
        assert!(Event::Resized(Size::new(20.0, 20.0)).supersedes_adjacent(&old));
        assert!(Event::Rescaled(2.0).supersedes_adjacent(&Event::Rescaled(1.5)));
    }

    #[test]
    fn lifecycle_and_time_sensitive_notifications_are_never_replaced() {
        let now = Instant::now();
        for (previous, next) in [
            (Event::Focused, Event::Focused),
            (Event::Unfocused, Event::Unfocused),
            (Event::CloseRequested, Event::CloseRequested),
            (Event::RedrawRequested(now), Event::RedrawRequested(now)),
            (Event::Closed, Event::Closed),
        ] {
            assert!(!next.supersedes_adjacent(&previous));
        }
    }
}

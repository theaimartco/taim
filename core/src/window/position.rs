//! Screen-aware placement policy for TAIM windows.
//!
//! Coordinates coming from an application and coordinates relative to the
//! selected monitor have different origins. TAIM keeps those coordinate
//! spaces distinct until the platform shell performs DPI conversion.

use crate::{Point, Size};

/// Initial placement requested by an application.
#[derive(Debug, Clone, Copy, Default)]
pub enum Position {
    /// Let the native window manager select an initial location.
    #[default]
    Default,
    /// Center a window on the selected monitor without moving its upper-left
    /// corner off-screen when it is larger than that monitor.
    Centered,
    /// An explicit position in global logical desktop coordinates.
    Specific(Point),
    /// A callback returning monitor-relative logical coordinates.
    ///
    /// Arguments: requested logical window size, monitor logical size.
    SpecificWith(fn(Size, Size) -> Point),
}

/// A validated, unscaled placement decision. The platform shell converts a
/// monitor-relative value to physical desktop pixels using that monitor's
/// DPI scale factor and physical origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResolvedPosition {
    /// Absolute logical coordinates in the virtual desktop.
    Global(Point),
    /// Logical coordinates relative to the selected display's upper-left.
    MonitorRelative(Point),
}

impl Position {
    /// Resolve placement without relying on a live window or a native handle.
    ///
    /// Invalid, non-finite coordinates return `None`, allowing the compositor
    /// to choose a safe default. Explicit global positions do not require a
    /// monitor, while centered and callback-driven placement do.
    ///
    /// A callback may place a window outside its monitor deliberately, for
    /// example across a multi-monitor workspace; TAIM only rejects invalid
    /// numbers. Centering instead keeps the top-left visible on undersized
    /// displays.
    pub fn resolve(
        self,
        requested_size: Size,
        monitor_size: Option<Size>,
    ) -> Option<ResolvedPosition> {
        match self {
            Self::Default => None,
            Self::Specific(point) => {
                valid_point(point).then_some(ResolvedPosition::Global(point))
            }
            Self::Centered | Self::SpecificWith(_) => {
                let monitor = monitor_size.filter(|size| valid_extent(*size))?;
                if !valid_extent(requested_size) {
                    return None;
                }

                let local = match self {
                    Self::Centered => Point::new(
                        (monitor.width - requested_size.width).max(0.0) * 0.5,
                        (monitor.height - requested_size.height).max(0.0) * 0.5,
                    ),
                    Self::SpecificWith(callback) => callback(requested_size, monitor),
                    Self::Default | Self::Specific(_) => unreachable!(),
                };

                valid_point(local).then_some(ResolvedPosition::MonitorRelative(local))
            }
        }
    }
}

#[inline]
fn valid_point(point: Point) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

#[inline]
fn valid_extent(size: Size) -> bool {
    size.is_finite() && size.width > 0.0 && size.height > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prefer_bottom_right(window: Size, display: Size) -> Point {
        Point::new(display.width - window.width, display.height - window.height)
    }

    fn invalid_callback(_: Size, _: Size) -> Point {
        Point::new(f32::NAN, 0.0)
    }

    #[test]
    fn centered_placement_is_monitor_relative_and_deterministic() {
        let placement = Position::Centered.resolve(
            Size::new(800.0, 600.0),
            Some(Size::new(1920.0, 1080.0)),
        );
        assert_eq!(
            placement,
            Some(ResolvedPosition::MonitorRelative(Point::new(560.0, 240.0)))
        );
    }

    #[test]
    fn oversized_windows_start_at_monitor_origin() {
        assert_eq!(
            Position::Centered.resolve(
                Size::new(1800.0, 900.0),
                Some(Size::new(1280.0, 720.0))
            ),
            Some(ResolvedPosition::MonitorRelative(Point::ORIGIN))
        );
    }

    #[test]
    fn explicit_position_remains_global_without_a_monitor() {
        assert_eq!(
            Position::Specific(Point::new(-200.0, 12.0))
                .resolve(Size::new(100.0, 100.0), None),
            Some(ResolvedPosition::Global(Point::new(-200.0, 12.0)))
        );
        assert_eq!(
            Position::Centered.resolve(Size::new(100.0, 100.0), None),
            None
        );
        assert_eq!(
            Position::Default.resolve(Size::new(100.0, 100.0), None),
            None
        );
    }

    #[test]
    fn callback_runs_once_with_logical_dimensions() {
        assert_eq!(
            Position::SpecificWith(prefer_bottom_right).resolve(
                Size::new(400.0, 100.0),
                Some(Size::new(700.0, 400.0))
            ),
            Some(ResolvedPosition::MonitorRelative(Point::new(300.0, 300.0)))
        );
    }

    #[test]
    fn nonfinite_coordinates_do_not_cross_into_native_window_api() {
        assert_eq!(
            Position::Specific(Point::new(f32::INFINITY, 0.0))
                .resolve(Size::new(10.0, 10.0), None),
            None
        );
        assert_eq!(
            Position::SpecificWith(invalid_callback).resolve(
                Size::new(100.0, 100.0),
                Some(Size::new(200.0, 200.0))
            ),
            None
        );
        assert_eq!(
            Position::Centered.resolve(
                Size::new(100.0, 100.0),
                Some(Size::new(f32::NAN, 1080.0))
            ),
            None
        );
        assert_eq!(
            Position::Centered.resolve(
                Size::new(0.0, 100.0),
                Some(Size::new(1920.0, 1080.0))
            ),
            None
        );
    }
}

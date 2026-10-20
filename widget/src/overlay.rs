//! TAIM overlay placement policy and desktop pop-up constraints.
//!
//! Keep popup anchoring centralized so menus cannot be laid out with
//! NaN offsets or clipped to zero width at a right-hand window edge.
pub mod menu;

use crate::core::{Point, Size};

/// Fit a requested menu anchor inside a finite physical viewport.
///
/// TAIM preserves the logical Y anchor (clamped to the viewport), then
/// shifts the X coordinate left only when the popup would otherwise
/// overflow the right edge. Invalid dimensions collapse to zero.
/// This is safe to call during every layout without heap allocation.
#[inline]
pub(crate) fn fit_menu_anchor(anchor: Point, viewport: Size, popup_width: f32) -> Point {
    fn nonnegative(value: f32) -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    }

    let width = nonnegative(viewport.width);
    let height = nonnegative(viewport.height);
    let popup_width = nonnegative(popup_width).min(width);

    let x = if anchor.x.is_finite() { anchor.x } else { 0.0 };
    let y = if anchor.y.is_finite() { anchor.y } else { 0.0 };

    Point::new(x.clamp(0.0, width - popup_width), y.clamp(0.0, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn right_edge_popup_shifts_left_without_losing_menu_width() {
        assert_eq!(
            fit_menu_anchor(
                Point::new(190.0, 50.0),
                Size::new(200.0, 100.0),
                80.0,
            ),
            Point::new(120.0, 50.0)
        );
        assert_eq!(
            fit_menu_anchor(Point::new(10.0, 50.0), Size::new(200.0, 100.0), 80.0),
            Point::new(10.0, 50.0)
        );
    }

    #[test]
    fn malformed_popup_geometry_yields_finite_in_bounds_anchor() {
        assert_eq!(
            fit_menu_anchor(
                Point::new(f32::NAN, f32::INFINITY),
                Size::new(100.0, 100.0),
                50.0,
            ),
            Point::ORIGIN
        );
        assert_eq!(
            fit_menu_anchor(
                Point::new(200.0, -5.0),
                Size::new(100.0, 50.0),
                f32::INFINITY,
            ),
            Point::new(100.0, 0.0)
        );
    }
}

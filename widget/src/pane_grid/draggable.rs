//! TAIM pane drag-hit policy.
//!
//! User-defined pick regions are evaluated only for finite pointers inside
//! a finite, positive-size pane layout. This keeps drag-and-drop processing
//! from forwarding malformed compositor coordinates to widget handlers.

use crate::core::{Layout, Point, Rectangle};

/// Implement custom draggable pick areas within a workspace pane.
pub trait Draggable {
    /// Whether this draggable can be picked at a pointer location.
    fn can_be_dragged_at(&self, layout: Layout<'_>, cursor: Point) -> bool;

    /// Validate the shared pointer/layout invariants before invoking a
    /// caller-defined drag-hit handler.
    fn can_start_drag(&self, layout: Layout<'_>, cursor: Point) -> bool {
        if !valid_drag_hit(layout.bounds(), cursor) {
            return false;
        }

        self.can_be_dragged_at(layout, cursor)
    }
}

/// Pure geometry gate shared by every pane drag handler.
///
/// A finite point that lies outside the visible pane is not eligible.
/// Invalid extents and pointer coordinates never reach custom hit testing.
#[inline]
pub(crate) fn valid_drag_hit(bounds: Rectangle, cursor: Point) -> bool {
    bounds.x.is_finite()
        && bounds.y.is_finite()
        && bounds.width.is_finite()
        && bounds.height.is_finite()
        && bounds.width > 0.0
        && bounds.height > 0.0
        && cursor.x.is_finite()
        && cursor.y.is_finite()
        && bounds.contains(cursor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Size;

    #[test]
    fn accepts_only_finite_pointer_inside_nonempty_pane() {
        let pane = Rectangle::new(Point::new(10.0, 20.0), Size::new(100.0, 60.0));
        assert!(valid_drag_hit(pane, Point::new(20.0, 40.0)));
        assert!(!valid_drag_hit(pane, Point::new(-1.0, 40.0)));
        assert!(!valid_drag_hit(pane, Point::new(f32::NAN, 40.0)));
        assert!(!valid_drag_hit(pane, Point::new(f32::INFINITY, 40.0)));
    }

    #[test]
    fn invalid_or_empty_pane_layout_never_starts_drag() {
        let good = Point::new(10.0, 10.0);
        assert!(!valid_drag_hit(
            Rectangle::new(Point::ORIGIN, Size::new(0.0, 50.0)),
            good
        ));
        assert!(!valid_drag_hit(
            Rectangle::new(Point::ORIGIN, Size::new(f32::INFINITY, 50.0)),
            good
        ));
        assert!(!valid_drag_hit(
            Rectangle::new(Point::new(f32::NAN, 0.0), Size::new(50.0, 50.0)),
            good
        ));
    }
}

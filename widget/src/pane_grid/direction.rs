//! Directional probes used by TAIM's split-workspace navigation.
//!
//! A probe starts just beyond the **center** of the requested edge rather
//! than the top-left corner. This follows the user's visual focus when a
//! neighboring region has been subdivided.

use crate::core::{Point, Rectangle};

/// Cardinal movement relative to a pane's visible bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Search above the current pane.
    Up,
    /// Search below the current pane.
    Down,
    /// Search left of the current pane.
    Left,
    /// Search right of the current pane.
    Right,
}

impl Direction {
    /// Produce the sample point used by the pane-tree hit-test.
    ///
    /// Pane layouts are finite and checked before rendering; the probe is
    /// deliberately one coordinate unit beyond the pane's corresponding
    /// edge so it cannot select the starting pane again.
    pub(crate) fn probe(self, current: Rectangle) -> Point {
        let center_x = current.x + current.width * 0.5;
        let center_y = current.y + current.height * 0.5;

        match self {
            Self::Left => Point::new(current.x - 1.0, center_y),
            Self::Right => Point::new(current.x + current.width + 1.0, center_y),
            Self::Up => Point::new(center_x, current.y - 1.0),
            Self::Down => Point::new(center_x, current.y + current.height + 1.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_samples_center_of_each_edge() {
        let rectangle = Rectangle {
            x: 10.0,
            y: 20.0,
            width: 80.0,
            height: 50.0,
        };

        assert_eq!(Direction::Left.probe(rectangle), Point::new(9.0, 45.0));
        assert_eq!(Direction::Right.probe(rectangle), Point::new(91.0, 45.0));
        assert_eq!(Direction::Up.probe(rectangle), Point::new(50.0, 19.0));
        assert_eq!(Direction::Down.probe(rectangle), Point::new(50.0, 71.0));
    }
}

//! Stable pane partitioning along one screen axis.

use crate::core::Rectangle;

/// A fixed reference line for coordinate measurement.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Axis {
    /// Top-to-bottom partitioning with a horizontal divider.
    Horizontal,
    /// Left-to-right partitioning with a vertical divider.
    Vertical,
}

impl Axis {
    /// Splits a rectangle while honoring spacing and pane minimums whenever
    /// physically possible. Over-constrained minimums share available space
    /// proportionally instead of producing negative bounds.
    pub fn split(
        self,
        rectangle: &Rectangle,
        ratio: f32,
        spacing: f32,
        min_size_a: f32,
        min_size_b: f32,
    ) -> (Rectangle, Rectangle, f32) {
        let extent = match self {
            Self::Horizontal => finite_nonnegative(rectangle.height),
            Self::Vertical => finite_nonnegative(rectangle.width),
        };
        let (a_extent, b_extent, ratio, spacing) =
            partition(extent, ratio, spacing, min_size_a, min_size_b);

        match self {
            Self::Horizontal => (
                Rectangle {
                    height: a_extent,
                    ..*rectangle
                },
                Rectangle {
                    y: rectangle.y + a_extent + spacing,
                    height: b_extent,
                    ..*rectangle
                },
                ratio,
            ),
            Self::Vertical => (
                Rectangle {
                    width: a_extent,
                    ..*rectangle
                },
                Rectangle {
                    x: rectangle.x + a_extent + spacing,
                    width: b_extent,
                    ..*rectangle
                },
                ratio,
            ),
        }
    }

    /// Calculates the clamped split-line bounds inside a rectangle.
    pub fn split_line_bounds(self, rectangle: Rectangle, ratio: f32, spacing: f32) -> Rectangle {
        let extent = match self {
            Self::Horizontal => finite_nonnegative(rectangle.height),
            Self::Vertical => finite_nonnegative(rectangle.width),
        };
        let spacing = finite_nonnegative(spacing).min(extent);
        let ratio = finite_ratio(ratio);
        let offset = (extent * ratio - spacing * 0.5)
            .round()
            .clamp(0.0, extent - spacing);

        match self {
            Self::Horizontal => Rectangle {
                x: rectangle.x,
                y: rectangle.y + offset,
                width: finite_nonnegative(rectangle.width),
                height: spacing,
            },
            Self::Vertical => Rectangle {
                x: rectangle.x + offset,
                y: rectangle.y,
                width: spacing,
                height: finite_nonnegative(rectangle.height),
            },
        }
    }
}

#[inline]
fn partition(
    extent: f32,
    ratio: f32,
    spacing: f32,
    minimum_a: f32,
    minimum_b: f32,
) -> (f32, f32, f32, f32) {
    if extent <= 0.0 {
        return (0.0, 0.0, 0.5, 0.0);
    }

    let spacing = finite_nonnegative(spacing).min(extent);
    let available = extent - spacing;
    let minimum_a = finite_nonnegative(minimum_a);
    let minimum_b = finite_nonnegative(minimum_b);
    let minimum_total = minimum_a + minimum_b;

    let a = if minimum_total <= available {
        (extent * finite_ratio(ratio) - spacing * 0.5)
            .round()
            .clamp(minimum_a, available - minimum_b)
    } else if minimum_total > 0.0 {
        available * (minimum_a / minimum_total)
    } else {
        available * 0.5
    };
    let b = available - a;
    let effective_ratio = ((a + spacing * 0.5) / extent).clamp(0.0, 1.0);

    (a, b, effective_ratio, spacing)
}

#[inline]
fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[inline]
fn finite_ratio(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balanced_split_preserves_exact_extent() {
        let bounds = Rectangle::new([0.0, 0.0].into(), [9.0, 10.0].into());
        let (left, right, ratio) = Axis::Vertical.split(&bounds, 0.5, 1.0, 0.0, 0.0);

        assert_eq!(left.width, 4.0);
        assert_eq!(right.x, 5.0);
        assert_eq!(right.width, 4.0);
        assert_eq!(ratio, 0.5);
    }

    #[test]
    fn impossible_minimums_never_create_negative_regions() {
        let bounds = Rectangle::new([0.0, 0.0].into(), [10.0, 10.0].into());
        let (top, bottom, _) = Axis::Horizontal.split(&bounds, f32::NAN, 4.0, 20.0, 10.0);

        assert!(top.height >= 0.0);
        assert!(bottom.height >= 0.0);
        assert_eq!(top.height + 4.0 + bottom.height, bounds.height);
    }
}

mod bounds;
mod ops;
mod transform;

use crate::{Point, Size};

/// An axis-aligned rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rectangle<T = f32> {
    /// Horizontal coordinate of the top-left corner.
    pub x: T,
    /// Vertical coordinate of the top-left corner.
    pub y: T,
    /// Horizontal extent.
    pub width: T,
    /// Vertical extent.
    pub height: T,
}

impl<T: Default> Rectangle<T> {
    /// Builds an origin-anchored rectangle with `size`.
    pub fn with_size(size: Size<T>) -> Self {
        Self {
            x: T::default(),
            y: T::default(),
            width: size.width,
            height: size.height,
        }
    }
}

impl Rectangle<f32> {
    /// An unbounded rectangle used when no clip is active.
    pub const INFINITE: Self = Self::new(
        Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY),
        Size::INFINITE,
    );

    /// Builds a rectangle from a top-left point and a size.
    pub const fn new(top_left: Point, size: Size) -> Self {
        Self {
            x: top_left.x,
            y: top_left.y,
            width: size.width,
            height: size.height,
        }
    }
}

#[inline]
fn far_edge(origin: f32, extent: f32) -> f32 {
    if origin == f32::NEG_INFINITY && extent == f32::INFINITY {
        f32::INFINITY
    } else {
        origin + extent
    }
}

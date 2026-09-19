//! Borders and independently configurable corner radii.
mod radius;

pub use radius::Radius;

use crate::{Color, Pixels};

/// A border around a graphics primitive.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Border {
    /// Border color.
    pub color: Color,
    /// Nonnegative border width.
    pub width: f32,
    /// Corner radii.
    pub radius: Radius,
}

/// Creates a border with the given radii.
pub fn rounded(radius: impl Into<Radius>) -> Border {
    Border::default().rounded(radius)
}

/// Creates a border with the given color.
pub fn color(color: impl Into<Color>) -> Border {
    Border::default().color(color)
}

/// Creates a border with the given width.
pub fn width(width: impl Into<Pixels>) -> Border {
    Border::default().width(width)
}

impl Border {
    /// Sets border color.
    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.color = color.into();
        self
    }

    /// Sets corner radii.
    pub fn rounded(mut self, radius: impl Into<Radius>) -> Self {
        self.radius = radius.into();
        self
    }

    /// Sets a finite nonnegative width.
    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        let width = width.into().0;
        self.width = if width.is_finite() {
            width.max(0.0)
        } else {
            0.0
        };
        self
    }
}

/// Creates equal radii for every corner.
pub fn radius(value: impl Into<Pixels>) -> Radius {
    Radius::new(value)
}

/// Creates top-left-only radii.
pub fn top_left(value: impl Into<Pixels>) -> Radius {
    Radius::default().top_left(value)
}

/// Creates top-right-only radii.
pub fn top_right(value: impl Into<Pixels>) -> Radius {
    Radius::default().top_right(value)
}

/// Creates bottom-right-only radii.
pub fn bottom_right(value: impl Into<Pixels>) -> Radius {
    Radius::default().bottom_right(value)
}

/// Creates bottom-left-only radii.
pub fn bottom_left(value: impl Into<Pixels>) -> Radius {
    Radius::default().bottom_left(value)
}

/// Creates equal top radii.
pub fn top(value: impl Into<Pixels>) -> Radius {
    Radius::default().top(value)
}

/// Creates equal bottom radii.
pub fn bottom(value: impl Into<Pixels>) -> Radius {
    Radius::default().bottom(value)
}

/// Creates equal left radii.
pub fn left(value: impl Into<Pixels>) -> Radius {
    Radius::default().left(value)
}

/// Creates equal right radii.
pub fn right(value: impl Into<Pixels>) -> Radius {
    Radius::default().right(value)
}

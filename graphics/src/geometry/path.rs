//! Build different kinds of 2D shapes.
pub mod arc;

mod builder;

#[doc(no_inline)]
pub use arc::Arc;
pub use builder::Builder;

pub use lyon_path;

use crate::core::border;
use crate::core::{Point, Size};

/// An immutable set of points that may or may not be connected.
///
/// A single [`Path`] can represent different kinds of 2D shapes!
#[derive(Debug, Clone)]
pub struct Path {
    raw: lyon_path::Path,
}

impl Path {
    /// Creates a new [`Path`] with the provided closure.
    ///
    /// Use the [`Builder`] to configure your [`Path`].
    pub fn new(f: impl FnOnce(&mut Builder)) -> Self {
        Self::with_capacity(0, 0, f)
    }

    /// Creates a path while reserving its expected storage up front.
    pub fn with_capacity(
        endpoints: usize,
        control_points: usize,
        f: impl FnOnce(&mut Builder),
    ) -> Self {
        let mut builder = Builder::with_capacity(endpoints, control_points);

        f(&mut builder);

        builder.build()
    }

    /// Creates a new [`Path`] representing a line segment given its starting
    /// and end points.
    pub fn line(from: Point, to: Point) -> Self {
        Self::with_capacity(2, 0, |p| {
            p.move_to(from);
            p.line_to(to);
        })
    }

    /// Creates a new [`Path`] representing a rectangle given its top-left
    /// corner coordinate and its `Size`.
    pub fn rectangle(top_left: Point, size: Size) -> Self {
        Self::with_capacity(4, 0, |p| p.rectangle(top_left, size))
    }

    /// Creates a new [`Path`] representing a rounded rectangle given its top-left
    /// corner coordinate, its [`Size`] and [`border::Radius`].
    pub fn rounded_rectangle(top_left: Point, size: Size, radius: border::Radius) -> Self {
        Self::with_capacity(12, 8, |p| p.rounded_rectangle(top_left, size, radius))
    }

    /// Creates a new [`Path`] representing a circle given its center
    /// coordinate and its radius.
    pub fn circle(center: Point, radius: f32) -> Self {
        Self::with_capacity(10, 8, |p| {
            p.circle(center, radius);
            p.close();
        })
    }

    /// Returns the internal [`lyon_path::Path`].
    #[inline]
    pub fn raw(&self) -> &lyon_path::Path {
        &self.raw
    }

    /// Returns the current [`Path`] with the given transform applied to it.
    #[inline]
    pub fn transform(&self, transform: &lyon_path::math::Transform) -> Path {
        Path {
            raw: self.raw.clone().transformed(transform),
        }
    }
}

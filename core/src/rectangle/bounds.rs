use super::{Rectangle, far_edge};
use crate::{Point, Size, Vector};

impl Rectangle<f32> {
    /// Builds an origin-centered square from its radius.
    pub fn with_radius(radius: f32) -> Self {
        Self::new(
            Point::new(-radius, -radius),
            Size::new(radius * 2.0, radius * 2.0),
        )
    }

    /// Returns the center point.
    #[inline]
    pub fn center(&self) -> Point {
        Point::new(self.center_x(), self.center_y())
    }

    /// Returns the horizontal center coordinate.
    #[inline]
    pub fn center_x(&self) -> f32 {
        self.x + self.width * 0.5
    }

    /// Returns the vertical center coordinate.
    #[inline]
    pub fn center_y(&self) -> f32 {
        self.y + self.height * 0.5
    }

    /// Returns the top-left point.
    #[inline]
    pub fn position(&self) -> Point {
        Point::new(self.x, self.y)
    }

    /// Returns the rectangle size.
    #[inline]
    pub fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    /// Returns width multiplied by height.
    #[inline]
    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    /// Checks containment, excluding the right and bottom edges.
    #[inline]
    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.x
            && point.x < far_edge(self.x, self.width)
            && point.y >= self.y
            && point.y < far_edge(self.y, self.height)
    }

    /// Returns the shortest distance from `point` to the rectangle.
    pub fn distance(&self, point: Point) -> f32 {
        let dx = ((point.x - self.center_x()).abs() - self.width * 0.5).max(0.0);
        let dy = ((point.y - self.center_y()).abs() - self.height * 0.5).max(0.0);
        dx.hypot(dy)
    }

    /// Finds the translation that brings this rectangle inside `container`.
    pub fn offset(&self, container: &Self) -> Vector {
        let Some(overlap) = self.intersection(container) else {
            return Vector::ZERO;
        };
        let left = overlap.x - self.x;
        let top = overlap.y - self.y;
        Vector::new(
            if left > 0.0 {
                left
            } else {
                far_edge(overlap.x, overlap.width) - far_edge(self.x, self.width)
            },
            if top > 0.0 {
                top
            } else {
                far_edge(overlap.y, overlap.height) - far_edge(self.y, self.height)
            },
        )
    }

    /// Checks full containment, including the right and bottom edges.
    #[inline]
    pub fn is_within(&self, container: &Self) -> bool {
        self.x >= container.x
            && self.y >= container.y
            && far_edge(self.x, self.width) <= far_edge(container.x, container.width)
            && far_edge(self.y, self.height) <= far_edge(container.y, container.height)
    }

    /// Computes the strictly positive overlap with `other`.
    pub fn intersection(&self, other: &Self) -> Option<Self> {
        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let right = far_edge(self.x, self.width).min(far_edge(other.x, other.width));
        let bottom = far_edge(self.y, self.height).min(far_edge(other.y, other.height));
        (right > left && bottom > top)
            .then(|| Self::new(Point::new(left, top), Size::new(right - left, bottom - top)))
    }

    /// Checks whether this rectangle has a positive overlap with `other`.
    #[inline]
    pub fn intersects(&self, other: &Self) -> bool {
        let right = far_edge(self.x, self.width).min(far_edge(other.x, other.width));
        let bottom = far_edge(self.y, self.height).min(far_edge(other.y, other.height));
        right > self.x.max(other.x) && bottom > self.y.max(other.y)
    }

    /// Returns the smallest rectangle enclosing both inputs.
    pub fn union(&self, other: &Self) -> Self {
        let left = self.x.min(other.x);
        let top = self.y.min(other.y);
        let right = far_edge(self.x, self.width).max(far_edge(other.x, other.width));
        let bottom = far_edge(self.y, self.height).max(far_edge(other.y, other.height));
        Self::new(Point::new(left, top), Size::new(right - left, bottom - top))
    }
}

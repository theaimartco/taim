use crate::{Radians, Vector};

/// An amount of space in 2 dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Size<T = f32> {
    /// The width.
    pub width: T,
    /// The height.
    pub height: T,
}

mod ops;

impl<T> Size<T> {
    /// Creates a new  [`Size`] with the given width and height.
    pub const fn new(width: T, height: T) -> Self {
        Size { width, height }
    }

    /// Maps both components while preserving their axis.
    pub fn map<U>(self, mut map: impl FnMut(T) -> U) -> Size<U> {
        Size::new(map(self.width), map(self.height))
    }

    /// Combines this size component-wise with another size.
    pub fn zip<U, V>(self, other: Size<U>, mut zip: impl FnMut(T, U) -> V) -> Size<V> {
        Size::new(
            zip(self.width, other.width),
            zip(self.height, other.height),
        )
    }

    /// Borrows both components.
    pub const fn as_ref(&self) -> Size<&T> {
        Size::new(&self.width, &self.height)
    }
}

impl Size {
    /// A [`Size`] with zero width and height.
    pub const ZERO: Size = Size::new(0., 0.);

    /// A [`Size`] with a width and height of 1 unit.
    pub const UNIT: Size = Size::new(1., 1.);

    /// A [`Size`] with infinite width and height.
    pub const INFINITE: Size = Size::new(f32::INFINITY, f32::INFINITY);

    /// Returns the minimum of each component of this size and another.
    pub fn min(self, other: Self) -> Self {
        Size {
            width: self.width.min(other.width),
            height: self.height.min(other.height),
        }
    }

    /// Returns the maximum of each component of this size and another.
    pub fn max(self, other: Self) -> Self {
        Size {
            width: self.width.max(other.width),
            height: self.height.max(other.height),
        }
    }

    /// Returns whether both axes are finite.
    pub fn is_finite(self) -> bool {
        self.width.is_finite() && self.height.is_finite()
    }

    /// Returns whether either axis has no positive extent.
    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    /// Returns the non-negative finite area, or zero for invalid dimensions.
    pub fn area(self) -> f32 {
        if self.is_finite() && !self.is_empty() {
            (self.width * self.height).min(f32::MAX)
        } else {
            0.0
        }
    }

    /// Clamps each axis between the matching minimum and maximum.
    pub fn clamp(self, minimum: Self, maximum: Self) -> Self {
        Self::new(
            self.width.clamp(minimum.width, maximum.width),
            self.height.clamp(minimum.height, maximum.height),
        )
    }

    /// Expands this [`Size`] by the given amount.
    pub fn expand(self, other: impl Into<Size>) -> Self {
        let other = other.into();

        Size {
            width: self.width + other.width,
            height: self.height + other.height,
        }
    }

    /// Rotates this [`Size`] and returns the minimum [`Size`]
    /// containing it.
    pub fn rotate(self, rotation: Radians) -> Size {
        let radians = f32::from(rotation);
        let (sin, cos) = radians.sin_cos();
        let sin = sin.abs();
        let cos = cos.abs();

        Size {
            width: self.width.abs() * cos + self.height.abs() * sin,
            height: self.width.abs() * sin + self.height.abs() * cos,
        }
    }

    /// Applies an aspect ratio to this [`Size`] without
    /// exceeding its bounds.
    pub const fn ratio(self, aspect_ratio: f32) -> Size {
        if !(aspect_ratio > 0.0 && aspect_ratio <= f32::MAX) {
            return self;
        }

        Size {
            width: (self.height * aspect_ratio).min(self.width),
            height: (self.width / aspect_ratio).min(self.height),
        }
    }
}

impl<T> From<[T; 2]> for Size<T> {
    fn from([width, height]: [T; 2]) -> Self {
        Size { width, height }
    }
}

impl<T> From<(T, T)> for Size<T> {
    fn from((width, height): (T, T)) -> Self {
        Self { width, height }
    }
}

impl From<(u32, u32)> for Size {
    fn from((width, height): (u32, u32)) -> Self {
        Size::new(width as f32, height as f32)
    }
}

impl<T> From<Vector<T>> for Size<T> {
    fn from(vector: Vector<T>) -> Self {
        Size {
            width: vector.x,
            height: vector.y,
        }
    }
}

impl<T> From<Size<T>> for [T; 2] {
    fn from(size: Size<T>) -> Self {
        [size.width, size.height]
    }
}

impl<T> From<Size<T>> for Vector<T> {
    fn from(size: Size<T>) -> Self {
        Vector::new(size.width, size.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_uses_one_sine_cosine_pair_and_preserves_bounds() {
        let size = Size::new(10.0, 20.0);

        assert_eq!(size.rotate(Radians(0.0)), size);

        let quarter_turn = size.rotate(Radians(std::f32::consts::FRAC_PI_2));
        assert!((quarter_turn.width - 20.0).abs() < 0.000_01);
        assert!((quarter_turn.height - 10.0).abs() < 0.000_01);
    }

    #[test]
    fn area_saturates_instead_of_exposing_infinity() {
        assert_eq!(Size::new(f32::MAX, 2.0).area(), f32::MAX);
        assert_eq!(Size::new(f32::NAN, 2.0).area(), 0.0);
    }
}

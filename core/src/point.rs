use num_traits::{Float, Num};
use std::fmt;

/// A 2D point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point<T = f32> {
    /// The X coordinate.
    pub x: T,

    /// The Y coordinate.
    pub y: T,
}

mod ops;

impl Point {
    /// The origin (i.e. a [`Point`] at (0, 0)).
    pub const ORIGIN: Self = Self::new(0.0, 0.0);
}

impl<T: Num> Point<T> {
    /// Creates a new [`Point`] with the given coordinates.
    pub const fn new(x: T, y: T) -> Self {
        Self { x, y }
    }

    /// Computes the distance to another [`Point`].
    pub fn distance(&self, to: Self) -> T
    where
        T: Float,
    {
        let a = self.x - to.x;
        let b = self.y - to.y;

        a.hypot(b)
    }

    /// Squared distance for comparisons that do not need a square root.
    ///
    /// Use [`Point::distance`] when coordinates may be extremely large and
    /// the actual distance is required; `hypot` handles overflow better.
    #[inline]
    pub fn distance_squared(&self, to: Self) -> T
    where
        T: Copy,
    {
        let dx = self.x - to.x;
        let dy = self.y - to.y;
        dx * dx + dy * dy
    }
}

impl<T> From<[T; 2]> for Point<T>
where
    T: Num,
{
    fn from([x, y]: [T; 2]) -> Self {
        Point { x, y }
    }
}

impl<T> From<(T, T)> for Point<T>
where
    T: Num,
{
    fn from((x, y): (T, T)) -> Self {
        Self { x, y }
    }
}

impl<T> From<Point<T>> for [T; 2] {
    fn from(point: Point<T>) -> [T; 2] {
        [point.x, point.y]
    }
}

impl<T> fmt::Display for Point<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Point {{ x: {}, y: {} }}", self.x, self.y)
    }
}

impl Point<f32> {
    /// Rounds the [`Point`] coordinates.
    pub fn round(self) -> Self {
        Point {
            x: self.x.round(),
            y: self.y.round(),
        }
    }

    /// Snaps the [`Point`] to __unsigned__ integer coordinates.
    pub fn snap(self) -> Point<u32> {
        Point {
            x: self.x.round() as u32,
            y: self.y.round() as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Point;

    #[test]
    fn squared_distance_preserves_nearby_hit_test_order() {
        let origin = Point::ORIGIN;
        assert_eq!(origin.distance_squared(Point::new(3.0, 4.0)), 25.0);
        assert!(origin.distance_squared(Point::new(3.0, 4.0))
            < origin.distance_squared(Point::new(6.0, 8.0)));
    }
}

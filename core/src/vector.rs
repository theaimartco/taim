/// A 2D vector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Vector<T = f32> {
    /// The X component of the [`Vector`]
    pub x: T,

    /// The Y component of the [`Vector`]
    pub y: T,
}

mod ops;

impl<T> Vector<T> {
    /// Creates a new [`Vector`] with the given components.
    pub const fn new(x: T, y: T) -> Self {
        Self { x, y }
    }
}

impl Vector {
    /// The zero [`Vector`].
    pub const ZERO: Self = Self::new(0.0, 0.0);

    /// Rounds the [`Vector`].
    pub fn round(self) -> Self {
        Self {
            x: self.x.round(),
            y: self.y.round(),
        }
    }
}

impl<T> From<[T; 2]> for Vector<T> {
    fn from([x, y]: [T; 2]) -> Self {
        Self::new(x, y)
    }
}

impl<T> From<Vector<T>> for [T; 2]
where
    T: Copy,
{
    fn from(other: Vector<T>) -> Self {
        [other.x, other.y]
    }
}

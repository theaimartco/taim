use super::Cursor;
use crate::{Transformation, Vector};

impl std::ops::Add<Vector> for Cursor {
    type Output = Self;

    #[inline]
    fn add(self, translation: Vector) -> Self {
        self.map_position(|point| point + translation)
    }
}

impl std::ops::Sub<Vector> for Cursor {
    type Output = Self;

    #[inline]
    fn sub(self, translation: Vector) -> Self {
        self.map_position(|point| point - translation)
    }
}

impl std::ops::Mul<Transformation> for Cursor {
    type Output = Self;

    #[inline]
    fn mul(self, transformation: Transformation) -> Self {
        self.map_position(|point| point * transformation)
    }
}

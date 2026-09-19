use super::Transformation;
use crate::{Point, Rectangle, Size, Vector};

impl std::ops::Mul for Transformation {
    type Output = Self;

    #[inline]
    fn mul(self, next: Self) -> Self {
        Self(self.0 * next.0)
    }
}

impl std::ops::Mul<Transformation> for Point {
    type Output = Self;

    #[inline]
    fn mul(self, transform: Transformation) -> Self {
        Point::new(
            transform.0.x_axis.x * self.x + transform.0.y_axis.x * self.y + transform.0.w_axis.x,
            transform.0.x_axis.y * self.x + transform.0.y_axis.y * self.y + transform.0.w_axis.y,
        )
    }
}

impl std::ops::Mul<Transformation> for Vector {
    type Output = Self;

    #[inline]
    fn mul(self, transform: Transformation) -> Self {
        Vector::new(
            transform.0.x_axis.x * self.x + transform.0.y_axis.x * self.y,
            transform.0.x_axis.y * self.x + transform.0.y_axis.y * self.y,
        )
    }
}

impl std::ops::Mul<Transformation> for Size {
    type Output = Self;

    #[inline]
    fn mul(self, transform: Transformation) -> Self {
        let vector = Vector::new(self.width, self.height) * transform;
        Size::new(vector.x, vector.y)
    }
}

impl std::ops::Mul<Transformation> for Rectangle {
    type Output = Self;

    fn mul(self, transform: Transformation) -> Self {
        if !self.x.is_finite()
            || !self.y.is_finite()
            || !self.width.is_finite()
            || !self.height.is_finite()
        {
            return Rectangle::INFINITE;
        }
        let origin = self.position() * transform;
        let horizontal = Vector::new(self.width, 0.0) * transform;
        let vertical = Vector::new(0.0, self.height) * transform;
        let opposite = origin + horizontal + vertical;
        let right = origin + horizontal;
        let bottom = origin + vertical;

        let left = origin.x.min(right.x).min(bottom.x).min(opposite.x);
        let top = origin.y.min(right.y).min(bottom.y).min(opposite.y);
        let far_x = origin.x.max(right.x).max(bottom.x).max(opposite.x);
        let far_y = origin.y.max(right.y).max(bottom.y).max(opposite.y);
        Rectangle::new(Point::new(left, top), Size::new(far_x - left, far_y - top))
    }
}

impl AsRef<[f32; 16]> for Transformation {
    fn as_ref(&self) -> &[f32; 16] {
        self.0.as_ref()
    }
}

impl From<Transformation> for [f32; 16] {
    fn from(transform: Transformation) -> Self {
        *transform.as_ref()
    }
}

impl From<Transformation> for glam::Mat4 {
    fn from(transform: Transformation) -> Self {
        transform.0
    }
}

use super::Rectangle;
use crate::Vector;

impl std::ops::Mul<f32> for Rectangle<f32> {
    type Output = Self;

    fn mul(self, scale: f32) -> Self {
        Self {
            x: self.x * scale,
            y: self.y * scale,
            width: self.width * scale,
            height: self.height * scale,
        }
    }
}

impl From<Rectangle<u32>> for Rectangle<f32> {
    fn from(value: Rectangle<u32>) -> Self {
        Self {
            x: value.x as f32,
            y: value.y as f32,
            width: value.width as f32,
            height: value.height as f32,
        }
    }
}

impl<T: std::ops::Add<Output = T>> std::ops::Add<Vector<T>> for Rectangle<T> {
    type Output = Self;

    fn add(self, offset: Vector<T>) -> Self {
        Self {
            x: self.x + offset.x,
            y: self.y + offset.y,
            ..self
        }
    }
}

impl<T: std::ops::AddAssign> std::ops::AddAssign<Vector<T>> for Rectangle<T> {
    fn add_assign(&mut self, offset: Vector<T>) {
        self.x += offset.x;
        self.y += offset.y;
    }
}

impl<T: std::ops::Sub<Output = T>> std::ops::Sub<Vector<T>> for Rectangle<T> {
    type Output = Self;

    fn sub(self, offset: Vector<T>) -> Self {
        Self {
            x: self.x - offset.x,
            y: self.y - offset.y,
            ..self
        }
    }
}

impl<T: std::ops::SubAssign> std::ops::SubAssign<Vector<T>> for Rectangle<T> {
    fn sub_assign(&mut self, offset: Vector<T>) {
        self.x -= offset.x;
        self.y -= offset.y;
    }
}

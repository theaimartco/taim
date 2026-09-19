use super::Point;
use crate::Vector;

impl<T: std::ops::Add<Output = T>> std::ops::Add<Vector<T>> for Point<T> {
    type Output = Self;

    #[inline]
    fn add(self, delta: Vector<T>) -> Self {
        Point {
            x: self.x + delta.x,
            y: self.y + delta.y,
        }
    }
}

impl<T: std::ops::Sub<Output = T>> std::ops::Sub<Vector<T>> for Point<T> {
    type Output = Self;

    #[inline]
    fn sub(self, delta: Vector<T>) -> Self {
        Point {
            x: self.x - delta.x,
            y: self.y - delta.y,
        }
    }
}

impl<T: std::ops::Sub<Output = T>> std::ops::Sub<Point<T>> for Point<T> {
    type Output = Vector<T>;

    #[inline]
    fn sub(self, origin: Self) -> Vector<T> {
        Vector::new(self.x - origin.x, self.y - origin.y)
    }
}

impl<T: std::ops::AddAssign> std::ops::AddAssign<Vector<T>> for Point<T> {
    #[inline]
    fn add_assign(&mut self, delta: Vector<T>) {
        self.x += delta.x;
        self.y += delta.y;
    }
}

impl<T: std::ops::SubAssign> std::ops::SubAssign<Vector<T>> for Point<T> {
    #[inline]
    fn sub_assign(&mut self, delta: Vector<T>) {
        self.x -= delta.x;
        self.y -= delta.y;
    }
}

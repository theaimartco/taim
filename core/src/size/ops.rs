use super::Size;
use crate::Vector;

impl<T: std::ops::Add<Output = T>> std::ops::Add for Size<T> {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        self.zip(rhs, std::ops::Add::add)
    }
}

impl<T: std::ops::Sub<Output = T>> std::ops::Sub for Size<T> {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        self.zip(rhs, std::ops::Sub::sub)
    }
}

impl<T: Copy + std::ops::Mul<Output = T>> std::ops::Mul<T> for Size<T> {
    type Output = Self;

    #[inline]
    fn mul(self, factor: T) -> Self {
        self.map(|component| component * factor)
    }
}

impl<T: Copy + std::ops::Div<Output = T>> std::ops::Div<T> for Size<T> {
    type Output = Self;

    #[inline]
    fn div(self, divisor: T) -> Self {
        self.map(|component| component / divisor)
    }
}

impl<T: Copy + std::ops::Mul<Output = T>> std::ops::Mul<Vector<T>> for Size<T> {
    type Output = Self;

    #[inline]
    fn mul(self, scale: Vector<T>) -> Self {
        Size::new(self.width * scale.x, self.height * scale.y)
    }
}

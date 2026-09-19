use super::Vector;

impl<T> Vector<T> {
    #[inline]
    fn map<U>(self, mut transform: impl FnMut(T) -> U) -> Vector<U> {
        Vector::new(transform(self.x), transform(self.y))
    }

    #[inline]
    fn zip<U, V>(self, other: Vector<U>, mut combine: impl FnMut(T, U) -> V) -> Vector<V> {
        Vector::new(combine(self.x, other.x), combine(self.y, other.y))
    }
}

impl<T: std::ops::Neg<Output = T>> std::ops::Neg for Vector<T> {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        self.map(std::ops::Neg::neg)
    }
}

impl<T: std::ops::Add<Output = T>> std::ops::Add for Vector<T> {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        self.zip(rhs, std::ops::Add::add)
    }
}

impl<T: std::ops::Sub<Output = T>> std::ops::Sub for Vector<T> {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        self.zip(rhs, std::ops::Sub::sub)
    }
}

impl<T: std::ops::AddAssign> std::ops::AddAssign for Vector<T> {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl<T: std::ops::SubAssign> std::ops::SubAssign for Vector<T> {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl<T: Copy + std::ops::Mul<Output = T>> std::ops::Mul<T> for Vector<T> {
    type Output = Self;

    #[inline]
    fn mul(self, factor: T) -> Self {
        self.map(|component| component * factor)
    }
}

impl<T: Copy + std::ops::Div<Output = T>> std::ops::Div<T> for Vector<T> {
    type Output = Self;

    #[inline]
    fn div(self, divisor: T) -> Self {
        self.map(|component| component / divisor)
    }
}

impl<T: Copy + std::ops::MulAssign> std::ops::MulAssign<T> for Vector<T> {
    #[inline]
    fn mul_assign(&mut self, factor: T) {
        self.x *= factor;
        self.y *= factor;
    }
}

impl<T: Copy + std::ops::DivAssign> std::ops::DivAssign<T> for Vector<T> {
    #[inline]
    fn div_assign(&mut self, divisor: T) {
        self.x /= divisor;
        self.y /= divisor;
    }
}

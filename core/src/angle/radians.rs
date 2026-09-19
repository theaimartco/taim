//! Radian-valued angles used by render backends.

use super::Degrees;

use std::f32::consts::{PI, TAU};
use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, RangeInclusive, Rem, Sub, SubAssign};

/// An angle measured in radians.
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct Radians(pub f32);

impl Radians {
    /// Zero rotation.
    pub const ZERO: Self = Self(0.0);
    /// Half a turn.
    pub const PI: Self = Self(PI);
    /// One full turn.
    pub const TAU: Self = Self(TAU);
    /// Canonical inclusive turn range.
    pub const RANGE: RangeInclusive<Self> = Self::ZERO..=Self::TAU;

    /// Wraps a finite angle into `0..TAU`; invalid values become zero.
    #[inline]
    pub fn normalized(self) -> Self {
        if self.0.is_finite() {
            Self(self.0.rem_euclid(TAU))
        } else {
            Self::ZERO
        }
    }

    /// Computes sine and cosine together.
    #[inline]
    pub fn sin_cos(self) -> (f32, f32) {
        self.0.sin_cos()
    }

    /// Converts this angle to degrees.
    #[inline]
    pub fn to_degrees(self) -> Degrees {
        Degrees(self.0.to_degrees())
    }
}

impl From<Degrees> for Radians {
    fn from(value: Degrees) -> Self {
        value.to_radians()
    }
}

impl From<f32> for Radians {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl From<u8> for Radians {
    fn from(value: u8) -> Self {
        Self(f32::from(value))
    }
}

impl From<Radians> for f32 {
    fn from(value: Radians) -> Self {
        value.0
    }
}

impl From<Radians> for f64 {
    fn from(value: Radians) -> Self {
        Self::from(value.0)
    }
}

impl PartialEq<f32> for Radians {
    fn eq(&self, other: &f32) -> bool {
        self.0.eq(other)
    }
}

impl PartialOrd<f32> for Radians {
    fn partial_cmp(&self, other: &f32) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}

impl Add for Radians {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl Add<Degrees> for Radians {
    type Output = Self;

    fn add(self, rhs: Degrees) -> Self {
        Self(self.0 + rhs.0.to_radians())
    }
}

impl AddAssign for Radians {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl Sub for Radians {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl SubAssign for Radians {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl Mul for Radians {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        Self(self.0 * rhs.0)
    }
}

impl Mul<f32> for Radians {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self {
        Self(self.0 * rhs)
    }
}

impl Mul<Radians> for f32 {
    type Output = Radians;

    fn mul(self, rhs: Radians) -> Radians {
        Radians(self * rhs.0)
    }
}

impl Div<f32> for Radians {
    type Output = Self;

    fn div(self, rhs: f32) -> Self {
        Self(self.0 / rhs)
    }
}

impl Div for Radians {
    type Output = Self;

    fn div(self, rhs: Self) -> Self {
        Self(self.0 / rhs.0)
    }
}

impl Rem for Radians {
    type Output = Self;

    fn rem(self, rhs: Self) -> Self {
        Self(self.0 % rhs.0)
    }
}

impl num_traits::FromPrimitive for Radians {
    fn from_i64(value: i64) -> Option<Self> {
        Some(Self(value as f32))
    }

    fn from_u64(value: u64) -> Option<Self> {
        Some(Self(value as f32))
    }

    fn from_f64(value: f64) -> Option<Self> {
        let value = value as f32;
        value.is_finite().then_some(Self(value))
    }
}

impl num_traits::AsPrimitive<f32> for Radians {
    fn as_(self) -> f32 {
        self.0
    }
}

impl num_traits::AsPrimitive<f64> for Radians {
    fn as_(self) -> f64 {
        f64::from(self.0)
    }
}

impl fmt::Display for Radians {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} rad", self.0)
    }
}

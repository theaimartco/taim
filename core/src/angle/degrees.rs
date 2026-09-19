//! Degree-valued angles.

use super::Radians;

use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, RangeInclusive, Rem, Sub, SubAssign};

/// An angle measured in degrees.
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct Degrees(pub f32);

impl Degrees {
    /// Zero rotation.
    pub const ZERO: Self = Self(0.0);
    /// Half a turn.
    pub const HALF: Self = Self(180.0);
    /// One full turn.
    pub const FULL: Self = Self(360.0);
    /// Canonical inclusive turn range.
    pub const RANGE: RangeInclusive<Self> = Self::ZERO..=Self::FULL;

    /// Wraps a finite angle into `0..360`; invalid values become zero.
    #[inline]
    pub fn normalized(self) -> Self {
        if self.0.is_finite() {
            Self(self.0.rem_euclid(Self::FULL.0))
        } else {
            Self::ZERO
        }
    }

    /// Converts this angle to radians.
    #[inline]
    pub fn to_radians(self) -> Radians {
        Radians(self.0.to_radians())
    }
}

impl PartialEq<f32> for Degrees {
    fn eq(&self, other: &f32) -> bool {
        self.0.eq(other)
    }
}

impl PartialOrd<f32> for Degrees {
    fn partial_cmp(&self, other: &f32) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}

impl From<f32> for Degrees {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl From<u8> for Degrees {
    fn from(value: u8) -> Self {
        Self(f32::from(value))
    }
}

impl From<Radians> for Degrees {
    fn from(value: Radians) -> Self {
        Self(value.0.to_degrees())
    }
}

impl From<Degrees> for f32 {
    fn from(value: Degrees) -> Self {
        value.0
    }
}

impl From<Degrees> for f64 {
    fn from(value: Degrees) -> Self {
        Self::from(value.0)
    }
}

impl Add for Degrees {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign for Degrees {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl Sub for Degrees {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl SubAssign for Degrees {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl Mul<f32> for Degrees {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self {
        Self(self.0 * rhs)
    }
}

impl Div<f32> for Degrees {
    type Output = Self;

    fn div(self, rhs: f32) -> Self {
        Self(self.0 / rhs)
    }
}

impl Rem for Degrees {
    type Output = Self;

    fn rem(self, rhs: Self) -> Self {
        Self(self.0 % rhs.0)
    }
}

impl num_traits::FromPrimitive for Degrees {
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

impl num_traits::AsPrimitive<f32> for Degrees {
    fn as_(self) -> f32 {
        self.0
    }
}

impl num_traits::AsPrimitive<f64> for Degrees {
    fn as_(self) -> f64 {
        f64::from(self.0)
    }
}

impl fmt::Display for Degrees {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}°", self.0)
    }
}

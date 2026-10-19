//! Frame-layout rotation semantics for TAIM renderable content.
//!
//! Exact cardinal turns bypass trigonometry and produce stable integer-like
//! bounds, avoiding spurious layout invalidation from floating-point sine
//! errors at 90- and 180-degree positions.

use crate::{Degrees, Radians, Size};

/// Whether a transform changes the space allocated to its content.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rotation {
    /// Rotate visually, leaving the surrounding layout unchanged.
    Floating(Radians),
    /// Rotate visually and fit the resulting axis-aligned bounds.
    Solid(Radians),
}

impl Rotation {
    /// Angle in radians without discarding the chosen layout behavior.
    #[inline]
    pub fn radians(self) -> Radians {
        match self {
            Self::Floating(value) | Self::Solid(value) => value,
        }
    }

    /// Edit the angle in place, preserving floating/solid layout semantics.
    #[inline]
    pub fn radians_mut(&mut self) -> &mut Radians {
        match self {
            Self::Floating(value) | Self::Solid(value) => value,
        }
    }

    /// Angle expressed in degrees.
    #[inline]
    pub fn degrees(self) -> Degrees {
        self.radians().to_degrees()
    }

    /// Return the visual transform's layout bounds.
    ///
    /// Static cardinal angles are handled exactly without a sine/cosine
    /// calculation. Animated non-cardinal angles retain `Size::rotate`
    /// geometry and require only one sine/cosine pair in that routine.
    pub fn apply(self, size: Size) -> Size {
        match self {
            Self::Floating(_) => size,
            Self::Solid(angle) => {
                let value = angle.0.abs();

                if value == 0.0
                    || value == std::f32::consts::PI
                    || value == std::f32::consts::TAU
                {
                    Size::new(size.width.abs(), size.height.abs())
                } else if value == std::f32::consts::FRAC_PI_2
                    || value == 3.0 * std::f32::consts::FRAC_PI_2
                {
                    Size::new(size.height.abs(), size.width.abs())
                } else {
                    size.rotate(angle)
                }
            }
        }
    }
}

impl Default for Rotation {
    fn default() -> Self {
        Self::Floating(Radians::ZERO)
    }
}

impl From<Radians> for Rotation {
    fn from(value: Radians) -> Self {
        Self::Floating(value)
    }
}

impl From<f32> for Rotation {
    fn from(value: f32) -> Self {
        Self::Floating(Radians(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cardinal_turns_have_exact_bounds_without_rounding_drift() {
        let size = Size::new(15.0, 32.0);

        assert_eq!(Rotation::Solid(Radians(0.0)).apply(size), size);
        assert_eq!(
            Rotation::Solid(Radians(std::f32::consts::PI)).apply(size),
            size
        );
        assert_eq!(
            Rotation::Solid(Radians(-std::f32::consts::FRAC_PI_2)).apply(size),
            Size::new(32.0, 15.0)
        );
        assert_eq!(
            Rotation::Solid(Radians(3.0 * std::f32::consts::FRAC_PI_2)).apply(size),
            Size::new(32.0, 15.0)
        );
    }

    #[test]
    fn floating_rotation_keeps_layout_unchanged() {
        let size = Size::new(15.0, 32.0);
        assert_eq!(
            Rotation::Floating(Radians(1.234)).apply(size),
            size
        );
    }

    #[test]
    fn noncardinal_rotation_uses_geometric_bounds() {
        let size = Size::new(20.0, 10.0);
        let value = Radians(std::f32::consts::FRAC_PI_4);
        assert_eq!(Rotation::Solid(value).apply(size), size.rotate(value));
    }

    #[test]
    fn editing_rotation_preserves_layout_mode() {
        let mut value = Rotation::Solid(Radians(0.0));
        *value.radians_mut() = Radians(std::f32::consts::FRAC_PI_2);
        assert_eq!(value.apply(Size::new(10.0, 20.0)), Size::new(20.0, 10.0));
        assert_eq!(value.degrees().0, 90.0);
    }
}

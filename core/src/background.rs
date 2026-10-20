//! TAIM background paint values and safe opacity decisions.
//!
//! Renderer and scene invalidation code must agree on whether a painted
//! surface contributes visible pixels. Transparently filled UI surfaces
//! may be culled conservatively, while gradients with any visible stop
//! remain eligible for painting.

use crate::Color;
use crate::gradient::{self, Gradient};

/// One TAIM surface fill, carrying solid color or multi-stop gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Background {
    /// Constant color across a surface.
    Color(Color),
    /// Color stops along a gradient.
    Gradient(Gradient),
}

impl Background {
    /// Scale all alpha channels with a finite clamped opacity factor.
    ///
    /// Opacity outside 0..=1 is rejected by clamping, and non-finite values
    /// become fully transparent rather than leaking NaN into GPU buffers.
    /// This is intentionally consistent for solid fills and gradient stops.
    #[must_use]
    pub fn scale_alpha(self, factor: f32) -> Self {
        let opacity = if factor.is_finite() {
            factor.clamp(0.0, 1.0)
        } else {
            0.0
        };

        match self {
            Self::Color(color) => {
                Self::Color(color.scale_alpha(opacity))
            }
            Self::Gradient(gradient) => {
                Self::Gradient(gradient.scale_alpha(opacity))
            }
        }
    }

    /// Whether this fill provably contributes no visible pixels.
    ///
    /// Deliberately conservative: unknown alpha, including NaN, does not
    /// qualify as transparent. An empty gradient is also not classified
    /// transparent because render backends may supply a fallback stop.
    /// An entirely zero-alpha linear gradient stays transparent under
    /// interpolation, including between repeated-offset hard edges.
    #[must_use]
    pub fn is_fully_transparent(self) -> bool {
        match self {
            Self::Color(color) => color.a == 0.0,
            Self::Gradient(Gradient::Linear(gradient)) => {
                let mut has_stops = false;
                for stop in gradient.stops.into_iter().flatten() {
                    has_stops = true;
                    if stop.color.a != 0.0 {
                        return false;
                    }
                }
                has_stops
            }
        }
    }
}

impl From<Color> for Background {
    fn from(color: Color) -> Self {
        Self::Color(color)
    }
}

impl From<Gradient> for Background {
    fn from(gradient: Gradient) -> Self {
        Self::Gradient(gradient)
    }
}

impl From<gradient::Linear> for Background {
    fn from(gradient: gradient::Linear) -> Self {
        Self::Gradient(Gradient::Linear(gradient))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Radians;

    #[test]
    fn zero_alpha_solid_surface_needs_no_fill() {
        assert!(Background::Color(Color::TRANSPARENT).is_fully_transparent());
        assert!(!Background::Color(Color::BLACK).is_fully_transparent());
    }

    #[test]
    fn transparent_gradient_is_safe_to_cull_but_partially_opaque_is_not() {
        let clear = gradient::Linear::new(Radians(0.0))
            .add_stop(0.0, Color::TRANSPARENT)
            .add_stop(1.0, Color::TRANSPARENT);
        assert!(Background::from(clear).is_fully_transparent());
        assert!(!Background::from(
            clear.add_stop(0.5, Color::from_rgba(0.2, 0.4, 0.6, 0.1)),
        )
        .is_fully_transparent());
    }

    #[test]
    fn alpha_scaling_is_bounded_for_solid_and_gradient() {
        let opaque = Background::from(Color::WHITE);
        assert!(opaque.scale_alpha(f32::NAN).is_fully_transparent());
        assert!(opaque.scale_alpha(f32::INFINITY).is_fully_transparent());
        assert!(opaque.scale_alpha(-4.0).is_fully_transparent());
        assert_eq!(opaque.scale_alpha(4.0), opaque);

        let gradient = gradient::Linear::new(Radians(0.0))
            .add_stop(0.0, Color::BLACK)
            .add_stop(1.0, Color::WHITE);
        assert!(Background::from(gradient)
            .scale_alpha(-1.0)
            .is_fully_transparent());
    }

    #[test]
    fn empty_gradient_is_not_assumed_transparent() {
        let empty = gradient::Linear::new(Radians(0.0));
        assert!(!Background::from(empty).is_fully_transparent());
    }
}

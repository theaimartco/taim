//! Platform-adaptive theming for Taim.
//!
//! Each [`Platform`] adjusts token resolution to match the visual
//! conventions of its target operating system:
//!
//! - **Material** (default): Taim's own design language — rounded corners,
//!   easing-dominated motion, Roboto-inspired metrics.
//! - **Cupertino**: iOS-inspired — larger radii with continuous corners,
//!   spring-dominated motion, SF-inspired metrics.
//! - **Auto**: Detects the host OS at compile time and selects the
//!   appropriate platform.

use crate::tokens::{MotionTokens, Radius};
use taim_motion::SpringConfig;

/// The target platform for adaptive theming.
///
/// This affects radius scales, motion curves, typography metrics, and
/// other platform-specific conventions without changing the semantic
/// colour tokens (those are scheme-driven, not platform-driven).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Platform {
    /// Taim's own design language: Material 3-inspired defaults.
    #[default]
    Material,
    /// iOS/macOS-inspired conventions: continuous corners, spring motion.
    Cupertino,
}

impl Platform {
    /// Detects the platform from the compilation target.
    pub const fn detect() -> Self {
        if cfg!(target_os = "ios") || cfg!(target_os = "macos") {
            Self::Cupertino
        } else {
            Self::Material
        }
    }

    /// Returns the radius value for a given [`Radius`] token on this
    /// platform.
    ///
    /// Cupertino uses larger, continuous-curve radii; Material uses the
    /// standard Taim scale.
    pub const fn radius_pixels(&self, radius: Radius) -> Option<f32> {
        match self {
            Self::Material => radius.pixels(),
            Self::Cupertino => match radius {
                Radius::None => Some(0.0),
                // Cupertino's "continuous corners" are visually larger
                // than their pixel radius suggests; we scale up.
                Radius::Small => Some(8.0),
                Radius::Medium => Some(16.0),
                Radius::Large => Some(28.0),
                Radius::Full => None, // pill, same on both platforms
            },
        }
    }

    /// Returns the platform-tuned motion tokens.
    ///
    /// Cupertino favours springs over easing for interactive feedback;
    /// Material uses shorter, easing-driven tweens.
    pub const fn motion_tokens(&self) -> MotionTokens {
        match self {
            Self::Material => MotionTokens {
                feedback_ms: 120,
                hierarchy_ms: 240,
                gesture_spring: SpringConfig {
                    angular_frequency: 16.0,
                    damping_ratio: 0.9,
                    epsilon: 0.001,
                },
            },
            Self::Cupertino => MotionTokens {
                // iOS transitions tend to be slightly longer and
                // spring-driven rather than easing-driven.
                feedback_ms: 150,
                hierarchy_ms: 350,
                gesture_spring: SpringConfig {
                    // iOS springs are less stiff, more bouncy.
                    angular_frequency: 12.0,
                    damping_ratio: 0.85,
                    epsilon: 0.001,
                },
            },
        }
    }
}

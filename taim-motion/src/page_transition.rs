//! Page-level transition types for route changes.
//!
//! These define *what* happens when a page enters or exits, leaving
//! *when* and *how* to the widget layer's `Navigator`. Every transition
//! type produces a `TransformState` that a renderer applies to the
//! entering/exiting page surface.

use crate::spring::SpringConfig;
use crate::tween::{Easing, Interpolate};

/// The visual state of a page surface mid-transition.
///
/// A renderer applies these values to the page's layer: translate, scale,
/// and fade are composed, so a `SlideLeft` that is 40% complete has
/// `offset_x = -0.4`, `opacity = 1.0`, etc.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformState {
    /// Horizontal offset as a fraction of the surface width: -1.0 = fully
    /// off-screen left, 1.0 = fully off-screen right, 0.0 = home position.
    pub offset_x: f32,
    /// Vertical offset as a fraction of the surface height.
    pub offset_y: f32,
    /// Uniform scale around the surface center.
    pub scale: f32,
    /// Surface opacity, 0.0 = invisible, 1.0 = fully opaque.
    pub opacity: f32,
}

impl Default for TransformState {
    fn default() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            scale: 1.0,
            opacity: 1.0,
        }
    }
}

impl Interpolate for TransformState {
    fn interpolate(from: Self, to: Self, amount: f32) -> Self {
        Self {
            offset_x: f32::interpolate(from.offset_x, to.offset_x, amount),
            offset_y: f32::interpolate(from.offset_y, to.offset_y, amount),
            scale: f32::interpolate(from.scale, to.scale, amount),
            opacity: f32::interpolate(from.opacity, to.opacity, amount),
        }
    }
}

impl TransformState {
    /// The identity state: no translation, full scale, full opacity.
    pub const IDENTITY: Self = Self {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 1.0,
        opacity: 1.0,
    };

    /// Fully invisible: off-screen is irrelevant when opacity is zero.
    pub const INVISIBLE: Self = Self {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 1.0,
        opacity: 0.0,
    };

    /// Returns whether this state is visually indistinguishable from the
    /// identity (home position).
    pub fn is_settled(&self) -> bool {
        (self.offset_x.abs() < 0.001)
            && (self.offset_y.abs() < 0.001)
            && ((self.scale - 1.0).abs() < 0.001)
            && ((self.opacity - 1.0).abs() < 0.001)
    }
}

/// A page transition specification.
///
/// Each variant defines the enter and exit transforms for a page route
/// change. The `Navigator` widget uses these to drive paired
/// [`Transition`](crate::Transition)s for the old and new page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PageTransition {
    /// A simple cross-fade between pages.
    Fade {
        /// Duration in milliseconds.
        duration_ms: u16,
        /// Easing curve.
        easing: Easing,
    },
    /// The new page slides in from the right, pushing the old page left.
    /// Matches the platform-standard "forward" navigation.
    SlideLeft {
        /// Duration in milliseconds.
        duration_ms: u16,
        /// Easing curve.
        easing: Easing,
    },
    /// The new page slides in from the left, pushing the old page right.
    /// The platform-standard "back" navigation.
    SlideRight {
        /// Duration in milliseconds.
        duration_ms: u16,
        /// Easing curve.
        easing: Easing,
    },
    /// The new page slides up from the bottom. Used for modal-like
    /// presentations (bottom sheet, full-screen dialog).
    SlideUp {
        /// Duration in milliseconds.
        duration_ms: u16,
        /// Easing curve.
        easing: Easing,
    },
    /// A scale + fade transition: the new page scales up from 85% while
    /// fading in, the old page scales down and fades out. Matches
    /// Material 3's "container transform".
    ZoomFade {
        /// Duration in milliseconds.
        duration_ms: u16,
        /// Easing curve.
        easing: Easing,
    },
    /// A spring-physics-driven slide that decelerates naturally. Used
    /// for gesture-driven navigation (swipe back).
    SpringSlide {
        /// The spring configuration.
        spring: SpringConfig,
    },
    /// No animation — an instant cut.
    None,
}

impl Default for PageTransition {
    fn default() -> Self {
        Self::SlideLeft {
            duration_ms: 300,
            easing: Easing::EaseInOut,
        }
    }
}

impl PageTransition {
    /// A fast fade, suitable for tab switches within the same level.
    pub const FADE: Self = Self::Fade {
        duration_ms: 200,
        easing: Easing::EaseOut,
    };

    /// The standard forward navigation slide.
    pub const FORWARD: Self = Self::SlideLeft {
        duration_ms: 300,
        easing: Easing::EaseInOut,
    };

    /// The standard back navigation slide.
    pub const BACK: Self = Self::SlideRight {
        duration_ms: 300,
        easing: Easing::EaseInOut,
    };

    /// A modal presentation from the bottom.
    pub const MODAL: Self = Self::SlideUp {
        duration_ms: 400,
        easing: Easing::EaseOut,
    };

    /// Material 3-style container transform.
    pub const ZOOM: Self = Self::ZoomFade {
        duration_ms: 350,
        easing: Easing::EaseInOut,
    };

    /// Returns the duration in seconds, or `None` for spring / instant.
    pub fn duration_seconds(&self) -> Option<f64> {
        match self {
            Self::Fade { duration_ms, .. }
            | Self::SlideLeft { duration_ms, .. }
            | Self::SlideRight { duration_ms, .. }
            | Self::SlideUp { duration_ms, .. }
            | Self::ZoomFade { duration_ms, .. } => Some(f64::from(*duration_ms) / 1000.0),
            Self::SpringSlide { .. } => None,
            Self::None => Some(0.0),
        }
    }

    /// Computes the entering page's transform at normalized progress `t`
    /// (0.0 = start of transition, 1.0 = fully arrived).
    pub fn enter_at(&self, t: f32) -> TransformState {
        let t = t.clamp(0.0, 1.0);
        let eased = self.ease(t);

        match self {
            Self::Fade { .. } => TransformState {
                opacity: eased,
                ..TransformState::IDENTITY
            },
            Self::SlideLeft { .. } => TransformState {
                offset_x: 1.0 - eased,
                ..TransformState::IDENTITY
            },
            Self::SlideRight { .. } => TransformState {
                offset_x: -(1.0 - eased),
                ..TransformState::IDENTITY
            },
            Self::SlideUp { .. } => TransformState {
                offset_y: 1.0 - eased,
                ..TransformState::IDENTITY
            },
            Self::ZoomFade { .. } => TransformState {
                scale: 0.85 + 0.15 * eased,
                opacity: eased,
                ..TransformState::IDENTITY
            },
            Self::SpringSlide { .. } => TransformState {
                offset_x: 1.0 - t,
                ..TransformState::IDENTITY
            },
            Self::None => TransformState::IDENTITY,
        }
    }

    /// Computes the exiting page's transform at normalized progress `t`
    /// (0.0 = still fully visible, 1.0 = fully gone).
    pub fn exit_at(&self, t: f32) -> TransformState {
        let t = t.clamp(0.0, 1.0);
        let eased = self.ease(t);

        match self {
            Self::Fade { .. } => TransformState {
                opacity: 1.0 - eased,
                ..TransformState::IDENTITY
            },
            Self::SlideLeft { .. } => TransformState {
                // The old page slides partially left (parallax: 30% of
                // full width) while the new page covers it.
                offset_x: -0.3 * eased,
                ..TransformState::IDENTITY
            },
            Self::SlideRight { .. } => TransformState {
                offset_x: 0.3 * eased,
                ..TransformState::IDENTITY
            },
            Self::SlideUp { .. } => TransformState {
                // Old page fades slightly as the sheet covers it.
                opacity: 1.0 - 0.3 * eased,
                ..TransformState::IDENTITY
            },
            Self::ZoomFade { .. } => TransformState {
                scale: 1.0 - 0.1 * eased,
                opacity: 1.0 - eased,
                ..TransformState::IDENTITY
            },
            Self::SpringSlide { .. } => TransformState {
                offset_x: -0.3 * t,
                ..TransformState::IDENTITY
            },
            Self::None => TransformState::INVISIBLE,
        }
    }

    /// Applies the easing curve for tween-based transitions.
    fn ease(&self, t: f32) -> f32 {
        match self {
            Self::Fade { easing, .. }
            | Self::SlideLeft { easing, .. }
            | Self::SlideRight { easing, .. }
            | Self::SlideUp { easing, .. }
            | Self::ZoomFade { easing, .. } => easing.apply(t),
            Self::SpringSlide { .. } | Self::None => t,
        }
    }
}

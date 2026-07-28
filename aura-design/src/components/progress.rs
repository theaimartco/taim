#![allow(missing_docs)]

use super::{ComponentSpec, Density, InputMethods, StateSupport};
use crate::tokens::Radius;

use aura_core::Size;
use aura_core::semantics::{Actions, Role};

/// A determinate or indeterminate progress indicator.
///
/// `value` is `None` for indeterminate work (duration unknown) and
/// `Some(fraction)` for determinate. An indeterminate indicator is the
/// one place Aura animates continuously, which is exactly why
/// [`Self::respects_reduced_motion`] matters: under a reduced-motion
/// policy the animation must not be a spinning distraction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progress {
    pub value: Option<f32>,
    pub circular: bool,
}

impl Progress {
    /// Returns the clamped completion fraction, or `None` when
    /// indeterminate. Out-of-range values clamp rather than overflowing
    /// the track.
    pub fn fraction(&self) -> Option<f32> {
        self.value.map(|value| value.clamp(0.0, 1.0))
    }

    /// Whether this indicator is indeterminate (continuously animating).
    pub const fn is_indeterminate(&self) -> bool {
        self.value.is_none()
    }

    /// Under a reduced-motion policy an indeterminate indicator keeps
    /// communicating "work is happening" -- it must not simply freeze,
    /// which would read as "stalled" -- but does so with a non-spatial
    /// pulse rather than continuous travel.
    ///
    /// Reduced motion is a semantic substitution, never a global
    /// zero-duration switch (see `aura_motion::MotionPolicy`).
    pub fn respects_reduced_motion(
        &self,
        policy: aura_motion::MotionPolicy,
    ) -> aura_motion::Effect {
        use aura_motion::{Effect, MotionPolicy};

        match (policy, self.is_indeterminate()) {
            // Determinate progress just moves to its new value; that is a
            // meaningful position change, not decoration.
            (_, false) => Effect::Transform,
            (MotionPolicy::Full, true) => Effect::Transform,
            // Indeterminate under reduced motion: fade in place.
            (MotionPolicy::Reduced, true) => Effect::Fade,
        }
    }
}

impl ComponentSpec for Progress {
    fn role(&self) -> Role {
        Role::Group
    }

    fn actions(&self) -> Actions {
        // Progress reports; it is never operated.
        Actions::empty()
    }

    fn min_size(&self, _density: Density) -> Size {
        if self.circular {
            Size::new(48.0, 48.0)
        } else {
            // A linear track is thin by design; it is not a touch target.
            Size::new(120.0, 8.0)
        }
    }

    fn supported_states(&self) -> StateSupport {
        StateSupport::LOADING | StateSupport::ERROR
    }

    fn input_methods(&self) -> InputMethods {
        InputMethods::empty()
    }

    fn radius(&self) -> Radius {
        Radius::Full
    }
}

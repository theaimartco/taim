//! Animation: time-based [`Animation`]s between states and physics-based
//! [`Motion`].
pub use taim_motion::Easing;
pub use taim_motion::{
    Effect as MotionEffect, Interpolate as MotionInterpolate, MotionPolicy, Spring, SpringConfig,
    Timestamp as MotionTimestamp,
};

mod motion;
mod tween;

pub use motion::{Motion, MotionClock};
pub use tween::Animation;

/// A state an [`Animation`] can move between, placed on a number line.
///
/// Only the relative positions matter: they decide how far apart two states
/// are and in which direction a transition runs.
pub trait Float {
    /// This state's position.
    fn float_value(&self) -> f32;
}

impl Float for f32 {
    fn float_value(&self) -> f32 {
        *self
    }
}

impl Float for bool {
    fn float_value(&self) -> f32 {
        f32::from(u8::from(*self))
    }
}

/// A value an [`Animation`] can blend.
pub trait Interpolable {
    /// The value `ratio` of the way from `self` to `other`.
    fn interpolated(&self, other: Self, ratio: f32) -> Self;
}

impl Interpolable for f32 {
    fn interpolated(&self, other: Self, ratio: f32) -> Self {
        // Weighted form: exact at both ends (`ratio` 0 gives `self`,
        // 1 gives `other`), which callers comparing against the endpoints
        // rely on.
        self * (1.0 - ratio) + other * ratio
    }
}

/// Blends when both sides are present; otherwise there is nothing to blend.
impl<T: Interpolable + Copy> Interpolable for Option<T> {
    fn interpolated(&self, other: Self, ratio: f32) -> Self {
        Some(self.as_ref()?.interpolated(other?, ratio))
    }
}

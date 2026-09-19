//! Animate your applications.
pub use lilt::{Easing, FloatRepresentable as Float, Interpolable};
pub use taim_motion::{
    Effect as MotionEffect, Interpolate as MotionInterpolate, MotionPolicy, Spring, SpringConfig,
    Timestamp as MotionTimestamp,
};

mod motion;
mod tween;

pub use motion::{Motion, MotionClock};
pub use tween::Animation;

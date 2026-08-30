//! Frame-clock-driven motion primitives for Taim.
//!
//! This crate owns no event loop, timer, renderer, or widget state. Callers
//! advance motion with elapsed monotonic time from the runtime's shared frame
//! clock, which keeps results independent of display refresh rate and makes
//! the math deterministic under a test clock.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod clock;
pub mod decay;
pub mod keyframes;
pub mod page_transition;
pub mod reduced_motion;
pub mod shared_element;
pub mod spring;
pub mod timeline;
pub mod transition;
pub mod tween;

pub use clock::{Clock, Timestamp};
pub use decay::{Bounds, Decay, DecayStep};
pub use keyframes::{Keyframe, Keyframes, KeyframesError};
pub use page_transition::{PageTransition, TransformState};
pub use reduced_motion::{Effect, MotionPolicy};
pub use shared_element::{SharedElement, SharedElementId, SharedElements, Snapshot};
pub use spring::{Spring, SpringConfig};
pub use timeline::{Direction, Segment, Timeline};
pub use transition::{Transition, TransitionState};
pub use tween::{Easing, Interpolate, Tween};

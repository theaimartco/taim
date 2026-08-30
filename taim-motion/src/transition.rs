//! Interruptible state transitions.

use crate::clock::Timestamp;
use crate::tween::{Easing, Interpolate, Tween};

/// The lifecycle of a [`Transition`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionState {
    /// The transition has no active interpolation.
    Settled,
    /// The transition is moving toward a target.
    Running,
}

/// A retargetable tween that preserves its sampled position on interruption.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition<T> {
    tween: Tween<T>,
    started_at: Timestamp,
    state: TransitionState,
}

impl<T: Interpolate> Transition<T> {
    /// Creates a settled transition at `value`.
    pub fn new(value: T, now: Timestamp) -> Self {
        Self {
            tween: Tween::new(value, value, 0.0),
            started_at: now,
            state: TransitionState::Settled,
        }
    }

    /// Starts or interrupts toward `target`, retaining the exact current value.
    pub fn retarget(&mut self, target: T, now: Timestamp, duration: f64, easing: Easing) {
        let current = self.value_at(now);
        self.tween = Tween::new(current, target, duration).easing(easing);
        self.started_at = now;
        self.state = if duration <= 0.0 {
            TransitionState::Settled
        } else {
            TransitionState::Running
        };
    }

    /// Samples the transition and settles it once complete.
    pub fn value_at(&mut self, now: Timestamp) -> T {
        let elapsed = now.elapsed_since(self.started_at);
        let value = self.tween.sample(elapsed);
        if self.tween.is_complete(elapsed) {
            self.state = TransitionState::Settled;
        }
        value
    }

    /// Returns whether another frame is needed.
    pub const fn is_running(&self) -> bool {
        matches!(self.state, TransitionState::Running)
    }

    /// Returns whether this transition needs another frame at `now` without
    /// mutating completion bookkeeping.
    pub fn is_running_at(&self, now: Timestamp) -> bool {
        matches!(self.state, TransitionState::Running)
            && !self.tween.is_complete(now.elapsed_since(self.started_at))
    }
}

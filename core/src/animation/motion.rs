use super::{MotionInterpolate, MotionTimestamp};
use crate::time::{Duration, Instant};

/// An adapter from Taim's monotonic clock to Taim Motion's frame-clock API.
///
/// Create one clock per application or window and use [`Self::at`] to convert
/// redraw timestamps. Motion calculations use the elapsed duration from this
/// stable origin, never a frame count or an independent widget timer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionClock {
    origin: Instant,
}

impl MotionClock {
    /// Creates a clock whose origin is `origin`.
    pub const fn new(origin: Instant) -> Self {
        Self { origin }
    }

    /// Converts an Taim redraw timestamp to an Taim Motion timestamp.
    pub fn at(&self, instant: Instant) -> MotionTimestamp {
        MotionTimestamp::from_seconds(instant.saturating_duration_since(self.origin).as_secs_f64())
    }
}

impl taim_motion::Clock for MotionClock {
    fn now(&self) -> MotionTimestamp {
        self.at(Instant::now())
    }
}

/// A compatibility adapter that lets existing transition widgets drive Taim
/// Motion's interruptible tween engine from Taim redraw timestamps.
///
/// Unlike [`Animation`], this type uses Taim Motion's typed interpolation and
/// preserves its sampled value when a target is replaced. It remains opt-in so
/// existing applications retain the established `lilt`-backed API.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion<T: MotionInterpolate> {
    clock: MotionClock,
    transition: taim_motion::Transition<T>,
    duration: Duration,
    easing: taim_motion::Easing,
}

impl<T: MotionInterpolate> Motion<T> {
    /// Creates a settled motion at `value` using the current monotonic time.
    pub fn new(value: T) -> Self {
        let origin = Instant::now();
        let clock = MotionClock::new(origin);

        Self {
            clock,
            transition: taim_motion::Transition::new(value, MotionTimestamp::from_seconds(0.0)),
            duration: Duration::from_millis(100),
            easing: taim_motion::Easing::Linear,
        }
    }

    /// Sets this motion's duration.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Sets this motion's easing curve.
    pub fn easing(mut self, easing: taim_motion::Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Retargets without discarding the currently sampled position.
    pub fn go_mut(&mut self, target: T, at: Instant) {
        self.transition.retarget(
            target,
            self.clock.at(at),
            self.duration.as_secs_f64(),
            self.easing,
        );
    }

    /// Samples the current value at a redraw timestamp.
    pub fn value_at(&mut self, at: Instant) -> T {
        self.transition.value_at(self.clock.at(at))
    }

    /// Returns whether this motion needs another frame at `at`.
    pub fn is_animating(&self, at: Instant) -> bool {
        self.transition.is_running_at(self.clock.at(at))
    }
}

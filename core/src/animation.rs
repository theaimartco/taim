//! Animate your applications.
use crate::time::{Duration, Instant};

pub use aura_motion::{
    Effect as MotionEffect, Interpolate as MotionInterpolate, MotionPolicy, Spring, SpringConfig,
    Timestamp as MotionTimestamp,
};
pub use lilt::{Easing, FloatRepresentable as Float, Interpolable};

/// An adapter from Aura's monotonic clock to Aura Motion's frame-clock API.
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

    /// Converts an Aura redraw timestamp to an Aura Motion timestamp.
    pub fn at(&self, instant: Instant) -> MotionTimestamp {
        MotionTimestamp::from_seconds(instant.saturating_duration_since(self.origin).as_secs_f64())
    }
}

impl aura_motion::Clock for MotionClock {
    fn now(&self) -> MotionTimestamp {
        self.at(Instant::now())
    }
}

/// A compatibility adapter that lets existing transition widgets drive Aura
/// Motion's interruptible tween engine from Aura redraw timestamps.
///
/// Unlike [`Animation`], this type uses Aura Motion's typed interpolation and
/// preserves its sampled value when a target is replaced. It remains opt-in so
/// existing applications retain the established `lilt`-backed API.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion<T: MotionInterpolate> {
    clock: MotionClock,
    transition: aura_motion::Transition<T>,
    duration: Duration,
    easing: aura_motion::Easing,
}

impl<T: MotionInterpolate> Motion<T> {
    /// Creates a settled motion at `value` using the current monotonic time.
    pub fn new(value: T) -> Self {
        let origin = Instant::now();
        let clock = MotionClock::new(origin);

        Self {
            clock,
            transition: aura_motion::Transition::new(value, MotionTimestamp::from_seconds(0.0)),
            duration: Duration::from_millis(100),
            easing: aura_motion::Easing::Linear,
        }
    }

    /// Sets this motion's duration.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Sets this motion's easing curve.
    pub fn easing(mut self, easing: aura_motion::Easing) -> Self {
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

/// The animation of some particular state.
///
/// It tracks state changes and allows projecting interpolated values
/// through time.
#[derive(Debug, Clone)]
pub struct Animation<T>
where
    T: Clone + Copy + PartialEq + Float,
{
    raw: lilt::Animated<T, Instant>,
    duration: Duration, // TODO: Expose duration getter in `lilt`
}

impl<T> Animation<T>
where
    T: Clone + Copy + PartialEq + Float,
{
    /// Creates a new [`Animation`] with the given initial state.
    pub fn new(state: T) -> Self {
        Self {
            raw: lilt::Animated::new(state),
            duration: Duration::from_millis(100),
        }
    }

    /// Sets the [`Easing`] function of the [`Animation`].
    ///
    /// See the [Easing Functions Cheat Sheet](https://easings.net) for
    /// details!
    pub fn easing(mut self, easing: Easing) -> Self {
        self.raw = self.raw.easing(easing);
        self
    }

    /// Sets the duration of the [`Animation`] to 100ms.
    pub fn very_quick(self) -> Self {
        self.duration(Duration::from_millis(100))
    }

    /// Sets the duration of the [`Animation`] to 200ms.
    pub fn quick(self) -> Self {
        self.duration(Duration::from_millis(200))
    }

    /// Sets the duration of the [`Animation`] to 400ms.
    pub fn slow(self) -> Self {
        self.duration(Duration::from_millis(400))
    }

    /// Sets the duration of the [`Animation`] to 500ms.
    pub fn very_slow(self) -> Self {
        self.duration(Duration::from_millis(500))
    }

    /// Sets the duration of the [`Animation`] to the given value.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.raw = self.raw.duration(duration.as_secs_f32() * 1_000.0);
        self.duration = duration;
        self
    }

    /// Sets a delay for the [`Animation`].
    pub fn delay(mut self, duration: Duration) -> Self {
        self.raw = self.raw.delay(duration.as_secs_f64() as f32 * 1000.0);
        self
    }

    /// Makes the [`Animation`] repeat a given amount of times.
    ///
    /// Providing 1 repetition plays the animation twice in total.
    pub fn repeat(mut self, repetitions: u32) -> Self {
        self.raw = self.raw.repeat(repetitions);
        self
    }

    /// Makes the [`Animation`] repeat forever.
    pub fn repeat_forever(mut self) -> Self {
        self.raw = self.raw.repeat_forever();
        self
    }

    /// Makes the [`Animation`] automatically reverse when repeating.
    pub fn auto_reverse(mut self) -> Self {
        self.raw = self.raw.auto_reverse();
        self
    }

    /// Transitions the [`Animation`] from its current state to the given new state
    /// at the given time.
    pub fn go(mut self, new_state: T, at: Instant) -> Self {
        self.go_mut(new_state, at);
        self
    }

    /// Transitions the [`Animation`] from its current state to the given new state
    /// at the given time, by reference.
    pub fn go_mut(&mut self, new_state: T, at: Instant) {
        self.raw.transition(new_state, at);
    }

    /// Returns true if the [`Animation`] is currently in progress.
    ///
    /// An [`Animation`] is in progress when it is transitioning to a different state.
    pub fn is_animating(&self, at: Instant) -> bool {
        self.raw.in_progress(at)
    }

    /// Projects the [`Animation`] into an interpolated value at the given [`Instant`]; using the
    /// closure provided to calculate the different keyframes of interpolated values.
    ///
    /// If the [`Animation`] state is a `bool`, you can use the simpler [`interpolate`] method.
    ///
    /// [`interpolate`]: Animation::interpolate
    pub fn interpolate_with<I>(&self, f: impl Fn(T) -> I, at: Instant) -> I
    where
        I: Interpolable,
    {
        self.raw.animate(f, at)
    }

    /// Retuns the current state of the [`Animation`].
    pub fn value(&self) -> T {
        self.raw.value
    }
}

impl Animation<f32> {
    /// Projects the [`Animation`] into an interpolated value at the given [`Instant`].
    pub fn interpolate(&self, at: Instant) -> f32 {
        self.interpolate_with(std::convert::identity, at)
    }
}

impl Animation<bool> {
    /// Projects the [`Animation`] into an interpolated value at the given [`Instant`]; using the
    /// `start` and `end` values as the origin and destination keyframes.
    pub fn interpolate<I>(&self, start: I, end: I, at: Instant) -> I
    where
        I: Interpolable + Clone,
    {
        self.raw.animate_bool(start, end, at)
    }

    /// Returns the remaining [`Duration`] of the [`Animation`].
    pub fn remaining(&self, at: Instant) -> Duration {
        Duration::from_secs_f32(self.interpolate(self.duration.as_secs_f32(), 0.0, at))
    }
}

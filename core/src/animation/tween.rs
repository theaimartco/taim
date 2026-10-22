//! Time-based transitions between discrete states.
use super::{Easing, Float, Interpolable};
use crate::time::{Duration, Instant};

/// A value that moves smoothly to each new state it is given.
///
/// The animation tracks a *state* of type `T` (a `bool`, an index, an
/// offset…). Every state has a position on a number line
/// ([`Float::float_value`]); a transition moves along that line from where
/// the animation currently is to the new state's position. Interrupting a
/// transition starts the next one from the current, eased position, so
/// reversing a toggle half way through never jumps.
#[derive(Debug, Clone)]
pub struct Animation<T>
where
    T: Copy + PartialEq + Float,
{
    /// The state being moved to.
    state: T,
    /// The state the current transition started from.
    previous: T,
    /// Where on the number line the current transition started; differs
    /// from `previous` after an interruption.
    origin: f32,
    started: Option<Instant>,
    timing: Timing,
}

#[derive(Debug, Clone, Copy)]
struct Timing {
    duration: Duration,
    delay: Duration,
    easing: Easing,
    /// Extra plays after the first; `None` repeats forever.
    repeats: Option<u32>,
    auto_reverse: bool,
}

impl<T> Animation<T>
where
    T: Copy + PartialEq + Float,
{
    /// An animation resting at `state`: 100 ms, sine ease-in-out.
    pub fn new(state: T) -> Self {
        Self {
            state,
            previous: state,
            origin: state.float_value(),
            started: None,
            timing: Timing {
                duration: Duration::from_millis(100),
                delay: Duration::ZERO,
                easing: Easing::EaseInOutSine,
                repeats: Some(0),
                auto_reverse: false,
            },
        }
    }

    /// Sets the easing curve.
    pub fn easing(mut self, easing: Easing) -> Self {
        self.timing.easing = easing;
        self
    }

    /// 100 ms.
    pub fn very_quick(self) -> Self {
        self.duration(Duration::from_millis(100))
    }

    /// 200 ms.
    pub fn quick(self) -> Self {
        self.duration(Duration::from_millis(200))
    }

    /// 400 ms.
    pub fn slow(self) -> Self {
        self.duration(Duration::from_millis(400))
    }

    /// 500 ms.
    pub fn very_slow(self) -> Self {
        self.duration(Duration::from_millis(500))
    }

    /// Sets how long one transition takes.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.timing.duration = duration;
        self
    }

    /// Waits `delay` after each state change before moving.
    pub fn delay(mut self, delay: Duration) -> Self {
        self.timing.delay = delay;
        self
    }

    /// Plays each transition `repetitions` extra times (1 plays it twice).
    pub fn repeat(mut self, repetitions: u32) -> Self {
        self.timing.repeats = Some(repetitions);
        self
    }

    /// Plays each transition forever.
    pub fn repeat_forever(mut self) -> Self {
        self.timing.repeats = None;
        self
    }

    /// Plays every other repetition backwards.
    pub fn auto_reverse(mut self) -> Self {
        self.timing.auto_reverse = true;
        self
    }

    /// [`go_mut`](Self::go_mut), by value.
    pub fn go(mut self, state: T, at: Instant) -> Self {
        self.go_mut(state, at);
        self
    }

    /// Starts moving to `state` at `at`. Does nothing if `state` is already
    /// the target.
    pub fn go_mut(&mut self, state: T, at: Instant) {
        if state == self.state {
            return;
        }

        self.origin = if self.is_animating(at) {
            self.position(at)
        } else {
            self.state.float_value()
        };
        self.previous = self.state;
        self.state = state;
        self.started = Some(at);
    }

    /// Whether a transition is still running at `at`.
    pub fn is_animating(&self, at: Instant) -> bool {
        self.progress(at).is_some()
    }

    /// The animated value at `at`, obtained by mapping states to values with
    /// `f` and blending between them.
    pub fn interpolate_with<I>(&self, f: impl Fn(T) -> I, at: Instant) -> I
    where
        I: Interpolable,
    {
        let target = f(self.state);

        // `origin` is a point on the number line, but `I` only exists at the
        // states themselves; rebuild the starting value by blending the two
        // states the interrupted transition was between.
        let from = self.previous.float_value();
        let span = self.state.float_value() - from;
        let start_ratio = if span == 0.0 {
            0.0
        } else {
            (self.origin - from) / span
        };
        let start = f(self.previous).interpolated(f(self.state), start_ratio);

        start.interpolated(target, self.eased(at))
    }

    /// The state being moved to (not the animated value).
    pub fn value(&self) -> T {
        self.state
    }

    /// Eased progress of the current transition, 0 to 1; 1 when at rest.
    fn eased(&self, at: Instant) -> f32 {
        match self.progress(at) {
            Some(linear) => self.timing.easing.apply(linear),
            None => self.resting_progress(),
        }
    }

    /// Linear progress through the current play, or `None` when finished.
    fn progress(&self, at: Instant) -> Option<f32> {
        let started = self.started?;
        let Timing {
            duration,
            delay,
            repeats,
            auto_reverse,
            ..
        } = self.timing;

        let elapsed = at.saturating_duration_since(started).saturating_sub(delay);
        if duration.is_zero() {
            return None;
        }

        let plays = elapsed.as_secs_f32() / duration.as_secs_f32();
        if repeats.is_some_and(|extra| plays >= (extra + 1) as f32) {
            return None;
        }

        let fraction = plays.fract();
        let backwards = auto_reverse && (plays as u64) % 2 == 1;

        Some(if backwards { 1.0 - fraction } else { fraction })
    }

    /// Where a finished transition rests: at the end, unless it auto-reversed
    /// an even number of plays and so came back to the start.
    fn resting_progress(&self) -> f32 {
        match self.timing.repeats {
            Some(extra) if self.timing.auto_reverse && extra % 2 == 1 => 0.0,
            _ => 1.0,
        }
    }

    /// Current position on the number line.
    fn position(&self, at: Instant) -> f32 {
        let target = self.state.float_value();
        self.origin + (target - self.origin) * self.eased(at)
    }
}

impl Animation<f32> {
    /// The animated number at `at`.
    pub fn interpolate(&self, at: Instant) -> f32 {
        self.interpolate_with(|value| value, at)
    }
}

impl Animation<bool> {
    /// Blends from `start` (state `false`) to `end` (state `true`) at `at`.
    pub fn interpolate<I>(&self, start: I, end: I, at: Instant) -> I
    where
        I: Interpolable + Clone,
    {
        self.interpolate_with(
            |on| if on { end.clone() } else { start.clone() },
            at,
        )
    }

    /// Time left until the animation reaches `true` (zero once there).
    pub fn remaining(&self, at: Instant) -> Duration {
        let duration = self.timing.duration.as_secs_f32();
        Duration::from_secs_f32(self.interpolate(duration, 0.0, at).max(0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(start: Instant, millis: u64) -> Instant {
        start + Duration::from_millis(millis)
    }

    #[test]
    fn rests_at_its_state_until_told_to_move() {
        let now = Instant::now();
        let animation = Animation::new(3.0_f32);

        assert!(!animation.is_animating(now));
        assert_eq!(animation.interpolate(now), 3.0);
    }

    #[test]
    fn reaches_the_target_exactly() {
        let start = Instant::now();
        let mut animation = Animation::new(false).quick();
        animation.go_mut(true, start);

        assert!(animation.is_animating(ms(start, 100)));
        let halfway = animation.interpolate(0.0_f32, 1.0, ms(start, 100));
        assert!((halfway - 0.5).abs() < 1e-3, "sine midpoint, got {halfway}");

        assert!(!animation.is_animating(ms(start, 200)));
        assert_eq!(animation.interpolate(0.0_f32, 1.0, ms(start, 250)), 1.0);
        assert_eq!(animation.remaining(ms(start, 250)), Duration::ZERO);
    }

    #[test]
    fn interrupting_continues_from_the_current_position() {
        let start = Instant::now();
        let mut animation = Animation::new(0.0_f32).quick().easing(Easing::Linear);
        animation.go_mut(10.0, start);

        let reversal = ms(start, 50);
        let before = animation.interpolate(reversal);
        animation.go_mut(0.0, reversal);
        let after = animation.interpolate(reversal);

        assert!((before - 2.5).abs() < 1e-3);
        assert!((after - before).abs() < 1e-3, "no jump: {before} -> {after}");
        assert_eq!(animation.interpolate(ms(reversal, 200)), 0.0);
    }

    #[test]
    fn same_state_does_not_restart() {
        let start = Instant::now();
        let mut animation = Animation::new(1.0_f32);
        animation.go_mut(1.0, start);

        assert!(!animation.is_animating(start));
    }

    #[test]
    fn delay_holds_the_start_value() {
        let start = Instant::now();
        let animation = Animation::new(0.0_f32)
            .easing(Easing::Linear)
            .delay(Duration::from_millis(100))
            .go(10.0, start);

        assert_eq!(animation.interpolate(ms(start, 50)), 0.0);
        assert!((animation.interpolate(ms(start, 150)) - 5.0).abs() < 1e-3);
    }

    #[test]
    fn repeats_and_auto_reverse() {
        let start = Instant::now();
        let animation = Animation::new(0.0_f32)
            .easing(Easing::Linear)
            .repeat(1)
            .auto_reverse()
            .go(10.0, start);

        assert!((animation.interpolate(ms(start, 25)) - 2.5).abs() < 1e-3);
        assert!((animation.interpolate(ms(start, 125)) - 7.5).abs() < 1e-3);
        assert!(!animation.is_animating(ms(start, 200)));
        assert_eq!(animation.interpolate(ms(start, 200)), 0.0);
    }
}

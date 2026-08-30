//! Typed interpolation and easing curves.

// `Timestamp` conversion from an OS monotonic clock can lose a few ulps when
// two values are independently converted to floating-point seconds. Treating
// one nanosecond as the end boundary avoids a spurious extra frame exactly at
// a requested deadline while remaining far below any display cadence.
const COMPLETION_EPSILON: f64 = 0.000_000_001;

/// A value that can be linearly interpolated without allocation.
pub trait Interpolate: Copy {
    /// Interpolates from `from` to `to` at normalized progress `amount`.
    /// Implementations may assume `amount` is within `0.0..=1.0`.
    fn interpolate(from: Self, to: Self, amount: f32) -> Self;
}

impl Interpolate for f32 {
    fn interpolate(from: Self, to: Self, amount: f32) -> Self {
        from + (to - from) * amount
    }
}

impl Interpolate for f64 {
    fn interpolate(from: Self, to: Self, amount: f32) -> Self {
        from + (to - from) * f64::from(amount)
    }
}

/// A bounded easing curve for tween progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Easing {
    /// Constant velocity.
    #[default]
    Linear,
    /// Quadratic acceleration.
    EaseIn,
    /// Quadratic deceleration.
    EaseOut,
    /// Quadratic acceleration then deceleration.
    EaseInOut,
}

impl Easing {
    /// Applies this curve to normalized progress, clamping invalid input.
    pub fn apply(self, progress: f32) -> f32 {
        let progress = progress.clamp(0.0, 1.0);

        match self {
            Self::Linear => progress,
            Self::EaseIn => progress * progress,
            Self::EaseOut => 1.0 - (1.0 - progress) * (1.0 - progress),
            Self::EaseInOut if progress < 0.5 => 2.0 * progress * progress,
            Self::EaseInOut => {
                let inverse = -2.0 * progress + 2.0;
                1.0 - inverse * inverse / 2.0
            }
        }
    }
}

/// A finite-duration interpolation between two values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tween<T> {
    from: T,
    to: T,
    duration: f64,
    easing: Easing,
}

impl<T: Interpolate> Tween<T> {
    /// Creates a tween. Negative and non-finite durations complete instantly.
    pub fn new(from: T, to: T, duration: f64) -> Self {
        Self {
            from,
            to,
            duration: duration.max(0.0),
            easing: Easing::Linear,
        }
    }

    /// Sets the easing curve.
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Returns the configured duration in seconds.
    pub const fn duration(&self) -> f64 {
        self.duration
    }

    /// Samples this tween at elapsed seconds.
    pub fn sample(&self, elapsed: f64) -> T {
        let progress = if self.is_complete(elapsed) {
            1.0
        } else {
            (elapsed / self.duration).clamp(0.0, 1.0) as f32
        };

        T::interpolate(self.from, self.to, self.easing.apply(progress))
    }

    /// Returns whether `elapsed` is at or beyond the end of this tween.
    pub fn is_complete(&self, elapsed: f64) -> bool {
        elapsed + COMPLETION_EPSILON >= self.duration
    }
}

//! TAIM clock units for desktop and browser runtimes.
//!
//! Rendering and scheduling use monotonic `Instant`; external system timestamps
//! should not be used for frame deadlines. All public unit helpers saturate
//! instead of wrapping in release builds or panicking with debug overflow.

pub use web_time::{Duration, Instant, SystemTime};

const SECONDS_PER_MINUTE: u64 = 60;
const SECONDS_PER_HOUR: u64 = SECONDS_PER_MINUTE * 60;
const SECONDS_PER_DAY: u64 = SECONDS_PER_HOUR * 24;

/// Monotonic scheduling interval from a millisecond count.
#[inline]
pub fn milliseconds(value: u64) -> Duration {
    Duration::from_millis(value)
}

/// Monotonic scheduling interval from seconds.
#[inline]
pub fn seconds(value: u64) -> Duration {
    Duration::from_secs(value)
}

/// Convert minutes to seconds with bounded integer arithmetic.
#[inline]
pub fn minutes(value: u64) -> Duration {
    seconds(value.saturating_mul(SECONDS_PER_MINUTE))
}

/// Convert hours directly to seconds (one saturating multiply).
#[inline]
pub fn hours(value: u64) -> Duration {
    seconds(value.saturating_mul(SECONDS_PER_HOUR))
}

/// Convert days to seconds without overflowing intermediate units.
#[inline]
pub fn days(value: u64) -> Duration {
    seconds(value.saturating_mul(SECONDS_PER_DAY))
}

/// Clamp a very large scheduling interval to an optional caller budget.
///
/// Useful for VYOM animation timers and idle-resource policy. `budget`
/// may be zero when a caller wants an immediate execution deadline.
#[inline]
pub fn bounded(interval: Duration, budget: Duration) -> Duration {
    interval.min(budget)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_duration_units_agree() {
        assert_eq!(milliseconds(1_000), seconds(1));
        assert_eq!(minutes(2), seconds(120));
        assert_eq!(hours(1), minutes(60));
        assert_eq!(days(1), hours(24));
    }

    #[test]
    fn huge_duration_units_saturate_without_wrapping() {
        let maximum = Duration::from_secs(u64::MAX);
        assert_eq!(minutes(u64::MAX), maximum);
        assert_eq!(hours(u64::MAX), maximum);
        assert_eq!(days(u64::MAX), maximum);
        assert_eq!(days(u64::MAX).as_secs(), u64::MAX);
    }

    #[test]
    fn bounded_deadline_preserves_zero_and_minimum() {
        assert_eq!(bounded(hours(2), minutes(5)), minutes(5));
        assert_eq!(bounded(seconds(2), seconds(10)), seconds(2));
        assert_eq!(bounded(minutes(1), Duration::ZERO), Duration::ZERO);
    }
}

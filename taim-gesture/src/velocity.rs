//! A timestamped, bounded velocity estimator.
//!
//! [`Estimator`] answers "how fast, and in what direction, was this
//! pointer moving" from a short, bounded history of recent samples --
//! bounded both by count ([`Estimator::CAPACITY`]) and by age
//! ([`Estimator::MAX_SAMPLE_AGE`]), so a pointer that pauses mid-drag
//! and then resumes does not have its velocity estimate polluted by
//! stale, pre-pause samples, and so a very long-lived pan does not grow
//! its own history without bound.
use taim_core::time::{Duration, Instant};
use taim_core::{Point, Vector};

/// One timestamped position sample fed into an [`Estimator`].
#[derive(Debug, Clone, Copy, PartialEq)]
struct Sample {
    position: Point,
    timestamp: Instant,
}

/// A bounded, timestamped velocity estimator over a pointer's recent
/// positions.
///
/// Holds at most [`Estimator::CAPACITY`] samples, each no older than
/// [`Estimator::MAX_SAMPLE_AGE`] relative to the most recent one --
/// pushing a new sample evicts both anything beyond the capacity and
/// anything that has aged out, so [`Estimator::estimate`] always
/// reflects only recent motion.
#[derive(Debug, Clone, Default)]
pub struct Estimator {
    samples: std::collections::VecDeque<Sample>,
}

impl Estimator {
    /// The maximum number of samples retained at once.
    pub const CAPACITY: usize = 8;

    /// The maximum age (relative to the most recently pushed sample) a
    /// sample may have before it is evicted.
    pub const MAX_SAMPLE_AGE: Duration = Duration::from_millis(100);

    /// Creates a fresh, empty [`Estimator`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a new position sample at `timestamp`.
    ///
    /// Samples must be pushed in non-decreasing timestamp order (the
    /// same order [`crate::recognizer::Recognizer::on_event`] already
    /// requires of the [`taim_core::pointer::Pointer`] events an
    /// [`Estimator`] is typically fed from); out-of-order samples are
    /// still accepted but may be evicted immediately by the age check
    /// against a newer sample already present.
    pub fn push(&mut self, position: Point, timestamp: Instant) {
        self.samples.push_back(Sample {
            position,
            timestamp,
        });

        while self.samples.len() > Self::CAPACITY {
            let _ = self.samples.pop_front();
        }

        while let Some(oldest) = self.samples.front() {
            if timestamp.saturating_duration_since(oldest.timestamp) > Self::MAX_SAMPLE_AGE {
                let _ = self.samples.pop_front();
            } else {
                break;
            }
        }
    }

    /// Removes every recorded sample, as if this [`Estimator`] were
    /// freshly created.
    pub fn reset(&mut self) {
        self.samples.clear();
    }

    /// Estimates the current velocity, in logical pixels per second, as
    /// a [`Vector`] from the oldest retained sample to the newest.
    ///
    /// Returns `None` when fewer than two samples (spanning a nonzero
    /// duration) are available -- there is no velocity to estimate from
    /// a single point, and no division to safely perform against a zero
    /// time delta.
    pub fn estimate(&self) -> Option<Vector> {
        let oldest = self.samples.front()?;
        let newest = self.samples.back()?;

        let elapsed = newest.timestamp.saturating_duration_since(oldest.timestamp);

        if elapsed.is_zero() {
            return None;
        }

        let displacement = newest.position - oldest.position;
        let seconds = elapsed.as_secs_f32();

        Some(Vector::new(
            displacement.x / seconds,
            displacement.y / seconds,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn epoch() -> Instant {
        static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        *EPOCH.get_or_init(Instant::now)
    }

    fn t(millis: u64) -> Instant {
        epoch() + Duration::from_millis(millis)
    }

    #[test]
    fn a_fresh_estimator_has_no_estimate() {
        let estimator = Estimator::new();

        assert_eq!(estimator.estimate(), None);
    }

    #[test]
    fn a_single_sample_has_no_estimate() {
        let mut estimator = Estimator::new();
        estimator.push(Point::new(0.0, 0.0), t(0));

        assert_eq!(estimator.estimate(), None);
    }

    #[test]
    fn two_samples_produce_a_linear_velocity_estimate() {
        let mut estimator = Estimator::new();
        estimator.push(Point::new(0.0, 0.0), t(0));
        estimator.push(Point::new(10.0, 0.0), t(50));

        let velocity = estimator
            .estimate()
            .expect("two samples must produce an estimate");

        assert!((velocity.x - 200.0).abs() < 0.01, "got {velocity:?}");
        assert!(velocity.y.abs() < 0.01);
    }

    #[test]
    fn samples_beyond_capacity_are_evicted() {
        let mut estimator = Estimator::new();

        for i in 0..(Estimator::CAPACITY * 2) {
            estimator.push(Point::new(i as f32, 0.0), t(i as u64));
        }

        assert_eq!(estimator.samples.len(), Estimator::CAPACITY);
    }

    #[test]
    fn samples_older_than_max_age_are_evicted() {
        let mut estimator = Estimator::new();

        estimator.push(Point::new(0.0, 0.0), t(0));
        estimator.push(
            Point::new(10.0, 0.0),
            t(Estimator::MAX_SAMPLE_AGE.as_millis() as u64 + 50),
        );

        assert_eq!(
            estimator.samples.len(),
            1,
            "the first sample must have aged out relative to the second"
        );
    }

    #[test]
    fn reset_clears_every_sample() {
        let mut estimator = Estimator::new();
        estimator.push(Point::new(0.0, 0.0), t(0));
        estimator.push(Point::new(10.0, 10.0), t(10));

        estimator.reset();

        assert_eq!(estimator.estimate(), None);
    }

    #[test]
    fn a_stationary_pointer_estimates_zero_velocity() {
        let mut estimator = Estimator::new();
        estimator.push(Point::new(5.0, 5.0), t(0));
        estimator.push(Point::new(5.0, 5.0), t(100));

        let velocity = estimator.estimate().unwrap();

        assert_eq!(velocity, Vector::new(0.0, 0.0));
    }
}

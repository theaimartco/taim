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

    /// Predicts the pointer's future position after `delta` time using kinematic trajectory extrapolation
    /// (closed-form curvature fitting over recent history with deterministic linear fallback, Pillars VI & XII).
    pub fn predict_future_position(&self, delta: Duration) -> Option<Point> {
        let newest = self.samples.back()?;
        let dt = delta.as_secs_f32();
        if dt <= 0.0 {
            return Some(newest.position);
        }

        let vel = self.estimate()?;

        // When 3 or more samples are available, perform a least-squares quadratic fit
        // x(t) = x0 + vx * t + 0.5 * ax * t^2 relative to t = 0 (newest sample).
        if self.samples.len() >= 3 {
            let t_new = newest.timestamp;
            let mut sum_u2 = 0.0f32;
            let mut sum_w2 = 0.0f32;
            let mut sum_uw = 0.0f32;
            let mut sum_ux = 0.0f32;
            let mut sum_wx = 0.0f32;
            let mut sum_uy = 0.0f32;
            let mut sum_wy = 0.0f32;

            for sample in self.samples.iter() {
                // Negative time offset into the past
                let elapsed = t_new
                    .saturating_duration_since(sample.timestamp)
                    .as_secs_f32();
                let u = -elapsed;
                let w = 0.5 * u * u;
                let dx = sample.position.x - newest.position.x;
                let dy = sample.position.y - newest.position.y;

                sum_u2 += u * u;
                sum_w2 += w * w;
                sum_uw += u * w;
                sum_ux += u * dx;
                sum_wx += w * dx;
                sum_uy += u * dy;
                sum_wy += w * dy;
            }

            let det = sum_u2 * sum_w2 - sum_uw * sum_uw;
            if det > 1e-9 {
                let vx_fit = (sum_ux * sum_w2 - sum_wx * sum_uw) / det;
                let ax_fit = (sum_u2 * sum_wx - sum_uw * sum_ux) / det;
                let vy_fit = (sum_uy * sum_w2 - sum_wy * sum_uw) / det;
                let ay_fit = (sum_u2 * sum_wy - sum_uw * sum_uy) / det;

                // Physical acceleration bounds on touchscreen glass (max 20,000 px/s^2)
                let ax = ax_fit.clamp(-20000.0, 20000.0);
                let ay = ay_fit.clamp(-20000.0, 20000.0);

                // Blend fitted velocity with bulk velocity estimate to reject sensor jitter
                let vx = 0.7 * vx_fit + 0.3 * vel.x;
                let vy = 0.7 * vy_fit + 0.3 * vel.y;

                return Some(Point::new(
                    newest.position.x + vx * dt + 0.5 * ax * dt * dt,
                    newest.position.y + vy * dt + 0.5 * ay * dt * dt,
                ));
            }
        }

        // Deterministic linear fallback
        Some(Point::new(
            newest.position.x + vel.x * dt,
            newest.position.y + vel.y * dt,
        ))
    }

    /// Extrapolates a multi-step speculative trajectory spline forward in time for predictive presentation.
    pub fn predict_trajectory(&self, horizon: Duration, steps: usize) -> Vec<Point> {
        if steps == 0 {
            return Vec::new();
        }
        let total_secs = horizon.as_secs_f32();
        let step_dt = total_secs / steps as f32;
        (1..=steps)
            .filter_map(|i| {
                self.predict_future_position(Duration::from_secs_f32(step_dt * i as f32))
            })
            .collect()
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

    #[test]
    fn predict_future_position_speculatively_extrapolates_trajectory() {
        let mut estimator = Estimator::new();
        // Moving at 1000 px/sec along X
        estimator.push(Point::new(0.0, 0.0), t(0));
        estimator.push(Point::new(10.0, 0.0), t(10));
        estimator.push(Point::new(20.0, 0.0), t(20));

        // Predict 16ms ahead (1 frame at 60Hz)
        let predicted = estimator.predict_future_position(Duration::from_millis(16));
        assert!(predicted.is_some());
        let pos = predicted.unwrap();
        // Should be around x = 20.0 + (1000 * 0.016) = 36.0
        assert!((pos.x - 36.0).abs() < 2.0);
        assert_eq!(pos.y, 0.0);
    }

    #[test]
    fn predict_trajectory_emits_ordered_steps_along_motion_arc() {
        let mut estimator = Estimator::new();
        estimator.push(Point::new(0.0, 0.0), t(0));
        estimator.push(Point::new(10.0, 5.0), t(10));
        estimator.push(Point::new(25.0, 15.0), t(20));

        let trajectory = estimator.predict_trajectory(Duration::from_millis(30), 3);
        assert_eq!(trajectory.len(), 3);
        // Each step must advance progressively forward in X and Y
        assert!(trajectory[0].x > 25.0);
        assert!(trajectory[1].x > trajectory[0].x);
        assert!(trajectory[2].x > trajectory[1].x);
        assert!(trajectory[0].y > 15.0);
        assert!(trajectory[1].y > trajectory[0].y);
        assert!(trajectory[2].y > trajectory[1].y);
    }

    #[test]
    fn predict_future_position_falls_back_cleanly_on_few_samples() {
        let mut estimator = Estimator::new();
        estimator.push(Point::new(10.0, 10.0), t(0));
        estimator.push(Point::new(20.0, 20.0), t(10));

        let predicted = estimator.predict_future_position(Duration::from_millis(10));
        assert!(predicted.is_some());
        let p = predicted.unwrap();
        // 1000 px/sec -> 10px forward
        assert!((p.x - 30.0).abs() < 0.1);
        assert!((p.y - 30.0).abs() < 0.1);
    }
}

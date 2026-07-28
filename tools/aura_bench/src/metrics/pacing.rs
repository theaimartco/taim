//! Cadence, jitter, queue depth, and missed-frame classification.
//!
//! `metrics::frame_time` measures CPU-side per-phase timing;
//! `metrics::latency` measures event-to-present latency for a single
//! frame. Neither answers the question this module does: across a
//! *sequence* of frames, is presentation happening at a stable cadence
//! (as close to the target refresh interval as the frames actually
//! produced allow), or is it drifting/jittering in a way that would be
//! visible as stutter even if every individual frame's CPU cost was
//! within budget?
//!
//! This is the aggregate-over-time counterpart to
//! `aura_core::frame::Feedback` (Phase 6), which this module consumes
//! directly: a [`PacingSamples`] is built from a sequence of
//! [`aura_core::frame::Feedback`] values (as `wgpu::window::
//! frame_feedback::FeedbackSource` would produce for a real run), so a
//! benchmark scene that wires up real presentation feedback gets
//! cadence/jitter/missed-frame numbers for free from the same feedback
//! stream `runtime::frame_scheduler::Scheduler` already consumes.

use aura_core::frame;
use std::time::Duration;

/// One frame's contribution to a pacing measurement: when it was
/// presented (if at all), what it was expected to hit, and how many
/// other frames were already in flight when it was submitted.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PacingSample {
    /// Whether the backend reported this frame as dropped
    /// (see [`frame::Feedback::dropped`]).
    pub dropped: bool,
    /// How far this frame's presentation missed its predicted target,
    /// if both were known (see [`frame::Feedback::target_error`]).
    /// `None` for a dropped frame or one with no known target.
    pub target_error: Option<Duration>,
    /// How many previously submitted frames had not yet resolved
    /// (via presented or dropped feedback) at the moment this frame was
    /// submitted -- the presentation queue depth this frame observed.
    ///
    /// Bounded in practice by
    /// `wgpu::window::compositor::Settings::desired_maximum_frame_latency`:
    /// a queue depth consistently at or above that configured limit
    /// signals the CPU is submitting faster than the presentation
    /// engine can drain, which is a distinct problem from cadence/
    /// jitter (frames arriving on an uneven schedule) even though both
    /// eventually show up as stutter.
    pub queue_depth: usize,
}

impl PacingSample {
    /// Builds a [`PacingSample`] from a real [`frame::Feedback`] value
    /// and the queue depth observed when that frame was submitted.
    ///
    /// Not called anywhere in `aura_bench` itself yet: this tool runs
    /// entirely headlessly with no real compositor to source
    /// `frame::Feedback` from (see [`super::pacing`]'s own module
    /// documentation and `main.rs`'s synthesized-from-CPU-time
    /// fallback). This is the real bridge a future scene driving an
    /// actual windowed compositor (via `wgpu::window::frame_feedback::
    /// FeedbackSource`) would use instead of that fallback; kept and
    /// tested now rather than deferred, since the type it bridges from
    /// (`frame::Feedback`) already exists.
    #[allow(dead_code)]
    pub fn from_feedback(feedback: &frame::Feedback, queue_depth: usize) -> Self {
        Self {
            dropped: feedback.dropped,
            target_error: feedback.target_error,
            queue_depth,
        }
    }
}

/// A sequence of [`PacingSample`]s collected over one measured run.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PacingSamples {
    /// One entry per frame, in presentation order.
    pub frames: Vec<PacingSample>,
}

impl PacingSamples {
    /// Creates an empty sample set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one frame's pacing sample.
    pub fn push(&mut self, sample: PacingSample) {
        self.frames.push(sample);
    }

    /// Records pacing samples for every [`frame::Feedback`] in `feedback`,
    /// as a convenience over calling [`PacingSamples::push`] in a loop.
    /// `queue_depth` is applied uniformly to every sample in this batch;
    /// a caller that tracked a different depth per frame should call
    /// [`PacingSamples::push`] directly instead.
    ///
    /// Not called anywhere in `aura_bench` itself yet -- see
    /// [`PacingSample::from_feedback`]'s own documentation for why.
    #[allow(dead_code)]
    pub fn extend_from_feedback<'a>(
        &mut self,
        feedback: impl IntoIterator<Item = &'a frame::Feedback>,
        queue_depth: usize,
    ) {
        for sample in feedback {
            self.push(PacingSample::from_feedback(sample, queue_depth));
        }
    }

    /// Summarizes the samples into cadence/jitter/missed-frame statistics.
    ///
    /// `target_period` is the nominal interval between frames (e.g.
    /// [`crate::config::RefreshRate::frame_budget`]), used only to
    /// express jitter as a fraction of the target for readability; the
    /// underlying `target_error` values already come from each sample's
    /// own predicted target, not a recomputation against `target_period`.
    pub fn summarize(&self, target_period: Duration) -> PacingSummary {
        let dropped_count = self.frames.iter().filter(|f| f.dropped).count();

        let errors: Vec<Duration> = self.frames.iter().filter_map(|f| f.target_error).collect();

        let missed_count = dropped_count
            + errors
                .iter()
                .filter(|error| **error > Duration::ZERO)
                .count();

        let mean_error = if errors.is_empty() {
            Duration::ZERO
        } else {
            let total: Duration = errors.iter().sum();
            total / errors.len() as u32
        };

        let jitter = if errors.len() < 2 {
            Duration::ZERO
        } else {
            let mean_nanos = mean_error.as_nanos() as f64;

            let variance = errors
                .iter()
                .map(|error| {
                    let diff = error.as_nanos() as f64 - mean_nanos;
                    diff * diff
                })
                .sum::<f64>()
                / errors.len() as f64;

            Duration::from_nanos(variance.sqrt().round() as u64)
        };

        let max_queue_depth = self.frames.iter().map(|f| f.queue_depth).max().unwrap_or(0);

        let mean_queue_depth = if self.frames.is_empty() {
            0.0
        } else {
            self.frames.iter().map(|f| f.queue_depth).sum::<usize>() as f64
                / self.frames.len() as f64
        };

        PacingSummary {
            sample_count: self.frames.len(),
            dropped_count,
            missed_count,
            missed_rate: if self.frames.is_empty() {
                0.0
            } else {
                missed_count as f64 / self.frames.len() as f64
            },
            mean_target_error: mean_error,
            jitter,
            jitter_ratio: if target_period.is_zero() {
                0.0
            } else {
                jitter.as_secs_f64() / target_period.as_secs_f64()
            },
            max_queue_depth,
            mean_queue_depth,
        }
    }
}

/// Cadence/jitter/missed-frame summary of one measured run.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PacingSummary {
    /// Number of frames the summary was computed over.
    pub sample_count: usize,
    /// Number of frames reported as dropped by the backend.
    pub dropped_count: usize,
    /// Number of frames that either dropped or missed their predicted
    /// target (a positive `target_error`).
    pub missed_count: usize,
    /// `missed_count` as a fraction of `sample_count` (`0.0` if empty).
    pub missed_rate: f64,
    /// The mean `target_error` across every frame with a known target,
    /// excluding dropped frames (which have none).
    pub mean_target_error: Duration,
    /// The standard deviation of `target_error` across every frame with
    /// a known target -- how much presentation timing bounces around
    /// its own mean, independent of whether the mean itself is on
    /// target. A stable cadence with a small, consistent offset has low
    /// jitter even if `mean_target_error` is nonzero; an unstable one
    /// alternating between early and late has high jitter even with a
    /// mean near zero.
    pub jitter: Duration,
    /// [`PacingSummary::jitter`] expressed as a fraction of the target
    /// refresh period passed to [`PacingSamples::summarize`], for
    /// comparing jitter across different refresh rates on equal footing
    /// (a jitter of 2ms is a small fraction of a 60Hz frame but a large
    /// one at 120Hz).
    pub jitter_ratio: f64,
    /// The largest [`PacingSample::queue_depth`] observed across every
    /// sample -- the deepest the presentation queue got during this run.
    pub max_queue_depth: usize,
    /// The mean [`PacingSample::queue_depth`] across every sample
    /// (`0.0` if empty).
    pub mean_queue_depth: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on_time() -> PacingSample {
        PacingSample {
            dropped: false,
            target_error: Some(Duration::ZERO),
            queue_depth: 0,
        }
    }

    fn late(by: Duration) -> PacingSample {
        PacingSample {
            dropped: false,
            target_error: Some(by),
            queue_depth: 0,
        }
    }

    fn dropped() -> PacingSample {
        PacingSample {
            dropped: true,
            target_error: None,
            queue_depth: 0,
        }
    }

    fn with_queue_depth(mut sample: PacingSample, depth: usize) -> PacingSample {
        sample.queue_depth = depth;
        sample
    }

    #[test]
    fn empty_samples_summarize_to_zero() {
        let samples = PacingSamples::new();
        let summary = samples.summarize(Duration::from_millis(16));

        assert_eq!(summary.sample_count, 0);
        assert_eq!(summary.missed_rate, 0.0);
        assert_eq!(summary.jitter, Duration::ZERO);
    }

    #[test]
    fn perfectly_on_time_frames_have_zero_missed_rate_and_jitter() {
        let mut samples = PacingSamples::new();
        for _ in 0..10 {
            samples.push(on_time());
        }

        let summary = samples.summarize(Duration::from_millis(16));

        assert_eq!(summary.missed_count, 0);
        assert_eq!(summary.missed_rate, 0.0);
        assert_eq!(summary.jitter, Duration::ZERO);
        assert_eq!(summary.mean_target_error, Duration::ZERO);
    }

    #[test]
    fn a_dropped_frame_counts_as_missed() {
        let mut samples = PacingSamples::new();
        samples.push(on_time());
        samples.push(dropped());

        let summary = samples.summarize(Duration::from_millis(16));

        assert_eq!(summary.dropped_count, 1);
        assert_eq!(summary.missed_count, 1);
        assert_eq!(summary.missed_rate, 0.5);
    }

    #[test]
    fn a_late_frame_counts_as_missed_and_contributes_to_mean_error() {
        let mut samples = PacingSamples::new();
        samples.push(on_time());
        samples.push(late(Duration::from_millis(4)));

        let summary = samples.summarize(Duration::from_millis(16));

        assert_eq!(summary.missed_count, 1);
        assert_eq!(summary.mean_target_error, Duration::from_millis(2));
    }

    #[test]
    fn consistent_lateness_has_low_jitter_despite_a_nonzero_mean_error() {
        let mut samples = PacingSamples::new();
        for _ in 0..10 {
            samples.push(late(Duration::from_millis(2)));
        }

        let summary = samples.summarize(Duration::from_millis(16));

        assert!(summary.mean_target_error > Duration::ZERO);
        assert_eq!(summary.jitter, Duration::ZERO);
    }

    #[test]
    fn alternating_early_and_late_frames_have_high_jitter() {
        let mut samples = PacingSamples::new();
        for i in 0..10 {
            let error = if i % 2 == 0 {
                Duration::ZERO
            } else {
                Duration::from_millis(10)
            };
            samples.push(late(error));
        }

        let summary = samples.summarize(Duration::from_millis(16));

        assert!(summary.jitter > Duration::from_millis(1));
    }

    #[test]
    fn jitter_ratio_scales_with_the_target_period() {
        let mut samples = PacingSamples::new();
        samples.push(late(Duration::ZERO));
        samples.push(late(Duration::from_millis(4)));

        let at_60hz = samples.summarize(Duration::from_secs_f64(1.0 / 60.0));
        let at_120hz = samples.summarize(Duration::from_secs_f64(1.0 / 120.0));

        assert!(at_120hz.jitter_ratio > at_60hz.jitter_ratio);
    }

    #[test]
    fn extend_from_feedback_builds_samples_from_real_frame_feedback_values() {
        let mut samples = PacingSamples::new();

        let feedback = [
            frame::Feedback::dropped(frame::Id::FIRST),
            frame::Feedback::presented(frame::Id::FIRST.next(), std::time::Instant::now(), None),
        ];

        samples.extend_from_feedback(&feedback, 2);

        assert_eq!(samples.frames.len(), 2);
        assert!(samples.frames[0].dropped);
        assert!(!samples.frames[1].dropped);
        assert_eq!(samples.frames[0].queue_depth, 2);
    }

    #[test]
    fn max_queue_depth_reports_the_deepest_observed_value() {
        let mut samples = PacingSamples::new();
        samples.push(with_queue_depth(on_time(), 1));
        samples.push(with_queue_depth(on_time(), 3));
        samples.push(with_queue_depth(on_time(), 2));

        let summary = samples.summarize(Duration::from_millis(16));

        assert_eq!(summary.max_queue_depth, 3);
    }

    #[test]
    fn mean_queue_depth_averages_across_every_sample() {
        let mut samples = PacingSamples::new();
        samples.push(with_queue_depth(on_time(), 0));
        samples.push(with_queue_depth(on_time(), 2));

        let summary = samples.summarize(Duration::from_millis(16));

        assert_eq!(summary.mean_queue_depth, 1.0);
    }

    #[test]
    fn queue_depth_stats_are_zero_for_empty_samples() {
        let samples = PacingSamples::new();
        let summary = samples.summarize(Duration::from_millis(16));

        assert_eq!(summary.max_queue_depth, 0);
        assert_eq!(summary.mean_queue_depth, 0.0);
    }
}

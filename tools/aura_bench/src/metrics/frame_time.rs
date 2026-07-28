//! Per-frame timing: CPU phase breakdown, percentile summaries, and
//! missed-frame classification against a target frame budget.

use std::time::Duration;

/// The CPU-side phases of a single measured frame.
///
/// These names match `phase.md`'s "measurements required" list: diff,
/// event, layout, semantics, paint preparation, upload. Phase 1 only has
/// direct access to `update` (which folds diff/event/layout together
/// today, since `aura_runtime::UserInterface` does not yet split them --
/// that split is Phase 3's job) and `draw`. Semantics and upload timings
/// are recorded as zero until the phases that produce them exist, rather
/// than inventing numbers; see `docs/performance/methodology.md`.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FramePhases {
    /// Time spent in `UserInterface::update` (diff + event dispatch +
    /// layout, currently unseparated).
    pub update: Duration,
    /// Time spent in `UserInterface::draw` (paint preparation).
    pub draw: Duration,
    /// Time spent producing a readback (`Renderer::screenshot`), which
    /// stands in for GPU submission + presentation time until Phase 6
    /// wires up real presentation feedback.
    pub present: Duration,
}

impl FramePhases {
    /// Total CPU time across all recorded phases.
    pub fn total(&self) -> Duration {
        self.update + self.draw + self.present
    }
}

/// A single measured run's collected per-frame samples.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct FrameTimeSamples {
    /// One entry per frame, in the order frames were produced.
    pub frames: Vec<FramePhases>,
}

impl FrameTimeSamples {
    /// Creates an empty sample set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one frame's phase timings.
    pub fn push(&mut self, phases: FramePhases) {
        self.frames.push(phases);
    }

    /// Summarizes the samples into percentiles and a missed-frame rate
    /// against the given frame budget.
    pub fn summarize(&self, frame_budget: Duration) -> FrameTimeSummary {
        let mut totals: Vec<Duration> = self.frames.iter().map(FramePhases::total).collect();
        totals.sort_unstable();

        let missed = totals.iter().filter(|total| **total > frame_budget).count();

        FrameTimeSummary {
            sample_count: totals.len(),
            p50: percentile(&totals, 0.50),
            p95: percentile(&totals, 0.95),
            p99: percentile(&totals, 0.99),
            missed_frame_rate: if totals.is_empty() {
                0.0
            } else {
                missed as f64 / totals.len() as f64
            },
            update_p95: percentile_of(&self.frames, 0.95, |f| f.update),
            draw_p95: percentile_of(&self.frames, 0.95, |f| f.draw),
            present_p95: percentile_of(&self.frames, 0.95, |f| f.present),
        }
    }
}

/// Percentile summary of one measured run, in the shape the versioned
/// report schema stores.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FrameTimeSummary {
    /// Number of frames the summary was computed over.
    pub sample_count: usize,
    /// Median total CPU frame time.
    pub p50: Duration,
    /// 95th percentile total CPU frame time.
    pub p95: Duration,
    /// 99th percentile total CPU frame time.
    pub p99: Duration,
    /// Fraction of frames whose total time exceeded the target frame
    /// budget (see [`crate::config::RefreshRate::frame_budget`]).
    pub missed_frame_rate: f64,
    /// 95th percentile of the `update` phase alone.
    pub update_p95: Duration,
    /// 95th percentile of the `draw` phase alone.
    pub draw_p95: Duration,
    /// 95th percentile of the `present`/readback phase alone.
    pub present_p95: Duration,
}

fn percentile(sorted: &[Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }

    let rank = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

fn percentile_of<T>(frames: &[T], p: f64, select: impl Fn(&T) -> Duration) -> Duration {
    let mut values: Vec<Duration> = frames.iter().map(select).collect();
    values.sort_unstable();
    percentile(&values, p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_are_monotonic() {
        let mut samples = FrameTimeSamples::new();

        for millis in 1..=100 {
            samples.push(FramePhases {
                update: Duration::from_millis(millis),
                draw: Duration::from_millis(1),
                present: Duration::from_millis(1),
            });
        }

        let summary = samples.summarize(Duration::from_millis(13));

        assert!(summary.p50 <= summary.p95);
        assert!(summary.p95 <= summary.p99);
        assert_eq!(summary.sample_count, 100);
    }

    #[test]
    fn missed_frame_rate_counts_frames_over_budget() {
        let mut samples = FrameTimeSamples::new();

        for _ in 0..8 {
            samples.push(FramePhases {
                update: Duration::from_millis(5),
                draw: Duration::from_millis(1),
                present: Duration::from_millis(1),
            });
        }

        for _ in 0..2 {
            samples.push(FramePhases {
                update: Duration::from_millis(20),
                draw: Duration::from_millis(1),
                present: Duration::from_millis(1),
            });
        }

        let summary = samples.summarize(Duration::from_millis(13));

        assert_eq!(summary.missed_frame_rate, 0.2);
    }

    #[test]
    fn empty_samples_summarize_to_zero() {
        let samples = FrameTimeSamples::new();
        let summary = samples.summarize(Duration::from_millis(13));

        assert_eq!(summary.sample_count, 0);
        assert_eq!(summary.p50, Duration::ZERO);
        assert_eq!(summary.missed_frame_rate, 0.0);
    }
}

//! Per-frame timing: CPU phase breakdown, percentile summaries, and
//! missed-frame classification against a target frame budget.

use std::time::Duration;

/// The CPU-side phases of a single measured frame.
///
/// These names match `phase.md`'s "measurements required" list: diff,
/// event, layout, semantics, paint preparation, upload. Phase 1 only has
/// direct access to `build` (view reconstruction + diff + layout),
/// `update` (event dispatch), and `draw`. Semantics and upload timings
/// are recorded as zero until the phases that produce them exist, rather
/// than inventing numbers; see `docs/performance/methodology.md`.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FramePhases {
    /// Time spent building the view and reconciling/laying out the UI.
    pub build: Duration,
    /// Time spent in `UserInterface::update` (event dispatch).
    pub update: Duration,
    /// Time spent in `UserInterface::draw` and returning its retained cache.
    pub draw: Duration,
    /// Time spent producing a readback (`Renderer::screenshot`).
    ///
    /// This is **harness overhead, not framework cost**. A real
    /// application never reads the framebuffer back to host memory; the
    /// readback exists only so a headless scene can be forced to
    /// rasterize. On a 1280x800 viewport it costs a near-constant
    /// ~2.3 ms regardless of what the scene contains, which is roughly
    /// two hundred times the framework's own per-frame cost.
    ///
    /// It is therefore deliberately excluded from [`FramePhases::total`]
    /// and reported separately by [`FramePhases::harness_overhead`]. It
    /// stays measured, rather than dropped, so the gap between it and a
    /// real presentation path remains visible until Phase 6 wires up
    /// genuine presentation feedback.
    pub present: Duration,
}

impl FramePhases {
    /// Total measured CPU time in the UI pipeline for this frame.
    ///
    /// It includes benchmark-scene view construction as well as TAIM's
    /// diff/layout/update/draw. It excludes [`FramePhases::present`], which measures the
    /// harness's readback rather than anything a shipped application
    /// would pay for -- see that field's documentation.
    pub fn total(&self) -> Duration {
        self.build + self.update + self.draw
    }

    /// Time spent in harness-only work that no real application pays.
    ///
    /// Reported alongside [`FramePhases::total`] so a run's output shows
    /// what was excluded and why the two numbers differ by so much.
    pub fn harness_overhead(&self) -> Duration {
        self.present
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
            build_p95: percentile_of(&self.frames, 0.95, |f| f.build),
            update_p95: percentile_of(&self.frames, 0.95, |f| f.update),
            draw_p95: percentile_of(&self.frames, 0.95, |f| f.draw),
            present_p95: percentile_of(&self.frames, 0.95, FramePhases::harness_overhead),
        }
    }
}

/// Percentile summary of one measured run, in the shape the versioned
/// report schema stores.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FrameTimeSummary {
    /// Number of frames the summary was computed over.
    pub sample_count: usize,
    /// Median framework CPU frame time (`build` + `update` + `draw`).
    pub p50: Duration,
    /// 95th percentile framework CPU frame time (`update` + `draw`).
    pub p95: Duration,
    /// 99th percentile framework CPU frame time (`update` + `draw`).
    pub p99: Duration,
    /// Fraction of frames whose framework time exceeded the target frame
    /// budget (see [`crate::config::RefreshRate::frame_budget`]).
    pub missed_frame_rate: f64,
    /// 95th percentile of view construction, tree diff, and layout.
    pub build_p95: Duration,
    /// 95th percentile of event dispatch alone.
    pub update_p95: Duration,
    /// 95th percentile of the `draw` phase alone.
    pub draw_p95: Duration,
    /// 95th percentile of the harness-only readback phase.
    ///
    /// Excluded from [`FrameTimeSummary::p50`] and friends; see
    /// [`FramePhases::present`].
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
                build: Duration::ZERO,
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
                build: Duration::ZERO,
                update: Duration::from_millis(5),
                draw: Duration::from_millis(1),
                present: Duration::from_millis(1),
            });
        }

        for _ in 0..2 {
            samples.push(FramePhases {
                build: Duration::ZERO,
                update: Duration::from_millis(20),
                draw: Duration::from_millis(1),
                present: Duration::from_millis(1),
            });
        }

        let summary = samples.summarize(Duration::from_millis(13));

        assert_eq!(summary.missed_frame_rate, 0.2);
    }

    #[test]
    fn total_excludes_the_harness_readback() {
        // Regression guard. `present` is a `Renderer::screenshot`
        // readback that costs a near-constant ~2.3 ms no matter what the
        // scene contains. Folding it into `total` made every scene --
        // from an idle shell to ten thousand rows -- report the same
        // frame time, because the constant drowned the real work by
        // roughly two hundred to one. If someone re-adds it here, every
        // published performance number silently becomes a measurement of
        // the benchmark harness instead of the framework.
        let phases = FramePhases {
            build: Duration::from_micros(3),
            update: Duration::from_micros(5),
            draw: Duration::from_micros(7),
            present: Duration::from_millis(2),
        };

        assert_eq!(phases.total(), Duration::from_micros(15));
        assert_eq!(phases.harness_overhead(), Duration::from_millis(2));
    }

    #[test]
    fn scenes_with_different_work_produce_different_totals() {
        // The symptom the exclusion above exists to prevent: two scenes
        // whose framework cost differs by 2.4x must not summarize to the
        // same frame time just because they share a readback cost.
        let mut idle = FrameTimeSamples::new();
        let mut busy = FrameTimeSamples::new();

        for _ in 0..64 {
            idle.push(FramePhases {
                build: Duration::from_micros(1),
                update: Duration::from_micros(1),
                draw: Duration::from_micros(4),
                present: Duration::from_millis(2),
            });
            busy.push(FramePhases {
                build: Duration::from_micros(5),
                update: Duration::from_micros(5),
                draw: Duration::from_micros(7),
                present: Duration::from_millis(2),
            });
        }

        let budget = Duration::from_millis(16);

        assert!(
            busy.summarize(budget).p95 > idle.summarize(budget).p95,
            "a busier scene must report a larger frame time"
        );
    }

    #[test]
    fn empty_samples_summarize_to_zero() {
        let samples = FrameTimeSamples::new();
        let summary = samples.summarize(Duration::from_millis(13));

        assert_eq!(summary.sample_count, 0);
        assert_eq!(summary.p50, Duration::ZERO);
        assert_eq!(summary.missed_frame_rate, 0.0);
    }

    #[test]
    fn build_cost_is_included_in_frame_budget_and_reported_separately() {
        let mut samples = FrameTimeSamples::new();
        samples.push(FramePhases {
            build: Duration::from_millis(12),
            update: Duration::from_millis(2),
            draw: Duration::from_millis(2),
            present: Duration::from_millis(20),
        });

        let summary = samples.summarize(Duration::from_millis(15));
        assert_eq!(summary.p95, Duration::from_millis(16));
        assert_eq!(summary.build_p95, Duration::from_millis(12));
        assert_eq!(summary.missed_frame_rate, 1.0);
    }
}

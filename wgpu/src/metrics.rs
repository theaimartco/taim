//! Per-frame GPU hot-path counters.
//!
//! Mirrors `aura_runtime::diagnostics::Counters`'s exact shape and
//! zero-cost-when-disabled contract, for the same reason: phase.md's
//! Phase 5 optimization order starts with "instrument current
//! primitive, batch, upload, atlas miss, pipeline switch, and draw
//! counts" *before* any allocation/batching/persistence work, so later
//! steps have a concrete "how much work did we actually avoid" number
//! rather than only a wall-clock time (which `aura_debug`'s existing
//! `prepare`/`render` spans already provide, but do not count anything
//! by themselves).
//!
//! [`Counters`] is reset once per frame (unlike
//! `aura_runtime::diagnostics::Counters`, which accumulates for the
//! lifetime of whatever holds it): a GPU frame's cost is inherently
//! per-frame -- carrying counts across frames would conflate "this
//! frame did N draw calls" with "N draw calls have happened since the
//! renderer was created," and the former is what every consumer of this
//! module (benchmarks, the exit gate's own metrics) actually needs.

use std::fmt;

#[cfg(feature = "profiling")]
use std::sync::atomic::{AtomicU32, Ordering};

/// Which aspect of the GPU hot path a counter tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Metric {
    /// A primitive (quad, triangle, image, text, shader) was prepared
    /// (i.e. its data was written to a GPU buffer this frame).
    Primitive,
    /// A batch of same-kind primitives was rendered in one draw call.
    Batch,
    /// A buffer upload (`wgpu::util::StagingBelt::write_buffer` call)
    /// occurred.
    Upload,
    /// An image or glyph atlas lookup missed its cache and required a
    /// new allocation/rasterization/decode.
    AtlasMiss,
    /// A render pipeline was bound (`wgpu::RenderPass::set_pipeline`),
    /// counted so a frame that alternates rapidly between pipelines
    /// (state-switch heavy) is distinguishable from one that stays on a
    /// single pipeline for a large batch.
    PipelineSwitch,
    /// A draw call (`wgpu::RenderPass::draw`/`draw_indexed`) was
    /// recorded.
    Draw,
}

#[cfg(feature = "profiling")]
impl Metric {
    const COUNT: usize = 6;

    const fn index(self) -> usize {
        match self {
            Metric::Primitive => 0,
            Metric::Batch => 1,
            Metric::Upload => 2,
            Metric::AtlasMiss => 3,
            Metric::PipelineSwitch => 4,
            Metric::Draw => 5,
        }
    }
}

/// A set of per-[`Metric`] counters for a single frame.
///
/// Compiles to true zero-cost when the `profiling` feature is disabled:
/// [`Counters::increment`] and [`Counters::get`] become no-ops (`get`
/// always returns `0`) rather than atomic operations gated by a runtime
/// check.
pub struct Counters {
    #[cfg(feature = "profiling")]
    counts: [AtomicU32; Metric::COUNT],
}

impl Counters {
    /// Creates a fresh set of [`Counters`], all at zero.
    pub const fn new() -> Self {
        Self {
            #[cfg(feature = "profiling")]
            counts: [
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
            ],
        }
    }

    /// Increments the counter for the given [`Metric`] by one.
    ///
    /// Compiles out entirely when the `profiling` feature is disabled.
    #[cfg(feature = "profiling")]
    pub fn increment(&self, metric: Metric) {
        let _ = self.counts[metric.index()].fetch_add(1, Ordering::Relaxed);
    }

    /// Increments the counter for the given [`Metric`] by one.
    ///
    /// A no-op when the `profiling` feature is disabled.
    #[cfg(not(feature = "profiling"))]
    pub fn increment(&self, _metric: Metric) {}

    /// Increments the counter for the given [`Metric`] by `amount`.
    ///
    /// Compiles out entirely when the `profiling` feature is disabled.
    #[cfg(feature = "profiling")]
    pub fn increment_by(&self, metric: Metric, amount: u32) {
        let _ = self.counts[metric.index()].fetch_add(amount, Ordering::Relaxed);
    }

    /// Increments the counter for the given [`Metric`] by `amount`.
    ///
    /// A no-op when the `profiling` feature is disabled.
    #[cfg(not(feature = "profiling"))]
    pub fn increment_by(&self, _metric: Metric, _amount: u32) {}

    /// Returns how many times the given [`Metric`] fired this frame.
    ///
    /// Always returns `0` when the `profiling` feature is disabled,
    /// since nothing was ever counted; callers must not treat a `0` here
    /// as "this metric never happened" without first checking
    /// [`Counters::is_active`].
    #[cfg(feature = "profiling")]
    pub fn get(&self, metric: Metric) -> u32 {
        self.counts[metric.index()].load(Ordering::Relaxed)
    }

    /// Returns how many times the given [`Metric`] fired this frame.
    ///
    /// Always returns `0` when the `profiling` feature is disabled.
    #[cfg(not(feature = "profiling"))]
    pub fn get(&self, _metric: Metric) -> u32 {
        0
    }

    /// Resets every counter to zero, for the start of a new frame.
    #[cfg(feature = "profiling")]
    pub fn reset(&self) {
        for count in &self.counts {
            count.store(0, Ordering::Relaxed);
        }
    }

    /// A no-op when the `profiling` feature is disabled.
    #[cfg(not(feature = "profiling"))]
    pub fn reset(&self) {}

    /// Whether this build actually counts metrics (the `profiling`
    /// feature is enabled), so callers can distinguish "0 because
    /// disabled" from "0 because it truly never happened this frame."
    pub const fn is_active() -> bool {
        cfg!(feature = "profiling")
    }
}

impl Default for Counters {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Counters {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("Counters");

        let _ = debug.field("active", &Self::is_active());

        for metric in [
            Metric::Primitive,
            Metric::Batch,
            Metric::Upload,
            Metric::AtlasMiss,
            Metric::PipelineSwitch,
            Metric::Draw,
        ] {
            let _ = debug.field(&format!("{metric:?}"), &self.get(metric));
        }

        debug.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_counters_set_starts_at_zero() {
        let counters = Counters::new();

        assert_eq!(counters.get(Metric::Draw), 0);
        assert_eq!(counters.get(Metric::Batch), 0);
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn increment_advances_only_the_targeted_metric() {
        let counters = Counters::new();

        counters.increment(Metric::Draw);
        counters.increment(Metric::Draw);
        counters.increment(Metric::Batch);

        assert_eq!(counters.get(Metric::Draw), 2);
        assert_eq!(counters.get(Metric::Batch), 1);
        assert_eq!(counters.get(Metric::Upload), 0);
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn increment_by_adds_the_given_amount() {
        let counters = Counters::new();

        counters.increment_by(Metric::Primitive, 42);

        assert_eq!(counters.get(Metric::Primitive), 42);
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn reset_zeroes_every_metric() {
        let counters = Counters::new();

        counters.increment(Metric::Draw);
        counters.increment(Metric::AtlasMiss);
        counters.reset();

        assert_eq!(counters.get(Metric::Draw), 0);
        assert_eq!(counters.get(Metric::AtlasMiss), 0);
    }

    #[cfg(not(feature = "profiling"))]
    #[test]
    fn increment_is_a_no_op_when_profiling_is_disabled() {
        let counters = Counters::new();

        counters.increment(Metric::Draw);
        counters.increment(Metric::Draw);

        assert_eq!(counters.get(Metric::Draw), 0);
        assert!(!Counters::is_active());
    }

    #[test]
    fn debug_formatting_does_not_panic() {
        let counters = Counters::new();

        let _ = format!("{counters:?}");
    }
}

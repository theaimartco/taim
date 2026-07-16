//! Phase counters for the widget tree pipeline.
//!
//! [`Counters`] tracks how many times each phase of a frame
//! (diff/event/layout/semantics/paint-preparation/upload) ran, so later
//! phases' incremental work (Phase 3's reconciliation, Phase 4's damage
//! tracking) can be measured against a concrete "how much work did we
//! actually avoid" number instead of only a wall-clock time.
//!
//! Phase 2 only introduces the counters and their accessors; nothing in
//! `UserInterface` increments them yet ("without yet changing all
//! behavior" is this phase's explicit objective). The phase that adds
//! reconciliation, incremental layout, etc. is expected to call
//! [`Counters::increment`] at the point it can prove a phase ran (or was
//! skipped).
//!
//! # Zero-cost when disabled
//!
//! Every counter increment is compiled out entirely when the `profiling`
//! feature is disabled: [`Counters::increment`] and [`Counters::get`]
//! become no-ops (`get` always returns `0`) rather than atomic
//! operations gated by a runtime check, so a non-profiling release
//! build pays nothing for this module's existence.

use std::fmt;

#[cfg(feature = "profiling")]
use std::sync::atomic::{AtomicU64, Ordering};

/// Which phase of the frame pipeline a counter tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Phase {
    /// Widget tree reconciliation (`Widget::diff`).
    Diff,
    /// Runtime event dispatch (`Widget::update`).
    Event,
    /// Layout computation (`Widget::layout`).
    Layout,
    /// Semantic tree computation (see [`iced_core::semantics`]).
    Semantics,
    /// Paint preparation (`Widget::draw`).
    PaintPreparation,
    /// Resource upload to the renderer (e.g. glyph/image atlas writes).
    Upload,
    /// A [`crate::invalidation::Aggregator`] gave up tracking precise
    /// damage regions and fell back to a full-surface redraw because
    /// more than [`crate::invalidation::Aggregator::MAX_REGIONS`]
    /// distinct areas accumulated in one frame.
    DamageFallback,
}

#[cfg(feature = "profiling")]
impl Phase {
    const COUNT: usize = 7;

    const fn index(self) -> usize {
        match self {
            Phase::Diff => 0,
            Phase::Event => 1,
            Phase::Layout => 2,
            Phase::Semantics => 3,
            Phase::PaintPreparation => 4,
            Phase::Upload => 5,
            Phase::DamageFallback => 6,
        }
    }
}

/// A set of per-[`Phase`] counters.
///
/// Counting is additive only: a [`Counters`] tracks how many times a
/// phase ran over the lifetime of whatever holds it (typically one
/// `UserInterface`, though nothing in this module assumes that); reset
/// to zero by replacing it with [`Counters::new`], there is no in-place
/// reset method, since resetting a live counter silently is a common
/// source of misleading diagnostics.
pub struct Counters {
    #[cfg(feature = "profiling")]
    counts: [AtomicU64; Phase::COUNT],
}

impl Counters {
    /// Creates a fresh set of [`Counters`], all at zero.
    pub const fn new() -> Self {
        Self {
            #[cfg(feature = "profiling")]
            counts: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
        }
    }

    /// Increments the counter for the given [`Phase`] by one.
    ///
    /// Compiles out entirely when the `profiling` feature is disabled.
    #[cfg(feature = "profiling")]
    pub fn increment(&self, phase: Phase) {
        let _ = self.counts[phase.index()].fetch_add(1, Ordering::Relaxed);
    }

    /// Increments the counter for the given [`Phase`] by one.
    ///
    /// A no-op when the `profiling` feature is disabled.
    #[cfg(not(feature = "profiling"))]
    pub fn increment(&self, _phase: Phase) {}

    /// Returns how many times the given [`Phase`] has run.
    ///
    /// Always returns `0` when the `profiling` feature is disabled,
    /// since nothing was ever counted; callers must not treat a `0` here
    /// as "this phase never ran" without first checking
    /// [`Counters::is_active`].
    #[cfg(feature = "profiling")]
    pub fn get(&self, phase: Phase) -> u64 {
        self.counts[phase.index()].load(Ordering::Relaxed)
    }

    /// Returns how many times the given [`Phase`] has run.
    ///
    /// Always returns `0` when the `profiling` feature is disabled.
    #[cfg(not(feature = "profiling"))]
    pub fn get(&self, _phase: Phase) -> u64 {
        0
    }

    /// Whether this build actually counts phases (the `profiling`
    /// feature is enabled), so callers can distinguish "0 because
    /// disabled" from "0 because it truly never ran."
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

        for phase in [
            Phase::Diff,
            Phase::Event,
            Phase::Layout,
            Phase::Semantics,
            Phase::PaintPreparation,
            Phase::Upload,
            Phase::DamageFallback,
        ] {
            let _ = debug.field(&format!("{phase:?}"), &self.get(phase));
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

        assert_eq!(counters.get(Phase::Diff), 0);
        assert_eq!(counters.get(Phase::Layout), 0);
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn increment_advances_only_the_targeted_phase() {
        let counters = Counters::new();

        counters.increment(Phase::Layout);
        counters.increment(Phase::Layout);
        counters.increment(Phase::Event);

        assert_eq!(counters.get(Phase::Layout), 2);
        assert_eq!(counters.get(Phase::Event), 1);
        assert_eq!(counters.get(Phase::Diff), 0);
    }

    #[cfg(not(feature = "profiling"))]
    #[test]
    fn increment_is_a_no_op_when_profiling_is_disabled() {
        let counters = Counters::new();

        counters.increment(Phase::Layout);
        counters.increment(Phase::Layout);

        assert_eq!(counters.get(Phase::Layout), 0);
        assert!(!Counters::is_active());
    }

    #[test]
    fn debug_formatting_does_not_panic() {
        let counters = Counters::new();

        let _ = format!("{counters:?}");
    }
}

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
//! # Counters and traces
//!
//! [`Counters`] answers *how many times*; `taim_profiler` answers *how
//! long, and why*. They are separate on purpose — a counter is an atomic
//! add with no timestamp and no buffer, cheap enough to leave on in a
//! benchmark, while a trace costs a clock read and a ring slot.
//!
//! [`Trace`] bridges them: it maps this module's [`Phase`] onto the
//! profiler's, so an overlay reading the trace and a benchmark reading
//! the counters describe the same frame in the same vocabulary. Phase
//! 15's exit gate — *"overlays agree with benchmark counters"* — is that
//! correspondence, and it is why the mapping lives in one place rather
//! than being re-derived by each consumer.
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
    /// Semantic tree computation (see [`taim_core::semantics`]).
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

/// Records this module's [`Phase`]s into an `taim_profiler` trace.
///
/// Held by a caller that wants timings as well as counts. Cloning is
/// cheap; every clone shares one buffer.
///
/// # Why the mapping is not one-to-one
///
/// [`Phase`] is the *runtime pipeline's* vocabulary and
/// [`taim_profiler::Phase`] is the *frame's*. They mostly agree, and
/// where they do not the difference is real rather than cosmetic:
///
/// - [`Phase::PaintPreparation`] maps to [`taim_profiler::Phase::Paint`]:
///   the profiler does not distinguish preparing a draw from issuing it,
///   because a developer reading a flame chart cannot act on the
///   distinction.
/// - [`Phase::Upload`] maps to [`taim_profiler::Phase::Submit`]: an atlas
///   write is work handed to the GPU.
/// - [`Phase::DamageFallback`] is **not** a phase at all. It is the
///   record of the damage tracker giving up, which is a *cause* — see
///   [`Trace::damage_fallback`].
///
/// The match is exhaustive with no catch-all, deliberately: [`Phase`] is
/// `#[non_exhaustive]` to the outside world, but this module defines it,
/// so adding a phase here **fails to compile** until it is mapped. A `_`
/// arm would let a new phase silently record nothing, which is the same
/// class of bug as the mapping being wrong — and harder to notice.
#[derive(Debug, Clone)]
pub struct Trace {
    collector: taim_profiler::Collector,
}

impl Trace {
    /// A trace recording into `collector`.
    pub fn new(collector: taim_profiler::Collector) -> Self {
        Self { collector }
    }

    /// The collector being recorded into.
    pub fn collector(&self) -> &taim_profiler::Collector {
        &self.collector
    }

    /// The profiler phase corresponding to `phase`, if there is one.
    ///
    /// `None` for [`Phase::DamageFallback`], which is a cause rather than
    /// a span of time.
    pub fn phase_of(phase: Phase) -> Option<taim_profiler::Phase> {
        match phase {
            Phase::Diff => Some(taim_profiler::Phase::Diff),
            Phase::Event => Some(taim_profiler::Phase::Input),
            Phase::Layout => Some(taim_profiler::Phase::Layout),
            Phase::Semantics => Some(taim_profiler::Phase::Semantics),
            Phase::PaintPreparation => Some(taim_profiler::Phase::Paint),
            Phase::Upload => Some(taim_profiler::Phase::Submit),
            Phase::DamageFallback => None,
        }
    }

    /// Marks the start of `phase`.
    pub fn begin(&self, phase: Phase) {
        let Some(phase) = Self::phase_of(phase) else {
            return;
        };

        let at = self.collector.now();

        self.collector
            .record(|| taim_profiler::Event::PhaseBegin { phase, at });
    }

    /// Marks the end of `phase`.
    pub fn end(&self, phase: Phase) {
        let Some(phase) = Self::phase_of(phase) else {
            return;
        };

        let at = self.collector.now();

        self.collector
            .record(|| taim_profiler::Event::PhaseEnd { phase, at });
    }

    /// Opens a span for `phase`, closed when the returned [`Span`] drops.
    ///
    /// Prefer this to a [`Trace::begin`]/[`Trace::end`] pair anywhere the
    /// scope has more than one exit. `UserInterface::draw` returns early
    /// when there is no overlay; a hand-written `end` after that point is
    /// simply not reached, and the span stays open — silently charging the
    /// rest of the frame, and every frame after it, to `Paint`. A leaked
    /// span does not fail, it lies.
    pub fn span(&self, phase: Phase) -> Span {
        self.begin(phase);

        Span {
            trace: self.clone(),
            phase,
        }
    }

    /// Opens a frame, closed when the returned [`Frame`] drops.
    ///
    /// A frame is not a [`Phase`] and cannot be one: phases are the *parts*
    /// of a frame, and a frame that was also a phase would be counted
    /// inside itself. It is also not something this crate can infer —
    /// `UserInterface` has no idea whether `update` and `draw` belong to
    /// the same frame or to two, because that is the event loop's
    /// decision. So the boundary is the caller's to declare, and this is
    /// how.
    ///
    /// Without it, a trace is a list of phase spans with nothing to group
    /// them: every per-frame statistic — p95, missed-frame ratio, the
    /// unattributed residue — needs to know where one frame stops.
    pub fn frame(&self, number: u64) -> Frame {
        let at = self.collector.now();

        self.collector
            .record(|| taim_profiler::Event::FrameBegin { frame: number, at });

        Frame {
            trace: self.clone(),
            number,
        }
    }

    /// Records that the damage tracker fell back to a full redraw, and
    /// the frame it cost.
    ///
    /// A [`taim_profiler::Event::Missed`] rather than a phase span: this
    /// is the *explanation* for a frame's cost, and recording it as a
    /// diagnostic is what keeps it in the buffer when telemetry is being
    /// dropped under pressure — which is exactly when a damage fallback
    /// is most likely and most worth knowing about.
    pub fn damage_fallback(&self, frame: u64, over: taim_profiler::Nanos) {
        self.collector.record(|| taim_profiler::Event::Missed {
            frame,
            over,
            cause: taim_profiler::Cause::Phase(taim_profiler::Phase::Damage),
        });
    }
}

/// An open frame, closed when it drops.
///
/// Owns a [`Trace`] for the same reason [`Span`] does.
#[derive(Debug)]
#[must_use = "a frame closes when it drops; dropping it immediately measures nothing"]
pub struct Frame {
    trace: Trace,
    number: u64,
}

impl Frame {
    /// Which frame this is.
    pub fn number(&self) -> u64 {
        self.number
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        let at = self.trace.collector.now();

        self.trace
            .collector
            .record(|| taim_profiler::Event::FrameEnd {
                frame: self.number,
                at,
            });
    }
}

/// An open phase, closed when it drops.
///
/// Owns a [`Trace`] rather than borrowing one: a borrow would conflict
/// with the destructuring `&mut self` that the methods being measured
/// need, and a [`Trace`] is a shared handle, so cloning it costs an
/// atomic increment.
#[derive(Debug)]
#[must_use = "a span closes when it drops; dropping it immediately measures nothing"]
pub struct Span {
    trace: Trace,
    phase: Phase,
}

impl Drop for Span {
    fn drop(&mut self) {
        self.trace.end(self.phase);
    }
}

#[cfg(test)]
mod trace_tests {
    use super::*;

    #[test]
    fn every_counted_phase_maps_onto_a_traced_one() {
        // The exit gate's "overlays agree with benchmark counters" rests
        // on this: a phase a benchmark counts but a trace cannot name is a
        // number with no bar on the flame chart.
        for phase in [
            Phase::Diff,
            Phase::Event,
            Phase::Layout,
            Phase::Semantics,
            Phase::PaintPreparation,
            Phase::Upload,
        ] {
            assert!(
                Trace::phase_of(phase).is_some(),
                "{phase:?} is counted but cannot be traced"
            );
        }
    }

    #[test]
    fn a_damage_fallback_is_a_cause_not_a_phase() {
        // It is the record of the damage tracker giving up. Charging it
        // as a span would put a bar on the chart for something that took
        // no time and explains everything else.
        assert_eq!(Trace::phase_of(Phase::DamageFallback), None);
    }

    #[test]
    fn distinct_counted_phases_stay_distinct_when_traced() {
        // A mapping that collapsed two phases onto one would make a
        // regression in either look like a regression in both.
        let mapped: Vec<_> = [Phase::Diff, Phase::Event, Phase::Layout, Phase::Semantics]
            .iter()
            .filter_map(|phase| Trace::phase_of(*phase))
            .collect();

        let mut unique = mapped.clone();
        unique.sort();
        unique.dedup();

        assert_eq!(mapped.len(), unique.len(), "{mapped:?} collapsed");
    }

    #[test]
    fn a_traced_phase_spans_from_begin_to_end() {
        let trace = Trace::new(taim_profiler::Collector::enabled());

        trace.begin(Phase::Layout);
        trace.end(Phase::Layout);

        let events = trace.collector().events();

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].phase(), Some(taim_profiler::Phase::Layout));
        assert!(
            events[1].at() >= events[0].at(),
            "a phase cannot end before it began"
        );
    }

    #[test]
    fn a_disabled_collector_records_nothing() {
        let trace = Trace::new(taim_profiler::Collector::disabled());

        trace.begin(Phase::Layout);
        trace.end(Phase::Layout);
        trace.damage_fallback(1, 5_000_000);

        assert!(trace.collector().is_empty());
    }

    #[test]
    fn a_span_closes_on_an_early_return() {
        // The hazard this exists for: a hand-written `end` after an early
        // return is not reached, and the span stays open -- silently
        // charging the rest of the frame, and every frame after it, to
        // whichever phase leaked.
        let trace = Trace::new(taim_profiler::Collector::enabled());

        fn returns_early(trace: &Trace, early: bool) {
            let _span = trace.span(Phase::PaintPreparation);

            if early {
                return;
            }

            std::hint::black_box(());
        }

        returns_early(&trace, true);

        let events = trace.collector().events();

        assert_eq!(events.len(), 2, "the span must close on the early path");
        assert!(matches!(
            events[1],
            taim_profiler::Event::PhaseEnd {
                phase: taim_profiler::Phase::Paint,
                ..
            }
        ));
    }

    #[test]
    fn a_span_closes_when_a_panic_unwinds_through_it() {
        // A panic mid-frame must not leave the profiler believing `Paint`
        // never ended -- the next trace read would be nonsense.
        let trace = Trace::new(taim_profiler::Collector::enabled());

        let panicked = std::panic::catch_unwind({
            let trace = trace.clone();

            move || {
                let _span = trace.span(Phase::Layout);

                panic!("mid-frame");
            }
        });

        assert!(panicked.is_err());

        let events = trace.collector().events();

        assert_eq!(events.len(), 2, "the span must close while unwinding");
    }

    #[test]
    fn a_frame_brackets_the_phases_inside_it() {
        // Without a frame boundary, a trace is a list of phase spans with
        // nothing to group them -- and every per-frame statistic needs to
        // know where one frame stops.
        let trace = Trace::new(taim_profiler::Collector::enabled());

        {
            let _frame = trace.frame(7);
            let _layout = trace.span(Phase::Layout);
        }

        let events = trace.collector().events();

        assert!(matches!(
            events.first(),
            Some(taim_profiler::Event::FrameBegin { frame: 7, .. })
        ));
        assert!(
            matches!(
                events.last(),
                Some(taim_profiler::Event::FrameEnd { frame: 7, .. })
            ),
            "the frame must close after the phases inside it: {events:?}"
        );
    }

    #[test]
    fn a_frame_is_not_a_phase() {
        // A frame that was also a phase would be counted inside itself,
        // and its bar would be the sum of every other bar plus itself.
        let trace = Trace::new(taim_profiler::Collector::enabled());

        drop(trace.frame(1));

        for event in trace.collector().events() {
            assert_eq!(
                event.phase(),
                None,
                "a frame boundary must not claim a phase: {event:?}"
            );
        }
    }

    #[test]
    fn a_disabled_collector_records_no_frames() {
        let trace = Trace::new(taim_profiler::Collector::disabled());

        drop(trace.frame(1));

        assert!(trace.collector().is_empty());
    }

    #[test]
    fn a_damage_fallback_survives_pressure() {
        // It is a diagnostic, so it outlives the telemetry around it --
        // and a fallback is most likely exactly when the buffer is
        // overflowing.
        let trace = Trace::new(taim_profiler::Collector::with_capacity(4, true));

        trace.damage_fallback(1, 5_000_000);

        for _ in 0..100 {
            trace.begin(Phase::Layout);
            trace.end(Phase::Layout);
        }

        assert!(
            trace
                .collector()
                .events()
                .iter()
                .any(|event| matches!(event, taim_profiler::Event::Missed { frame: 1, .. })),
            "the explanation must outlive the measurements"
        );
    }
}

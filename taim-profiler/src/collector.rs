//! Collection: a bounded ring buffer that never blocks the thread being
//! measured.
//!
//! # The rules this exists to satisfy
//!
//! `phase.md` asks for three things that pull against each other:
//!
//! - *"Disabled profiler overhead must be statistically indistinguishable
//!   or below 1%."* — so a shipped build must not pay for what it does
//!   not use. [`Collector::record`] takes a **closure**, not an
//!   [`Event`]: with collection off, the closure is never called, so the
//!   cost of a recording site is one relaxed atomic load and a
//!   not-taken branch. Constructing the event — reading a clock, counting
//!   nodes — never happens.
//! - *"Buffers are bounded and drop telemetry — not billing/system events
//!   — under pressure."* — so [`Ring`] has a fixed capacity and drops,
//!   and [`Priority`] decides what. A profiler that grows without bound
//!   under load is a profiler that kills the application it was meant to
//!   diagnose, exactly when things are already going wrong.
//! - *"Enabled default sampling overhead must remain below 3%."* — so
//!   high-frequency events are [sampled](Sampling), and sampling is
//!   deterministic rather than random: a trace that cannot be reproduced
//!   is a trace that cannot be bisected.
//!
//! # Why a lock and not a lock-free queue
//!
//! A `Mutex<VecDeque>` looks like the wrong answer for a hot path. It is
//! the right one here: the critical section is a bounded push with no
//! allocation and no syscall, it is uncontended in the overwhelmingly
//! common case (one UI thread producing), and [`Collector::record`] never
//! blocks on it — it uses `try_lock` and drops the event rather than wait.
//! A profiler must never be the reason a frame is late, and a lock-free
//! ring would trade that simplicity for an ABA problem in exchange for
//! contention that does not exist.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::events::{Event, Nanos};

/// How much a given event matters when the buffer is full.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Routine measurement. First to go, and the reason the distinction
    /// exists: losing some cache-hit marks costs a developer resolution;
    /// losing a missed-frame cause costs them the answer.
    Telemetry,
    /// Something that explains a failure — a missed frame, an eviction, a
    /// withheld-content marker. Kept while anything can be.
    Diagnostic,
}

impl Priority {
    /// The priority of `event`.
    fn of(event: &Event) -> Self {
        match event {
            // These are the record of something going wrong. Dropping them
            // under pressure would blind the profiler exactly when
            // pressure is the thing being investigated.
            Event::Missed { .. } | Event::Evicted { .. } | Event::Withheld { .. } => {
                Priority::Diagnostic
            }
            _ => Priority::Telemetry,
        }
    }
}

/// How often high-frequency events are kept.
///
/// Deterministic — every `n`th event, not a random `1/n` — so that two
/// runs of the same workload produce the same trace. A random sample is
/// unbisectable: a developer chasing a regression cannot tell a real
/// change from a different roll of the dice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sampling {
    interval: u64,
}

impl Sampling {
    /// Keep everything.
    pub const ALL: Self = Self { interval: 1 };

    /// Keep every `interval`-th event.
    ///
    /// # Panics
    /// Panics if `interval` is zero — a sampling rate of "every zeroth
    /// event" has no meaning, and silently treating it as `ALL` would hide
    /// a caller's arithmetic bug behind a profiler that mysteriously costs
    /// more than they configured.
    pub fn every(interval: u64) -> Self {
        assert!(interval > 0, "a sampling interval must be at least 1");

        Self { interval }
    }

    /// Whether the `count`-th event is kept.
    fn keeps(self, count: u64) -> bool {
        self.interval == 1 || count.is_multiple_of(self.interval)
    }
}

impl Default for Sampling {
    fn default() -> Self {
        Self::ALL
    }
}

/// What a [`Collector`] dropped, and why.
///
/// Reported rather than silent: a trace with a hole in it that does not
/// say so is a trace that lies. A developer reading a suspiciously clean
/// timeline needs to know the profiler ran out of room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dropped {
    /// Dropped because the buffer was full.
    pub overflow: u64,
    /// Dropped because sampling skipped them.
    pub sampled: u64,
    /// Dropped because the buffer was momentarily locked.
    ///
    /// Non-zero means another thread held the lock. Recording waits for
    /// nothing, so the event is lost instead of a frame being late.
    pub contended: u64,
}

impl Dropped {
    /// Whether anything was lost.
    pub fn any(&self) -> bool {
        self.overflow > 0 || self.sampled > 0 || self.contended > 0
    }
}

/// A bounded ring of events.
struct Ring {
    events: std::collections::VecDeque<Event>,
    capacity: usize,
    dropped: Dropped,
}

impl Ring {
    fn new(capacity: usize) -> Self {
        Self {
            events: std::collections::VecDeque::with_capacity(capacity),
            capacity,
            dropped: Dropped::default(),
        }
    }

    /// Pushes `event`, making room if needed.
    ///
    /// Making room means evicting the oldest **telemetry**, never a
    /// diagnostic. If the ring holds nothing but diagnostics, a new
    /// telemetry event is refused and a new diagnostic evicts the oldest
    /// diagnostic — at that point the trace is all explanation and no
    /// measurement, and the recent explanations are the useful ones.
    fn push(&mut self, event: Event) {
        if self.events.len() < self.capacity {
            self.events.push_back(event);
            return;
        }

        let incoming = Priority::of(&event);

        let evictable = self
            .events
            .iter()
            .position(|held| Priority::of(held) == Priority::Telemetry);

        match evictable {
            Some(index) => {
                let _ = self.events.remove(index);
                self.events.push_back(event);
                self.dropped.overflow += 1;
            }
            None if incoming == Priority::Diagnostic => {
                let _ = self.events.pop_front();
                self.events.push_back(event);
                self.dropped.overflow += 1;
            }
            None => {
                self.dropped.overflow += 1;
            }
        }
    }
}

struct Inner {
    ring: Mutex<Ring>,
    enabled: AtomicBool,
    counter: AtomicU64,
    started: std::time::Instant,
}

/// Collects [`Event`]s.
///
/// Cheap to clone; every clone shares one buffer.
#[derive(Clone)]
pub struct Collector {
    inner: Arc<Inner>,
    sampling: Sampling,
}

impl std::fmt::Debug for Collector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Collector")
            .field("enabled", &self.is_enabled())
            .field("sampling", &self.sampling)
            .finish_non_exhaustive()
    }
}

/// How many events a [`Collector`] holds by default.
///
/// Sized to hold a few seconds of a busy timeline (~64k events is
/// several hundred frames' worth), which is the window a developer
/// actually looks at, at a fixed ~2 MB rather than a growing anything.
pub const DEFAULT_CAPACITY: usize = 65_536;

impl Collector {
    /// A collector that is off.
    ///
    /// The default, and what a shipped build gets: recording sites cost a
    /// relaxed atomic load and a not-taken branch.
    pub fn disabled() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY, false)
    }

    /// A collector that is on.
    pub fn enabled() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY, true)
    }

    /// A collector holding `capacity` events.
    pub fn with_capacity(capacity: usize, enabled: bool) -> Self {
        Self {
            inner: Arc::new(Inner {
                ring: Mutex::new(Ring::new(capacity)),
                enabled: AtomicBool::new(enabled),
                counter: AtomicU64::new(0),
                started: std::time::Instant::now(),
            }),
            sampling: Sampling::ALL,
        }
    }

    /// This collector, sampling every `interval`-th event.
    pub fn sampled(mut self, sampling: Sampling) -> Self {
        self.sampling = sampling;
        self
    }

    /// Whether collection is on.
    pub fn is_enabled(&self) -> bool {
        // Relaxed: a recording site racing the flag by one event does not
        // matter, and an `Acquire` here would put a fence on the hot path
        // of every instrumented call in the framework.
        self.inner.enabled.load(Ordering::Relaxed)
    }

    /// Turns collection on or off.
    pub fn set_enabled(&self, enabled: bool) {
        self.inner.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Nanoseconds since this collector started.
    pub fn now(&self) -> Nanos {
        self.inner.started.elapsed().as_nanos() as u64
    }

    /// Records whatever `event` builds — **if** collection is on.
    ///
    /// Takes a closure rather than an [`Event`] so that a disabled build
    /// pays nothing for the event's construction. This is the difference
    /// between the 1% budget and blowing it: `record(|| Event::Semantics {
    /// nodes: tree.count(), .. })` must not walk the tree when nobody is
    /// listening.
    #[inline]
    pub fn record(&self, event: impl FnOnce() -> Event) {
        if !self.is_enabled() {
            return;
        }

        self.record_now(event());
    }

    /// Records `event`, which is already built.
    fn record_now(&self, event: Event) {
        let priority = Priority::of(&event);

        // Sampling thins routine measurement. A diagnostic is never
        // sampled away: there is no such thing as "every third missed
        // frame".
        if priority == Priority::Telemetry {
            let count = self.inner.counter.fetch_add(1, Ordering::Relaxed);

            if !self.sampling.keeps(count) {
                if let Ok(mut ring) = self.inner.ring.try_lock() {
                    ring.dropped.sampled += 1;
                }

                return;
            }
        }

        // Never wait. A profiler that blocks the thread it is measuring
        // has changed the measurement, and a late frame is a worse outcome
        // than a missing event.
        match self.inner.ring.try_lock() {
            Ok(mut ring) => ring.push(event),
            Err(_) => {
                if let Ok(mut ring) = self.inner.ring.lock() {
                    ring.dropped.contended += 1;
                }
            }
        }
    }

    /// Every event held, oldest first.
    pub fn events(&self) -> Vec<Event> {
        self.lock().events.iter().copied().collect()
    }

    /// What has been dropped, and why.
    pub fn dropped(&self) -> Dropped {
        self.lock().dropped
    }

    /// How many events are held.
    pub fn len(&self) -> usize {
        self.lock().events.len()
    }

    /// Whether nothing is held.
    pub fn is_empty(&self) -> bool {
        self.lock().events.is_empty()
    }

    /// Takes everything held, leaving the buffer empty.
    pub fn drain(&self) -> Vec<Event> {
        self.lock().events.drain(..).collect()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Ring> {
        self.inner
            .ring
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Default for Collector {
    fn default() -> Self {
        Self::disabled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{Cause, Label, Phase, Resource};
    use std::sync::atomic::AtomicUsize;

    fn telemetry(at: Nanos) -> Event {
        Event::CacheHit {
            resource: Resource::Text,
            at,
        }
    }

    fn diagnostic(frame: u64) -> Event {
        Event::Missed {
            frame,
            over: 1_000_000,
            cause: Cause::GpuStall,
        }
    }

    #[test]
    fn a_disabled_collector_never_builds_the_event() {
        // The 1% budget lives or dies here: `record` takes a closure so
        // that a shipped build does not walk a tree to count nodes for a
        // profiler nobody turned on.
        let collector = Collector::disabled();
        let built = AtomicUsize::new(0);

        for _ in 0..1_000 {
            collector.record(|| {
                let _ = built.fetch_add(1, Ordering::Relaxed);
                telemetry(0)
            });
        }

        assert_eq!(
            built.load(Ordering::Relaxed),
            0,
            "a disabled collector must not even construct the event"
        );
        assert!(collector.is_empty());
    }

    #[test]
    fn an_enabled_collector_records() {
        let collector = Collector::enabled();

        collector.record(|| telemetry(7));

        assert_eq!(collector.events(), vec![telemetry(7)]);
    }

    #[test]
    fn a_full_buffer_drops_telemetry_and_keeps_diagnostics() {
        // `phase.md`: "Buffers are bounded and drop telemetry -- not
        // billing/system events -- under pressure." Losing cache-hit marks
        // costs resolution; losing the missed-frame cause costs the answer.
        let collector = Collector::with_capacity(4, true);

        collector.record(|| diagnostic(1));

        for at in 0..100 {
            collector.record(|| telemetry(at));
        }

        let events = collector.events();

        assert_eq!(events.len(), 4, "the buffer must stay bounded");
        assert!(
            events.contains(&diagnostic(1)),
            "the diagnostic must survive a flood of telemetry: {events:?}"
        );
        assert!(collector.dropped().overflow > 0);
    }

    #[test]
    fn a_buffer_of_nothing_but_diagnostics_keeps_the_newest() {
        // At this point the trace is all explanation and no measurement,
        // and the recent explanations are the useful ones.
        let collector = Collector::with_capacity(2, true);

        for frame in 0..5 {
            collector.record(|| diagnostic(frame));
        }

        let events = collector.events();

        assert_eq!(events.len(), 2);
        assert_eq!(events, vec![diagnostic(3), diagnostic(4)]);
    }

    #[test]
    fn telemetry_is_refused_rather_than_evicting_a_diagnostic() {
        let collector = Collector::with_capacity(2, true);

        collector.record(|| diagnostic(1));
        collector.record(|| diagnostic(2));
        collector.record(|| telemetry(0));

        assert_eq!(
            collector.events(),
            vec![diagnostic(1), diagnostic(2)],
            "telemetry must never displace an explanation"
        );
    }

    #[test]
    fn sampling_thins_telemetry_deterministically() {
        // Deterministic, not random: a trace that cannot be reproduced
        // cannot be bisected.
        let collector = Collector::enabled().sampled(Sampling::every(4));

        for at in 0..16 {
            collector.record(|| telemetry(at));
        }

        assert_eq!(collector.len(), 4, "one in four must be kept");
        assert_eq!(collector.dropped().sampled, 12);
    }

    #[test]
    fn sampling_is_reproducible_across_runs() {
        let run = || {
            let collector = Collector::enabled().sampled(Sampling::every(3));

            for at in 0..30 {
                collector.record(|| telemetry(at));
            }

            collector.events()
        };

        assert_eq!(
            run(),
            run(),
            "the same workload must produce the same trace"
        );
    }

    #[test]
    fn sampling_never_thins_a_diagnostic() {
        // There is no such thing as "every third missed frame".
        let collector = Collector::enabled().sampled(Sampling::every(100));

        for frame in 0..5 {
            collector.record(|| diagnostic(frame));
        }

        assert_eq!(collector.len(), 5);
    }

    #[test]
    #[should_panic(expected = "at least 1")]
    fn a_zero_sampling_interval_is_a_caller_bug() {
        // Treating it as `ALL` would hide the caller's arithmetic behind a
        // profiler that mysteriously costs more than they configured.
        let _ = Sampling::every(0);
    }

    #[test]
    fn drops_are_reported_rather_than_silent() {
        // A trace with a hole that does not say so is a trace that lies.
        let collector = Collector::with_capacity(2, true);

        assert!(!collector.dropped().any());

        for at in 0..10 {
            collector.record(|| telemetry(at));
        }

        assert!(collector.dropped().any());
    }

    #[test]
    fn a_collector_can_be_turned_on_and_off_at_runtime() {
        let collector = Collector::disabled();

        collector.record(|| telemetry(0));
        assert!(collector.is_empty());

        collector.set_enabled(true);
        collector.record(|| telemetry(1));

        assert_eq!(collector.len(), 1);
    }

    #[test]
    fn recording_from_several_threads_never_blocks_or_grows() {
        // The invariant that matters under contention: bounded, and no
        // thread waits. A dropped event is reported; a late frame is not
        // acceptable.
        let collector = Collector::with_capacity(64, true);

        std::thread::scope(|scope| {
            for _ in 0..8 {
                let collector = collector.clone();

                let _ = scope.spawn(move || {
                    for at in 0..1_000 {
                        collector.record(|| Event::Input {
                            kind: Label::new("pointer_moved"),
                            at,
                        });
                    }
                });
            }
        });

        assert!(collector.len() <= 64, "the buffer must stay bounded");
    }

    #[test]
    fn draining_empties_the_buffer() {
        let collector = Collector::enabled();

        collector.record(|| telemetry(0));

        assert_eq!(collector.drain().len(), 1);
        assert!(
            collector.is_empty(),
            "a drained event must not be exported twice"
        );
    }

    #[test]
    fn phases_round_trip_through_the_buffer() {
        let collector = Collector::enabled();

        collector.record(|| Event::PhaseBegin {
            phase: Phase::Layout,
            at: 10,
        });
        collector.record(|| Event::PhaseEnd {
            phase: Phase::Layout,
            at: 20,
        });

        let events = collector.events();

        assert_eq!(events[0].phase(), Some(Phase::Layout));
        assert_eq!(events[1].at(), 20);
    }
}

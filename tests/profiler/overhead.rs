//! Phase 15's cost rules: *"disabled profiler overhead must be
//! statistically indistinguishable or below 1%"*, *"enabled default
//! sampling overhead must remain below 3%"*, and *"buffers are bounded
//! and drop telemetry — not billing/system events — under pressure"*.
//!
//! # How to prove a cost rule, and how not to
//!
//! A test that times two loops and asserts `enabled < disabled * 1.01` is
//! the obvious approach and a bad one. On a shared machine, a scheduler
//! hiccup swamps a 1% effect, so the test either fails at random or
//! passes because the threshold was loosened until it stopped failing —
//! at which point it proves nothing but is still red-or-green, which is
//! worse than absent.
//!
//! So the guarantee here is **structural**, and the timing is a smoke
//! test:
//!
//! - The structural test is that a disabled collector never runs the
//!   closure. That is what makes disabled overhead a relaxed atomic load
//!   and a not-taken branch, and it is exactly checkable, every run, on
//!   any machine.
//! - The timing test takes the median of many trials and uses a bound
//!   loose enough never to fail on noise. It catches a *category* error —
//!   someone making `record` take a lock, or allocate, when disabled — not
//!   a 1% drift. The 1% number is a budget for `aura_bench` on a quiet
//!   reference machine; it is not something a unit test can honestly
//!   assert, and this file does not pretend otherwise.

use aura_profiler::collector::{Collector, Sampling};
use aura_profiler::events::{Cause, Event, Label, Phase, Resource};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

fn telemetry(at: u64) -> Event {
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

/// The median of `trials` runs of `work`, in nanoseconds.
///
/// Median, not mean: one descheduled trial ruins a mean and leaves a
/// median untouched, and a scheduler hiccup is the common case on any
/// machine that is also doing something else.
fn median_nanos(trials: usize, mut work: impl FnMut()) -> u128 {
    let mut samples: Vec<u128> = (0..trials)
        .map(|_| {
            let began = Instant::now();
            work();
            began.elapsed().as_nanos()
        })
        .collect();

    samples.sort_unstable();
    samples[samples.len() / 2]
}

#[test]
fn a_disabled_collector_never_builds_the_event() {
    // The structural guarantee the whole 1% budget rests on: `record`
    // takes a closure, so a shipped build does not read a clock or walk a
    // tree for a profiler nobody turned on.
    let collector = Collector::disabled();
    let built = AtomicUsize::new(0);

    for frame in 0..100_000 {
        collector.record(|| {
            let _ = built.fetch_add(1, Ordering::Relaxed);
            Event::FrameBegin { frame, at: 0 }
        });
    }

    assert_eq!(
        built.load(Ordering::Relaxed),
        0,
        "a disabled collector must not construct a single event"
    );
}

#[test]
fn a_disabled_collector_is_cheap_enough_to_leave_in_a_shipped_build() {
    // A smoke test, not a 1% proof -- see this file's module docs. It
    // catches someone making `record` take a lock or allocate when
    // disabled, which shows up as orders of magnitude, not percent.
    let collector = Collector::disabled();

    let baseline = median_nanos(9, || {
        for at in 0..10_000u64 {
            let _ = std::hint::black_box(at);
        }
    });

    let instrumented = median_nanos(9, || {
        for at in 0..10_000u64 {
            collector.record(|| telemetry(std::hint::black_box(at)));
        }
    });

    // Deliberately loose. The point is the shape of the cost, not its
    // size: an atomic load per site is nanoseconds, a lock or an
    // allocation is hundreds.
    assert!(
        instrumented < baseline.max(1) * 20,
        "a disabled recording site cost {instrumented}ns against a {baseline}ns baseline; \
         that is the shape of a lock or an allocation, not an atomic load"
    );
}

#[test]
fn an_enabled_collector_stays_within_a_frames_budget() {
    // The 3% rule's shape: recording must be cheap enough that a frame's
    // worth of events costs a rounding error of the frame. A 60Hz frame
    // is 16.6ms; a few hundred events is a generous frame's worth.
    let collector = Collector::enabled();

    let elapsed = median_nanos(9, || {
        for at in 0..500u64 {
            collector.record(|| telemetry(at));
        }
    });

    assert!(
        elapsed < 5_000_000,
        "500 events cost {elapsed}ns -- more than 5ms of a 16.6ms frame is not instrumentation, \
         it is the workload"
    );
}

#[test]
fn a_bounded_buffer_cannot_grow_no_matter_how_long_it_runs() {
    // The rule this protects: a profiler that grows without bound under
    // load kills the application it was meant to diagnose, exactly when
    // things are already going wrong.
    let collector = Collector::with_capacity(256, true);

    for at in 0..1_000_000u64 {
        collector.record(|| telemetry(at));
    }

    assert_eq!(
        collector.len(),
        256,
        "a million events must leave the buffer at its capacity, not above it"
    );
    assert!(collector.dropped().overflow > 0, "and must say so");
}

#[test]
fn overflow_drops_telemetry_and_keeps_the_explanation() {
    // *"drop telemetry -- not billing/system events -- under pressure"*.
    // Losing cache-hit marks costs a developer resolution; losing the
    // missed-frame cause costs them the answer.
    let collector = Collector::with_capacity(8, true);

    collector.record(|| diagnostic(1));
    collector.record(|| diagnostic(2));

    for at in 0..10_000u64 {
        collector.record(|| telemetry(at));
    }

    let events = collector.events();

    assert_eq!(events.len(), 8);

    for frame in [1, 2] {
        assert!(
            events.contains(&diagnostic(frame)),
            "the diagnostics must survive a flood of telemetry: {events:?}"
        );
    }
}

#[test]
fn overflow_is_reported_rather_than_silent() {
    // A trace with a hole in it that does not say so is a trace that lies:
    // a developer reading a suspiciously clean timeline needs to know the
    // profiler ran out of room.
    let collector = Collector::with_capacity(4, true);

    assert!(!collector.dropped().any(), "nothing dropped yet");

    for at in 0..100u64 {
        collector.record(|| telemetry(at));
    }

    assert_eq!(collector.dropped().overflow, 96);
}

#[test]
fn sampling_thins_telemetry_without_thinning_the_explanation() {
    let collector = Collector::enabled().sampled(Sampling::every(10));

    for at in 0..1_000u64 {
        collector.record(|| telemetry(at));
    }

    for frame in 0..5 {
        collector.record(|| diagnostic(frame));
    }

    let events = collector.events();
    let diagnostics = events
        .iter()
        .filter(|event| matches!(event, Event::Missed { .. }))
        .count();

    assert_eq!(
        diagnostics, 5,
        "there is no such thing as every tenth missed frame"
    );
    assert_eq!(
        events.len() - diagnostics,
        100,
        "one telemetry event in ten must survive"
    );
}

#[test]
fn a_sampled_trace_is_reproducible() {
    // Deterministic sampling, not random: a developer bisecting a
    // regression cannot tell a real change from a different roll of the
    // dice.
    let run = || {
        let collector = Collector::enabled().sampled(Sampling::every(7));

        for at in 0..700u64 {
            collector.record(|| telemetry(at));
        }

        collector.events()
    };

    assert_eq!(run(), run());
}

#[test]
fn recording_from_many_threads_stays_bounded_and_never_deadlocks() {
    // A profiler must never be the reason a frame is late. `record` uses
    // `try_lock` and drops rather than wait, so contention costs events,
    // not time.
    let collector = Collector::with_capacity(512, true);

    std::thread::scope(|scope| {
        for _ in 0..16 {
            let collector = collector.clone();

            let _ = scope.spawn(move || {
                for at in 0..20_000u64 {
                    collector.record(|| Event::Input {
                        kind: Label::new("pointer_moved"),
                        at,
                    });
                }
            });
        }
    });

    assert!(collector.len() <= 512);
}

#[test]
fn every_frames_time_can_be_traced_to_a_phase() {
    // Phase 15's exit gate: *"every global metric can be traced to a
    // phase/cause"*. A frame time that cannot be attributed is a number a
    // developer can do nothing with.
    let collector = Collector::enabled();

    collector.record(|| Event::FrameBegin { frame: 1, at: 0 });

    for (phase, began, ended) in [
        (Phase::Input, 0, 1_000_000),
        (Phase::Layout, 1_000_000, 4_000_000),
        (Phase::Paint, 4_000_000, 9_000_000),
    ] {
        collector.record(|| Event::PhaseBegin { phase, at: began });
        collector.record(|| Event::PhaseEnd { phase, at: ended });
    }

    collector.record(|| Event::FrameEnd {
        frame: 1,
        at: 10_000_000,
    });

    let events = collector.events();
    let totals = aura_profiler::export::phase_totals(&events);
    let attributed: u64 = totals.iter().map(|(_, total)| total).sum();

    assert_eq!(
        totals,
        vec![
            (Phase::Input, 1_000_000),
            (Phase::Layout, 3_000_000),
            (Phase::Paint, 5_000_000),
        ]
    );

    // The residue is the honest part: 1ms of this frame is unaccounted
    // for, and the report says so rather than quietly assigning it.
    assert_eq!(
        10_000_000 - attributed,
        1_000_000,
        "unattributed time must be visible as a residue, not hidden"
    );
}

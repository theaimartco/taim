//! Deterministic, fake-clock scenario tests for
//! `taim_runtime::frame_scheduler::Scheduler`.
//!
//! `runtime/src/frame_scheduler.rs`'s own unit tests already cover every
//! individual transition in isolation. These tests instead replay
//! realistic, multi-step sequences an application's event loop would
//! actually produce -- a redraw request followed by submission and
//! feedback, an animation running across several ticks, an occlusion
//! interrupting an in-progress animation -- and confirm the same
//! sequence of operations against the same fake clock produces the
//! exact same [`Scheduler`] decisions every time, which is the whole
//! point of taking every timestamp as an explicit parameter rather than
//! reading a live clock.

use std::time::{Duration, Instant};
use taim_runtime::frame_scheduler::Scheduler;

fn epoch() -> Instant {
    // A single, fixed instant shared by every test in this file (not a
    // fresh `Instant::now()` per call), so `t(millis)` offsets are
    // exactly reproducible within one test run.
    static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

fn t(millis: u64) -> Instant {
    epoch() + Duration::from_millis(millis)
}

/// Replays a fixed sequence of operations against a fresh [`Scheduler`],
/// returning the sequence of `should_produce_frame` results observed at
/// each of the given check times -- the shape a real event loop would
/// actually query.
fn replay(operations: impl Fn(&mut Scheduler), check_times: &[u64]) -> Vec<bool> {
    let mut scheduler = Scheduler::new();
    operations(&mut scheduler);

    check_times
        .iter()
        .map(|millis| scheduler.should_produce_frame(t(*millis)))
        .collect()
}

#[test]
fn the_same_sequence_of_operations_produces_the_same_decisions_every_time() {
    let operations = |scheduler: &mut Scheduler| {
        scheduler.request_redraw_at(t(10));
    };

    let first_run = replay(operations, &[0, 5, 10, 15]);
    let second_run = replay(operations, &[0, 5, 10, 15]);

    assert_eq!(first_run, second_run);
    assert_eq!(first_run, vec![false, false, true, true]);
}

#[test]
fn a_typical_redraw_cycle_requests_submits_and_recovers_via_feedback() {
    let mut scheduler = Scheduler::new();

    // A widget requests a redraw for 10ms from now.
    scheduler.request_redraw_at(t(10));
    assert!(!scheduler.should_produce_frame(t(5)));

    // The deadline arrives; the event loop produces and submits a frame.
    assert!(scheduler.should_produce_frame(t(10)));
    assert!(scheduler.begin_frame(t(10)));
    scheduler.submit(t(10));

    // While waiting on presentation feedback, no further frame should
    // be produced, even though nothing else has requested one.
    assert!(!scheduler.should_produce_frame(t(11)));

    // Feedback arrives quickly; the scheduler is free again.
    scheduler.feedback_received();
    assert!(!scheduler.should_produce_frame(t(12)));
}

#[test]
fn an_animation_runs_for_several_ticks_then_stops_cleanly() {
    let mut scheduler = Scheduler::new();
    scheduler.request_animation();

    // A continuous request stays live across presentation feedback until the
    // UI explicitly reports that animation has stopped.
    for millis in [0, 16, 32, 48] {
        assert!(scheduler.should_produce_frame(t(millis)));
        assert!(scheduler.begin_frame(t(millis)));
        scheduler.submit(t(millis));
        scheduler.feedback_received();
    }

    scheduler.stop_animating();

    assert!(!scheduler.should_produce_frame(t(64)));
}

#[test]
fn occlusion_interrupts_an_in_progress_animation_and_resuming_continues_it() {
    let mut scheduler = Scheduler::new();
    scheduler.request_animation();

    // A few ticks of animation happen normally.
    assert!(scheduler.should_produce_frame(t(0)));
    assert!(scheduler.begin_frame(t(0)));
    scheduler.submit(t(0));
    scheduler.feedback_received();

    // The window becomes occluded mid-animation.
    scheduler.suspend();
    assert!(!scheduler.should_produce_frame(t(16)));
    assert!(!scheduler.should_produce_frame(t(1000)));

    // It becomes visible again; the animation resumes without needing
    // to be re-requested from scratch.
    scheduler.resume();
    assert!(scheduler.should_produce_frame(t(1016)));
}

#[test]
fn a_dropped_feedback_callback_does_not_permanently_stall_a_later_animation_request() {
    let mut scheduler = Scheduler::new();
    scheduler.submit(t(0));

    // Feedback never arrives; the event loop keeps ticking.
    scheduler.tick(t(50));
    scheduler.tick(t(150));
    scheduler.tick(t(250)); // past FEEDBACK_TIMEOUT (200ms)

    // The scheduler must have recovered on its own by now, so a new
    // animation request actually takes effect.
    scheduler.request_animation();
    assert!(scheduler.should_produce_frame(t(251)));
}

#[test]
fn several_coalescing_requests_in_sequence_always_keep_the_earliest_deadline() {
    let mut scheduler = Scheduler::new();

    for millis in [100, 50, 200, 30, 80] {
        scheduler.request_redraw_at(t(millis));
    }

    assert_eq!(scheduler.deadline(), Some(t(30)));
}

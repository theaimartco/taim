//! Suspend/resume/deadline and missed-feedback [`Scheduler`] cases
//! through its public API, from outside the crate.
//!
//! `runtime/src/frame_scheduler.rs`'s own unit tests already cover
//! these transitions directly; these tests exercise the same
//! properties as an external, black-box consumer would (only the
//! public `Scheduler` methods, no access to its private `State`), and
//! add a few compound, multi-window-shaped scenarios its own module
//! tests do not.

use std::time::{Duration, Instant};
use taim_runtime::frame_scheduler::{FEEDBACK_TIMEOUT, Scheduler};

fn epoch() -> Instant {
    static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

fn t(millis: u64) -> Instant {
    epoch() + Duration::from_millis(millis)
}

#[test]
fn suspending_a_scheduler_with_no_pending_request_still_blocks_future_ones() {
    let mut scheduler = Scheduler::new();
    scheduler.suspend();

    scheduler.request_redraw_at(t(0));

    assert!(!scheduler.should_produce_frame(t(1000)));
}

#[test]
fn suspend_then_resume_with_nothing_pending_leaves_the_scheduler_idle() {
    let mut scheduler = Scheduler::new();
    scheduler.suspend();
    scheduler.resume();

    assert!(!scheduler.should_produce_frame(t(0)));
    assert_eq!(scheduler.deadline(), None);
}

#[test]
fn deadline_reflects_the_feedback_timeout_while_a_frame_is_submitted() {
    let mut scheduler = Scheduler::new();
    scheduler.submit(t(100));

    assert_eq!(scheduler.deadline(), Some(t(100) + FEEDBACK_TIMEOUT));
}

#[test]
fn a_suspended_scheduler_reports_no_deadline_even_with_a_pending_request() {
    let mut scheduler = Scheduler::new();
    scheduler.request_redraw_at(t(10));
    scheduler.suspend();

    assert_eq!(
        scheduler.deadline(),
        None,
        "an event loop waiting on this deadline must not wake up early just to \
         discover the surface is still occluded"
    );
}

#[test]
fn multiple_independent_schedulers_do_not_affect_each_other() {
    // Standing in for two windows in the same application: suspending
    // one must have no bearing on the other.
    let mut window_a = Scheduler::new();
    let mut window_b = Scheduler::new();

    window_a.request_animation();
    window_b.request_redraw_at(t(50));

    window_a.suspend();

    assert!(!window_a.should_produce_frame(t(0)));
    assert!(!window_b.should_produce_frame(t(10)));
    assert!(window_b.should_produce_frame(t(50)));
}

#[test]
fn repeated_suspend_resume_cycles_do_not_lose_or_duplicate_a_pending_deadline() {
    let mut scheduler = Scheduler::new();
    scheduler.request_redraw_at(t(100));

    for _ in 0..5 {
        scheduler.suspend();
        scheduler.resume();
    }

    assert_eq!(scheduler.deadline(), Some(t(100)));
}

#[test]
fn a_missed_feedback_deadline_and_a_pending_redraw_request_do_not_interfere() {
    // A frame is submitted while a separate redraw is already scheduled
    // for a later time (e.g. an animation deadline queued up behind an
    // in-flight submission).
    let mut scheduler = Scheduler::new();
    scheduler.submit(t(0));

    // Requesting a redraw while submitted is a no-op per `Scheduler`'s
    // own contract (an in-flight submission already implies "a frame is
    // coming"); once the timeout recovers the scheduler, a fresh
    // request is needed.
    scheduler.request_redraw_at(t(50));
    scheduler.tick(t(50));
    assert!(!scheduler.should_produce_frame(t(50)));

    scheduler.tick(t(250));
    scheduler.request_redraw_at(t(300));

    assert!(!scheduler.should_produce_frame(t(299)));
    assert!(scheduler.should_produce_frame(t(300)));
}

#[test]
fn suspending_while_a_frame_is_submitted_still_blocks_new_submissions() {
    let mut scheduler = Scheduler::new();
    scheduler.submit(t(0));
    scheduler.suspend();

    // `submit` itself is documented as a no-op while suspended, guarding
    // against a caller racing past a suspend event.
    scheduler.submit(t(5));

    assert!(!scheduler.should_produce_frame(t(1000)));
}

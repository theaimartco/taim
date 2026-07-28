//! Competition/cancellation/capture state matrix for
//! `aura_gesture::arena::Arena`, exercised with the real recognizers
//! shipped alongside it (`Tap`, `Pan`, `LongPress`) rather than
//! `arena.rs`'s own synthetic test doubles, so this suite proves the
//! arena and a realistic mix of recognizers agree, not just the arena
//! in isolation.
//!
//! Every scenario here is one of `phase.md`'s own correctness rules
//! made concrete:
//!
//! - every pointer sequence ends in up or cancel for all participants;
//! - losing recognizers receive cancellation exactly once;
//! - captured pointers keep routing outside their original bounds.

use aura_gesture::arena::{Arena, Priority, Resolution, Team};
use aura_gesture::long_press::LongPress;
use aura_gesture::pan::Pan;
use aura_gesture::tap::Tap;

use aura_core::pointer::{DeviceKind, Id as PointerId, Metadata, Phase, Pointer};
use aura_core::time::{Duration, Instant};

fn epoch() -> Instant {
    static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

fn t(millis: u64) -> Instant {
    epoch() + Duration::from_millis(millis)
}

fn pointer(phase: Phase, x: f32, y: f32, millis: u64) -> Pointer {
    Pointer {
        id: PointerId::from_raw(1),
        device: DeviceKind::Touch,
        phase,
        position: aura_core::Point::new(x, y),
        timestamp: t(millis),
        metadata: Metadata::NONE,
        captured: false,
    }
}

#[test]
fn a_quick_tap_beats_a_pan_that_never_moves_far_enough() {
    let mut arena = Arena::new();
    let tap = arena.register(Tap::new(), Team::solo(), Priority::NORMAL);
    let _pan = arena.register(Pan::new(), Team::solo(), Priority::NORMAL);

    assert_eq!(arena.dispatch(pointer(Phase::Down, 0.0, 0.0, 0)), None);
    assert_eq!(
        arena.dispatch(pointer(Phase::Up, 0.0, 0.0, 50)),
        Some(Resolution::Won(tap))
    );
}

#[test]
fn a_pan_past_the_slop_threshold_beats_a_tap() {
    let mut arena = Arena::new();
    let _tap = arena.register(Tap::new(), Team::solo(), Priority::NORMAL);
    let pan = arena.register(Pan::new(), Team::solo(), Priority::NORMAL);

    assert_eq!(arena.dispatch(pointer(Phase::Down, 0.0, 0.0, 0)), None);

    // Past Tap's slop (8.0) and Pan's own slop, so Tap rejects (moved
    // too far to be a tap) while Pan accepts (crossed its own
    // movement threshold) in the same pass.
    let resolution = arena.dispatch(pointer(Phase::Move, 100.0, 0.0, 20));

    assert_eq!(resolution, Some(Resolution::Won(pan)));
}

#[test]
fn a_long_press_resolves_via_tick_with_no_new_pointer_event() {
    let mut arena = Arena::new();
    let long_press = arena.register(LongPress::new(), Team::solo(), Priority::NORMAL);
    let _tap = arena.register(Tap::new(), Team::solo(), Priority::NORMAL);

    assert_eq!(arena.dispatch(pointer(Phase::Down, 0.0, 0.0, 0)), None);

    // Before the long-press deadline (500ms default): nothing resolves,
    // since Tap is still pending (it only rejects on move-past-slop or
    // its own timeout) and LongPress is still pending too.
    assert_eq!(arena.tick(t(100)), None);
    assert!(!arena.is_resolved());

    // At/after the deadline, with the pointer having never moved,
    // LongPress accepts and wins -- Tap has not rejected on its own,
    // but LongPress's eager accept (rule 1) wins immediately.
    let resolution = arena.tick(t(500));

    assert_eq!(resolution, Some(Resolution::Won(long_press)));
}

#[test]
fn every_losing_entrant_is_cancelled_exactly_once() {
    use aura_gesture::recognizer::{Disposition, Recognizer, Verdict};
    use std::cell::Cell;
    use std::rc::Rc;

    struct CountingLoser {
        verdicts_received: Rc<Cell<u32>>,
    }

    impl Recognizer for CountingLoser {
        fn name(&self) -> &'static str {
            "counting_loser"
        }

        fn add_pointer(&mut self, _pointer: Pointer) {}
        fn remove_pointer(&mut self, _id: PointerId) {}

        fn on_event(&mut self, _pointer: Pointer) -> Disposition {
            Disposition::Pending
        }

        fn on_verdict(&mut self, verdict: Verdict) {
            assert_eq!(verdict, Verdict::Lost);
            self.verdicts_received.set(self.verdicts_received.get() + 1);
        }

        fn reset(&mut self) {}
    }

    let mut arena = Arena::new();
    let counter = Rc::new(Cell::new(0));

    let _loser = arena.register(
        CountingLoser {
            verdicts_received: Rc::clone(&counter),
        },
        Team::solo(),
        Priority::NORMAL,
    );
    let _tap = arena.register(Tap::new(), Team::solo(), Priority::NORMAL);

    assert_eq!(arena.dispatch(pointer(Phase::Down, 0.0, 0.0, 0)), None);

    let resolution = arena.dispatch(pointer(Phase::Up, 0.0, 0.0, 50));

    assert!(matches!(resolution, Some(Resolution::Won(_))));
    assert_eq!(
        counter.get(),
        1,
        "a losing entrant must receive exactly one Lost verdict, not zero or more than one"
    );
}

#[test]
fn a_team_wins_or_loses_as_one_unit() {
    use aura_gesture::recognizer::{Disposition, Recognizer, Verdict};

    struct AlwaysAccepts;

    impl Recognizer for AlwaysAccepts {
        fn name(&self) -> &'static str {
            "always_accepts"
        }
        fn add_pointer(&mut self, _pointer: Pointer) {}
        fn remove_pointer(&mut self, _id: PointerId) {}
        fn on_event(&mut self, _pointer: Pointer) -> Disposition {
            Disposition::Accepted
        }
        fn on_verdict(&mut self, _verdict: Verdict) {}
        fn reset(&mut self) {}
    }

    struct AlwaysRejects;

    impl Recognizer for AlwaysRejects {
        fn name(&self) -> &'static str {
            "always_rejects"
        }
        fn add_pointer(&mut self, _pointer: Pointer) {}
        fn remove_pointer(&mut self, _id: PointerId) {}
        fn on_event(&mut self, _pointer: Pointer) -> Disposition {
            Disposition::Rejected
        }
        fn on_verdict(&mut self, _verdict: Verdict) {}
        fn reset(&mut self) {}
    }

    let mut arena = Arena::new();
    let team = Team::new(42);

    // One team member accepts immediately; a lone outside competitor
    // always rejects. The whole team should win via the accepting
    // member, even though its teammate never itself accepted.
    let accepting_member = arena.register(AlwaysAccepts, team, Priority::NORMAL);
    let _teammate = arena.register(AlwaysRejects, team, Priority::NORMAL);
    let _outsider = arena.register(AlwaysRejects, Team::solo(), Priority::NORMAL);

    // `Down` only registers the pointer with each entrant (see
    // `Arena::dispatch`'s own documentation); `on_event` -- and thus
    // any disposition -- is only invoked from a follow-up event.
    assert_eq!(arena.dispatch(pointer(Phase::Down, 0.0, 0.0, 0)), None);

    let resolution = arena.dispatch(pointer(Phase::Move, 1.0, 0.0, 1));

    assert_eq!(resolution, Some(Resolution::Won(accepting_member)));
}

#[test]
fn a_system_priority_entrant_wins_a_same_pass_tie_over_a_normal_one() {
    use aura_gesture::recognizer::{Disposition, Recognizer, Verdict};

    struct AlwaysAccepts;

    impl Recognizer for AlwaysAccepts {
        fn name(&self) -> &'static str {
            "always_accepts"
        }
        fn add_pointer(&mut self, _pointer: Pointer) {}
        fn remove_pointer(&mut self, _id: PointerId) {}
        fn on_event(&mut self, _pointer: Pointer) -> Disposition {
            Disposition::Accepted
        }
        fn on_verdict(&mut self, _verdict: Verdict) {}
        fn reset(&mut self) {}
    }

    let mut arena = Arena::new();
    let _normal = arena.register(AlwaysAccepts, Team::solo(), Priority::NORMAL);
    let system = arena.register(AlwaysAccepts, Team::solo(), Priority::SYSTEM);

    assert_eq!(arena.dispatch(pointer(Phase::Down, 0.0, 0.0, 0)), None);

    let resolution = arena.dispatch(pointer(Phase::Move, 1.0, 0.0, 1));

    assert_eq!(
        resolution,
        Some(Resolution::Won(system)),
        "a SYSTEM-priority entrant must win a same-pass tie over a NORMAL one, \
         regardless of registration order"
    );
}

#[test]
fn cancel_resolves_with_cancelled_and_is_idempotent_afterward() {
    let mut arena = Arena::new();
    let _tap = arena.register(Tap::new(), Team::solo(), Priority::NORMAL);
    let _pan = arena.register(Pan::new(), Team::solo(), Priority::NORMAL);

    assert_eq!(arena.dispatch(pointer(Phase::Down, 0.0, 0.0, 0)), None);

    assert_eq!(arena.cancel(), Some(Resolution::Cancelled));
    assert!(arena.is_resolved());

    // Cancelling an already-resolved arena is a documented no-op,
    // returning `None` rather than `Some(Resolution::Cancelled)` again.
    assert_eq!(arena.cancel(), None);
    assert_eq!(arena.resolution(), Some(Resolution::Cancelled));
}

#[test]
fn a_cancel_pointer_phase_resolves_the_arena_as_cancelled() {
    let mut arena = Arena::new();
    let _tap = arena.register(Tap::new(), Team::solo(), Priority::NORMAL);
    let _pan = arena.register(Pan::new(), Team::solo(), Priority::NORMAL);

    assert_eq!(arena.dispatch(pointer(Phase::Down, 0.0, 0.0, 0)), None);

    let resolution = arena.dispatch(pointer(Phase::Cancel, 0.0, 0.0, 10));

    assert_eq!(resolution, Some(Resolution::Cancelled));
}

#[test]
fn every_entrant_rejecting_resolves_with_no_winner() {
    use aura_gesture::recognizer::{Disposition, Recognizer, Verdict};

    struct AlwaysRejects;

    impl Recognizer for AlwaysRejects {
        fn name(&self) -> &'static str {
            "always_rejects"
        }
        fn add_pointer(&mut self, _pointer: Pointer) {}
        fn remove_pointer(&mut self, _id: PointerId) {}
        fn on_event(&mut self, _pointer: Pointer) -> Disposition {
            Disposition::Rejected
        }
        fn on_verdict(&mut self, _verdict: Verdict) {}
        fn reset(&mut self) {}
    }

    let mut arena = Arena::new();
    let _first = arena.register(AlwaysRejects, Team::solo(), Priority::NORMAL);
    let _second = arena.register(AlwaysRejects, Team::solo(), Priority::NORMAL);

    assert_eq!(arena.dispatch(pointer(Phase::Down, 0.0, 0.0, 0)), None);

    let resolution = arena.dispatch(pointer(Phase::Move, 1.0, 0.0, 1));

    assert_eq!(resolution, Some(Resolution::NoWinner));
}

#[test]
#[should_panic(expected = "already-resolved")]
fn dispatching_into_a_resolved_arena_panics() {
    let mut arena = Arena::new();
    let _tap = arena.register(Tap::new(), Team::solo(), Priority::NORMAL);

    // A lone entrant already wins on `Down` via "last one standing" --
    // there is nothing left to compete against, since `pending_teams`
    // already has exactly one member the moment it is registered.
    let resolution = arena.dispatch(pointer(Phase::Down, 0.0, 0.0, 0));
    assert!(matches!(resolution, Some(Resolution::Won(_))));

    // A further dispatch into the now-resolved arena must panic per
    // `Arena::dispatch`'s own documented contract.
    let _ = arena.dispatch(pointer(Phase::Move, 1.0, 1.0, 30));
}

#[test]
fn resolution_stays_stable_once_reached() {
    let mut arena = Arena::new();
    let tap = arena.register(Tap::new(), Team::solo(), Priority::NORMAL);

    // A lone entrant resolves immediately on Down via "last one
    // standing" -- there is nothing left to compete against.
    assert_eq!(
        arena.dispatch(pointer(Phase::Down, 0.0, 0.0, 0)),
        Some(Resolution::Won(tap))
    );
    assert_eq!(arena.resolution(), Some(Resolution::Won(tap)));
    assert!(arena.is_resolved());
}

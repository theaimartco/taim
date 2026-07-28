//! Handoff, fling, and boundary cases for
//! `aura_gesture::nested_scroll::{Coordinator, dispatch_chain}`, beyond
//! the two/three-level unit tests already covered in
//! `aura-gesture/src/nested_scroll.rs` itself.
//!
//! This suite's focus is the scenarios `phase.md`'s exit gate calls out
//! by name: "nested scroll has no stuck capture." A stuck capture here
//! would mean a child [`Coordinator`] keeps reporting a non-zero
//! remainder forever (starving every ancestor, the exact bug this
//! phase's own `widget::Scrollable` fix addressed for the single-level
//! case), or a fling that never actually drains to zero.

use aura_core::Vector;
use aura_gesture::nested_scroll::{Coordinator, Delta, Extent, dispatch_chain};

fn extent(max_y: f32) -> Extent {
    Extent {
        min: Vector::ZERO,
        max: Vector::new(0.0, max_y),
    }
}

#[test]
fn a_three_level_hierarchy_hands_off_in_order_child_first() {
    let mut grandchild = Coordinator::new(extent(10.0));
    let mut child = Coordinator::new(extent(20.0));
    let mut parent = Coordinator::new(extent(500.0));

    let consumption = dispatch_chain(
        &mut [&mut grandchild, &mut child, &mut parent],
        Delta {
            amount: Vector::new(0.0, 100.0),
        },
        false,
    );

    assert_eq!(grandchild.offset(), Vector::new(0.0, 10.0));
    assert_eq!(child.offset(), Vector::new(0.0, 20.0));
    assert_eq!(parent.offset(), Vector::new(0.0, 70.0));
    assert_eq!(consumption.consumed, Vector::new(0.0, 100.0));
    assert_eq!(consumption.remainder, Vector::ZERO);
}

#[test]
fn a_middle_level_that_cannot_move_at_all_passes_the_whole_delta_through() {
    // A middle level already at its own limit (zero-height extent, or
    // already at max) must not "eat" any of the delta -- it should
    // report the entire amount as remainder, letting the parent scroll
    // exactly as if the middle level were not there at all.
    let mut child = Coordinator::new(extent(50.0));
    let mut middle = Coordinator::new(extent(0.0));
    let mut parent = Coordinator::new(extent(500.0));

    let consumption = dispatch_chain(
        &mut [&mut child, &mut middle, &mut parent],
        Delta {
            amount: Vector::new(0.0, 100.0),
        },
        false,
    );

    assert_eq!(child.offset(), Vector::new(0.0, 50.0));
    assert_eq!(
        middle.offset(),
        Vector::ZERO,
        "a zero-extent level cannot consume anything"
    );
    assert_eq!(parent.offset(), Vector::new(0.0, 50.0));
    assert_eq!(consumption.remainder, Vector::ZERO);
}

#[test]
fn a_simulated_fling_drains_to_zero_and_never_gets_stuck() {
    // A fling is modeled as a sequence of shrinking deltas (simple
    // exponential decay, as a real velocity-driven fling would
    // produce) dispatched one at a time; every dispatch must eventually
    // leave both levels at their maximum with zero remainder forever
    // after, not oscillate or stall with a permanently non-zero
    // leftover.
    let mut child = Coordinator::new(extent(40.0));
    let mut parent = Coordinator::new(extent(200.0));

    let mut velocity = 50.0_f32;
    let mut last_consumption = None;

    for _ in 0..40 {
        if velocity.abs() < 0.01 {
            break;
        }

        let consumption = dispatch_chain(
            &mut [&mut child, &mut parent],
            Delta {
                amount: Vector::new(0.0, velocity),
            },
            false,
        );

        last_consumption = Some(consumption);
        velocity *= 0.8;
    }

    assert_eq!(child.offset(), Vector::new(0.0, 40.0));
    assert_eq!(parent.offset(), Vector::new(0.0, 200.0));

    // Once both levels are saturated, any further dispatch must report
    // the full amount back as remainder -- the "stuck capture" failure
    // mode would instead show a partial, permanently nonzero consumed
    // value that never lets the remainder reach the true total.
    let final_consumption = dispatch_chain(
        &mut [&mut child, &mut parent],
        Delta {
            amount: Vector::new(0.0, 5.0),
        },
        false,
    );

    assert_eq!(final_consumption.consumed, Vector::ZERO);
    assert_eq!(final_consumption.remainder, Vector::new(0.0, 5.0));
    assert!(last_consumption.is_some());
}

#[test]
fn overscroll_at_the_child_does_not_leak_into_the_parent() {
    // While a drag is in progress (`allow_overscroll = true`), a
    // saturated child should absorb the whole delta as rubber-band
    // stretch rather than ever handing a remainder up to the parent --
    // otherwise the parent would visibly "keep scrolling" underneath a
    // child that appears to have stopped, which is the nested-scroll
    // equivalent of a stuck/leaking capture.
    let mut child = Coordinator::new(extent(20.0));
    let mut parent = Coordinator::new(extent(500.0));

    let consumption = dispatch_chain(
        &mut [&mut child, &mut parent],
        Delta {
            amount: Vector::new(0.0, 100.0),
        },
        true,
    );

    assert_eq!(child.offset(), Vector::new(0.0, 20.0));
    assert_eq!(child.overscroll(), Vector::new(0.0, 80.0));
    assert_eq!(parent.offset(), Vector::ZERO);
    assert_eq!(consumption.remainder, Vector::ZERO);
}

#[test]
fn releasing_overscroll_mid_chain_settles_cleanly_before_a_fling() {
    let mut child = Coordinator::new(extent(20.0));
    let mut parent = Coordinator::new(extent(500.0));

    let _ = dispatch_chain(
        &mut [&mut child, &mut parent],
        Delta {
            amount: Vector::new(0.0, 100.0),
        },
        true,
    );

    assert_eq!(child.overscroll(), Vector::new(0.0, 80.0));

    child.settle_overscroll();

    assert_eq!(child.offset(), Vector::new(0.0, 20.0));
    assert_eq!(child.overscroll(), Vector::ZERO);

    // After settling, a subsequent fling-style dispatch behaves exactly
    // as it would have with no prior overscroll history -- no leftover
    // state biases it: the now-saturated child hands the whole delta
    // to the parent, which still has room to consume it.
    let consumption = dispatch_chain(
        &mut [&mut child, &mut parent],
        Delta {
            amount: Vector::new(0.0, 30.0),
        },
        false,
    );

    assert_eq!(child.offset(), Vector::new(0.0, 20.0));
    assert_eq!(consumption.consumed, Vector::new(0.0, 30.0));
    assert_eq!(consumption.remainder, Vector::ZERO);
    assert_eq!(parent.offset(), Vector::new(0.0, 30.0));
}

#[test]
fn a_resize_mid_gesture_reclamps_without_producing_a_stuck_offset() {
    let mut child = Coordinator::new(extent(500.0));

    let _ = child.dispatch(
        Delta {
            amount: Vector::new(0.0, 400.0),
        },
        false,
    );

    assert_eq!(child.offset(), Vector::new(0.0, 400.0));

    // Content shrinks mid-gesture (e.g. items were removed); the
    // existing offset must be reclamped immediately, not left dangling
    // past the new, smaller extent.
    child.set_extent(extent(50.0));

    assert_eq!(child.offset(), Vector::new(0.0, 50.0));

    // A subsequent dispatch behaves exactly as if the coordinator had
    // always had this extent -- no residual "memory" of the old range
    // causes it to accept a delta it should reject outright.
    let consumption = child.dispatch(
        Delta {
            amount: Vector::new(0.0, 10.0),
        },
        false,
    );

    assert_eq!(consumption.consumed, Vector::ZERO);
    assert_eq!(consumption.remainder, Vector::new(0.0, 10.0));
}

#[test]
fn an_empty_chain_reports_the_full_delta_as_remainder() {
    let consumption = dispatch_chain(
        &mut [],
        Delta {
            amount: Vector::new(3.0, 4.0),
        },
        false,
    );

    assert_eq!(consumption.consumed, Vector::ZERO);
    assert_eq!(consumption.remainder, Vector::new(3.0, 4.0));
}

#[test]
fn a_diagonal_delta_is_handed_off_component_wise() {
    // A horizontally-scrolling carousel nested inside a
    // vertically-scrolling page: the child only has room on the x axis,
    // the parent only on the y axis, so a single diagonal delta should
    // split cleanly between them with nothing left over.
    let mut carousel = Coordinator::new(Extent {
        min: Vector::ZERO,
        max: Vector::new(30.0, 0.0),
    });
    let mut page = Coordinator::new(Extent {
        min: Vector::ZERO,
        max: Vector::new(0.0, 300.0),
    });

    let consumption = dispatch_chain(
        &mut [&mut carousel, &mut page],
        Delta {
            amount: Vector::new(50.0, 50.0),
        },
        false,
    );

    assert_eq!(carousel.offset(), Vector::new(30.0, 0.0));
    assert_eq!(page.offset(), Vector::new(0.0, 50.0));
    assert_eq!(consumption.consumed, Vector::new(30.0, 50.0));
    assert_eq!(consumption.remainder, Vector::new(20.0, 0.0));
}

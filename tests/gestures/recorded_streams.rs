//! Deterministic multi-device event replays through
//! `aura_runtime::pointer_router::Router` combined with
//! `aura_gesture::arena::Arena`, `Tap`, `Pan`, and `LongPress`.
//!
//! Each scenario here is a fixed, hand-authored sequence of
//! [`aura_core::pointer::Pointer`] events (a recorded "stream") replayed
//! through a fresh [`Router`] twice, asserting the exact same sequence
//! of [`Routing`] decisions comes out both times -- the concrete
//! meaning of `phase.md`'s exit-gate requirement "recorded streams are
//! deterministic." A stream also mixes device kinds (mouse and touch
//! sharing one [`Router`], each with independent [`aura_core::pointer::Id`]s)
//! to prove routing for one contact never depends on another
//! unrelated contact happening to share the same tick.

use aura_gesture::arena::{Arena, Priority, Resolution, Team};
use aura_gesture::pan::Pan;
use aura_gesture::tap::Tap;

use aura_core::pointer::{DeviceKind, Id as PointerId, Metadata, Phase, Pointer};
use aura_core::time::{Duration, Instant};
use aura_core::widget::visit::Visit;
use aura_core::{Point, Rectangle, Size};

use aura_runtime::hit_test::{self, Path};
use aura_runtime::pointer_router::{Router, Routing};

fn epoch() -> Instant {
    static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

fn t(millis: u64) -> Instant {
    epoch() + Duration::from_millis(millis)
}

fn pointer(id: u64, device: DeviceKind, phase: Phase, x: f32, y: f32, millis: u64) -> Pointer {
    Pointer {
        id: PointerId::from_raw(id),
        device,
        phase,
        position: Point::new(x, y),
        timestamp: t(millis),
        metadata: Metadata::NONE,
        captured: false,
    }
}

/// A trivial always-contains-everything node, solely so
/// `hit_test::hit_test` can produce a real [`Path`] for
/// [`Router::start`] -- `Path` has no public constructor of its own
/// (see `runtime/src/pointer_router.rs`'s own tests for the same
/// pattern).
#[derive(Clone)]
struct Anywhere;

impl Visit for Anywhere {
    fn semantics(&mut self) -> Option<aura_core::semantics::Node> {
        None
    }

    fn contains(&self, _point: Point) -> bool {
        true
    }

    fn bounds(&self) -> Rectangle {
        Rectangle::new(Point::ORIGIN, Size::new(1.0, 1.0))
    }

    fn children(&mut self, _visit: &mut dyn FnMut(&mut dyn Visit)) {}
}

fn any_path() -> Path {
    hit_test::hit_test(&mut Anywhere, Point::ORIGIN).expect("Anywhere always contains the origin")
}

/// One entry in a recorded stream: the pointer event, plus (for a
/// `Down` with no existing capture) the fresh `Arena` a real caller
/// would register recognizers into for that contact.
enum Recorded {
    Down {
        id: u64,
        device: DeviceKind,
        x: f32,
        y: f32,
        millis: u64,
        arena: fn() -> Arena,
    },
    Continue {
        id: u64,
        device: DeviceKind,
        phase: Phase,
        x: f32,
        y: f32,
        millis: u64,
    },
}

fn replay(router: &mut Router, stream: &[Recorded]) -> Vec<Routing> {
    stream
        .iter()
        .map(|entry| match entry {
            Recorded::Down {
                id,
                device,
                x,
                y,
                millis,
                arena,
            } => {
                let event = pointer(*id, *device, Phase::Down, *x, *y, *millis);
                let routing = router.route(event);

                if matches!(routing, Routing::None) {
                    router.start(PointerId::from_raw(*id), any_path(), arena());
                    router.route(pointer(*id, *device, Phase::Down, *x, *y, *millis))
                } else {
                    routing
                }
            }
            Recorded::Continue {
                id,
                device,
                phase,
                x,
                y,
                millis,
            } => router.route(pointer(*id, *device, *phase, *x, *y, *millis)),
        })
        .collect()
}

fn tap_and_pan_arena() -> Arena {
    let mut arena = Arena::new();
    let _ = arena.register(Tap::new(), Team::solo(), Priority::NORMAL);
    let _ = arena.register(Pan::new(), Team::solo(), Priority::NORMAL);
    arena
}

#[test]
fn a_single_finger_tap_stream_resolves_identically_on_replay() {
    let stream = vec![
        Recorded::Down {
            id: 1,
            device: DeviceKind::Touch,
            x: 10.0,
            y: 10.0,
            millis: 0,
            arena: tap_and_pan_arena,
        },
        Recorded::Continue {
            id: 1,
            device: DeviceKind::Touch,
            phase: Phase::Up,
            x: 10.0,
            y: 10.0,
            millis: 40,
        },
    ];

    let mut first_router = Router::new();
    let mut second_router = Router::new();

    let first = replay(&mut first_router, &stream);
    let second = replay(&mut second_router, &stream);

    assert_eq!(first, second);
    assert!(matches!(
        first[1],
        Routing::Resolved {
            resolution: Resolution::Won(_),
            ..
        }
    ));
}

#[test]
fn a_mouse_and_a_touch_contact_interleaved_do_not_interfere() {
    let stream = vec![
        Recorded::Down {
            id: 0,
            device: DeviceKind::Mouse,
            x: 5.0,
            y: 5.0,
            millis: 0,
            arena: tap_and_pan_arena,
        },
        Recorded::Down {
            id: 1,
            device: DeviceKind::Touch,
            x: 200.0,
            y: 200.0,
            millis: 1,
            arena: tap_and_pan_arena,
        },
        // The mouse pans far away while the touch stays put -- each
        // contact's own arena must resolve independently.
        Recorded::Continue {
            id: 0,
            device: DeviceKind::Mouse,
            phase: Phase::Move,
            x: 500.0,
            y: 5.0,
            millis: 20,
        },
        Recorded::Continue {
            id: 1,
            device: DeviceKind::Touch,
            phase: Phase::Up,
            x: 200.0,
            y: 200.0,
            millis: 40,
        },
    ];

    let mut first_router = Router::new();
    let mut second_router = Router::new();

    let first = replay(&mut first_router, &stream);
    let second = replay(&mut second_router, &stream);

    assert_eq!(first, second);

    // Mouse (index 2) resolved via Pan (moved past slop) and stays
    // captured (this stream never sends it an Up/Cancel); touch (index
    // 3) resolved via Tap and, having also just terminated on Up, is no
    // longer tracked at all -- each contact's own arena resolved
    // independently of the other.
    assert!(matches!(first[2], Routing::Resolved { .. }));
    assert!(matches!(first[3], Routing::Resolved { .. }));

    assert!(first_router.is_captured(PointerId::from_raw(0)));
    assert!(!first_router.is_captured(PointerId::from_raw(1)));
}

#[test]
fn a_captured_pointer_keeps_routing_far_outside_its_starting_position() {
    let stream = vec![
        Recorded::Down {
            id: 1,
            device: DeviceKind::Touch,
            x: 0.0,
            y: 0.0,
            millis: 0,
            arena: tap_and_pan_arena,
        },
        // Cross Pan's slop to win capture...
        Recorded::Continue {
            id: 1,
            device: DeviceKind::Touch,
            phase: Phase::Move,
            x: 100.0,
            y: 0.0,
            millis: 10,
        },
        // ...then move somewhere the original hit-test position would
        // never have reached at all. A captured pointer must still
        // route (to the same Path), per "captured pointers keep
        // routing outside original bounds."
        Recorded::Continue {
            id: 1,
            device: DeviceKind::Touch,
            phase: Phase::Move,
            x: 9_999.0,
            y: 9_999.0,
            millis: 20,
        },
        Recorded::Continue {
            id: 1,
            device: DeviceKind::Touch,
            phase: Phase::Up,
            x: 9_999.0,
            y: 9_999.0,
            millis: 30,
        },
    ];

    let mut router = Router::new();
    let routings = replay(&mut router, &stream);

    assert!(matches!(routings[1], Routing::Resolved { .. }));
    assert!(matches!(routings[2], Routing::Captured { .. }));
    assert!(matches!(routings[3], Routing::Captured { .. }));

    // Every event after Up leaves no dangling tracked contact behind.
    assert_eq!(router.tracked_ids().count(), 0);
}

#[test]
fn a_cancelled_stream_leaves_no_pointer_tracked_afterward() {
    let stream = vec![
        Recorded::Down {
            id: 1,
            device: DeviceKind::Touch,
            x: 0.0,
            y: 0.0,
            millis: 0,
            arena: tap_and_pan_arena,
        },
        Recorded::Continue {
            id: 1,
            device: DeviceKind::Touch,
            phase: Phase::Cancel,
            x: 0.0,
            y: 0.0,
            millis: 5,
        },
    ];

    let mut first_router = Router::new();
    let mut second_router = Router::new();

    let first = replay(&mut first_router, &stream);
    let second = replay(&mut second_router, &stream);

    assert_eq!(first, second);
    assert!(matches!(
        first[1],
        Routing::Resolved {
            resolution: Resolution::Cancelled,
            ..
        }
    ));
    assert_eq!(first_router.tracked_ids().count(), 0);
}

#[test]
fn a_five_finger_multitouch_stream_resolves_every_contact_deterministically() {
    let mut stream = Vec::new();

    for finger in 0..5u64 {
        stream.push(Recorded::Down {
            id: finger,
            device: DeviceKind::Touch,
            x: finger as f32 * 100.0,
            y: 0.0,
            millis: finger,
            arena: tap_and_pan_arena,
        });
    }

    for finger in 0..5u64 {
        stream.push(Recorded::Continue {
            id: finger,
            device: DeviceKind::Touch,
            phase: Phase::Up,
            x: finger as f32 * 100.0,
            y: 0.0,
            millis: 50 + finger,
        });
    }

    let mut first_router = Router::new();
    let mut second_router = Router::new();

    let first = replay(&mut first_router, &stream);
    let second = replay(&mut second_router, &stream);

    assert_eq!(first, second);

    // Every finger's lift (the second half of the stream) resolved its
    // own arena via Tap.
    for routing in &first[5..10] {
        assert!(matches!(
            routing,
            Routing::Resolved {
                resolution: Resolution::Won(_),
                ..
            }
        ));
    }

    assert_eq!(first_router.tracked_ids().count(), 0);
}

#[test]
fn tick_driven_long_press_resolution_is_deterministic_across_replays() {
    use aura_gesture::long_press::LongPress;

    fn long_press_and_tap_arena() -> Arena {
        let mut arena = Arena::new();
        let _ = arena.register(LongPress::new(), Team::solo(), Priority::NORMAL);
        let _ = arena.register(Tap::new(), Team::solo(), Priority::NORMAL);
        arena
    }

    let down = Recorded::Down {
        id: 1,
        device: DeviceKind::Touch,
        x: 0.0,
        y: 0.0,
        millis: 0,
        arena: long_press_and_tap_arena,
    };

    let mut first_router = Router::new();
    let mut second_router = Router::new();

    let first_down = replay(&mut first_router, std::slice::from_ref(&down));
    let second_down = replay(&mut second_router, std::slice::from_ref(&down));

    assert_eq!(first_down, second_down);
    assert!(!first_router.is_captured(PointerId::from_raw(1)));

    let first_tick = first_router.tick(t(500));
    let second_tick = second_router.tick(t(500));

    assert_eq!(first_tick.len(), 1);
    assert_eq!(second_tick.len(), 1);
    assert_eq!(first_tick[0].0, second_tick[0].0);
    assert!(matches!(
        first_tick[0].1,
        Routing::Resolved {
            resolution: Resolution::Won(_),
            ..
        }
    ));
    assert!(first_router.is_captured(PointerId::from_raw(1)));
}

//! Routes pointer events to gesture competitions, without re-hit-testing
//! or re-competing once a pointer is captured, and coalesces
//! high-frequency move events per `core::event::Coalescing`.
//!
//! [`Router`] is the piece that turns `taim_gesture::Arena` (one
//! contact's competition) and `hit_test::Index` (spatial lookup) into
//! something a runtime can actually drive across many contacts and many
//! frames:
//!
//! - **Capture table**: once an [`taim_gesture::arena::Resolution::Won`]
//!   names a winner, [`Router`] remembers which [`hit_test::Path`] owns
//!   that [`taim_core::pointer::Id`] for the remainder of its lifetime.
//!   Every subsequent event for that pointer routes directly to the
//!   capture, skipping [`hit_test::Index::hit_test`] and arena
//!   competition entirely -- the concrete meaning of `phase.md`'s
//!   "captured pointers keep routing outside original bounds": there is
//!   no bounds check left to fail once captured.
//! - **Arena routing**: a `Down` event with no existing capture is
//!   hit-tested, and a fresh `Arena` is created for that contact,
//!   tracked until it resolves.
//! - **Move coalescing**: [`Router::route`] takes a single event at a
//!   time (the routing decision itself never needs to see more than
//!   one), but [`coalesce`] is provided separately for a caller with a
//!   backlog of queued events to collapse before ever reaching
//!   [`Router::route`], per [`taim_core::event::Coalescing`]'s
//!   contract.
use crate::core::pointer::{Id as PointerId, Phase, Pointer};

use taim_gesture::arena::{Arena, Resolution};

use crate::hit_test::Path;

/// What [`Router::route`] decided to do with one [`Pointer`] event.
#[derive(Debug, Clone, PartialEq)]
pub enum Routing {
    /// No arena exists for this pointer and none was started (e.g. a
    /// `Move`/`Up`/`Cancel` for an [`taim_core::pointer::Id`] the router
    /// has never seen a `Down` for, which should not normally happen but
    /// is not this module's job to treat as an error).
    None,

    /// The pointer is still competing; an [`Arena`] exists for it but
    /// has not yet resolved. `path` is the hit-tested origin
    /// [`Path`] the arena was started at (down position), for a caller
    /// that wants to know where a still-pending gesture began.
    Competing {
        /// The [`Path`] the arena was started at.
        path: Path,
    },

    /// The pointer's arena resolved this event; `path` is the winning
    /// [`Path`] and `resolution` the [`Resolution`] the arena itself
    /// returned. From this event onward (including this one), the
    /// pointer is captured to `path`.
    Resolved {
        /// The [`Path`] whose arena resolved.
        path: Path,
        /// The [`Resolution`] the arena reported.
        resolution: Resolution,
    },

    /// The pointer is captured (a prior event already resolved its
    /// arena); `path` is the capture target this event routes to
    /// directly, with no hit-testing or arena competition involved.
    Captured {
        /// The captured [`Path`].
        path: Path,
    },
}

struct Contact {
    path: Path,
    arena: Option<Arena>,
}

/// Routes [`Pointer`] events for many concurrent contacts, maintaining a
/// capture table and one [`Arena`] per still-competing contact.
#[derive(Default)]
pub struct Router {
    contacts: rustc_hash::FxHashMap<PointerId, Contact>,
}

impl std::fmt::Debug for Router {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Router")
            .field("tracked_contacts", &self.contacts.len())
            .finish()
    }
}

impl Router {
    /// Creates an empty [`Router`] with no tracked contacts.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns whether `id` is currently captured (its arena already
    /// resolved to a winner).
    pub fn is_captured(&self, id: PointerId) -> bool {
        self.contacts
            .get(&id)
            .is_some_and(|contact| contact.arena.is_none())
    }

    /// Returns the captured [`Path`] for `id`, if any.
    pub fn captured_path(&self, id: PointerId) -> Option<&Path> {
        let contact = self.contacts.get(&id)?;

        contact.arena.is_none().then_some(&contact.path)
    }

    /// Starts tracking a new contact at `id`, hit-tested to `path`, with
    /// the given freshly-registered `arena` -- called on a `Down` event
    /// with no existing capture, once the caller has hit-tested the
    /// position and registered whatever recognizers apply along `path`
    /// into `arena`.
    ///
    /// # Panics
    /// Panics if `id` is already tracked -- a `Down` for a contact that
    /// is already down is a caller bug (every contact's lifetime starts
    /// with exactly one `Down`, per
    /// [`taim_core::pointer::Phase::Down`]'s own contract).
    pub fn start(&mut self, id: PointerId, path: Path, arena: Arena) {
        assert!(
            !self.contacts.contains_key(&id),
            "pointer {id:?} is already tracked; Down must not be dispatched twice for the same contact"
        );

        let _ = self.contacts.insert(
            id,
            Contact {
                path,
                arena: Some(arena),
            },
        );
    }

    /// Routes `pointer` through this [`Router`]: dispatches it into the
    /// tracked [`Arena`] if one is still competing, records the capture
    /// if it just resolved, or routes directly if already captured.
    ///
    /// Returns [`Routing::None`] if no contact is tracked for
    /// `pointer.id` at all (see [`Routing::None`]'s own documentation).
    /// Terminal events ([`Phase::Up`]/[`Phase::Cancel`]) remove the
    /// contact from this [`Router`] after routing, so a later `Down`
    /// reusing the same [`PointerId`] (a mouse, which has exactly one
    /// fixed [`PointerId`] across separate press/release cycles) is
    /// treated as a genuinely new contact rather than tripping
    /// [`Router::start`]'s "already tracked" panic.
    pub fn route(&mut self, pointer: Pointer) -> Routing {
        // Direct users of Router may bypass core::Event::to_pointer.
        // Enforce the same finite-coordinate/cancellation contract here.
        // Invalid Move/Down is ignored, whereas invalid Up/Cancel is
        // converted to a safe Cancel so captured contacts are released.
        let Some(pointer) = pointer.sanitize_coordinates() else {
            // Hit-testing may already have prepared an arena for Down.
            // Rejecting that malformed initial sample must also discard
            // the pending arena, otherwise it would leak until focus reset.
            // A stray invalid Move must NOT discard existing capture.
            if matches!(pointer.phase, Phase::Down) {
                self.cancel(pointer.id);
            }
            return Routing::None;
        };
        let is_terminal = pointer.is_terminal();

        let routing = match self.contacts.get_mut(&pointer.id) {
            None => Routing::None,
            Some(contact) => match &mut contact.arena {
                None => Routing::Captured {
                    path: contact.path.clone(),
                },
                Some(arena) => match arena.dispatch(pointer) {
                    None => Routing::Competing {
                        path: contact.path.clone(),
                    },
                    Some(resolution) => {
                        // Only a winning recognizer captures the contact.
                        // `NoWinner` and `Cancelled` resolve the competition
                        // but must not turn the path into a capture target.
                        let won = matches!(resolution, Resolution::Won(_));
                        if won {
                            contact.arena = None;
                        }

                        Routing::Resolved {
                            path: contact.path.clone(),
                            resolution,
                        }
                    }
                },
            },
        };

        if is_terminal
            || matches!(
                routing,
                Routing::Resolved {
                    resolution: Resolution::NoWinner | Resolution::Cancelled,
                    ..
                }
            )
        {
            let _ = self.contacts.remove(&pointer.id);
        }

        routing
    }

    /// Forces the [`Arena`] tracked for `id` to resolve as cancelled
    /// (see [`Arena::cancel`]), removing `id` from this [`Router`]
    /// regardless of the outcome -- for a caller that knows a contact's
    /// interaction is being abandoned for a reason no [`Pointer`] event
    /// itself captures (e.g. its owning widget left the tree).
    ///
    /// A no-op if `id` is not tracked, or is already captured (an
    /// already-resolved arena has nothing left to cancel; the capture
    /// itself is untouched by this method, matching how a captured
    /// pointer's `Up`/`Cancel` -- not an external abandonment -- is the
    /// normal way a capture ends).
    pub fn cancel(&mut self, id: PointerId) {
        let was_competing = self
            .contacts
            .get_mut(&id)
            .and_then(|contact| contact.arena.as_mut())
            .map(|arena| {
                let _ = arena.cancel();
            })
            .is_some();

        if was_competing {
            let _ = self.contacts.remove(&id);
        }
    }

    /// Advances every still-competing [`Arena`] via [`Arena::tick`],
    /// resolving any whose deadline-based recognizers ([`LongPress`],
    /// most notably) have now elapsed, returning the [`Routing`] for
    /// each contact that changed state this tick.
    ///
    /// [`LongPress`]: taim_gesture::long_press::LongPress
    pub fn tick(&mut self, now: crate::core::time::Instant) -> Vec<(PointerId, Routing)> {
        let mut changes = Vec::new();

        for (&id, contact) in &mut self.contacts {
            if let Some(arena) = &mut contact.arena
                && let Some(resolution) = arena.tick(now)
            {
                if matches!(resolution, Resolution::Won(_)) {
                    contact.arena = None;
                }

                changes.push((
                    id,
                    Routing::Resolved {
                        path: contact.path.clone(),
                        resolution,
                    },
                ));
            }
        }

        for (id, routing) in &changes {
            if matches!(
                routing,
                Routing::Resolved {
                    resolution: Resolution::NoWinner | Resolution::Cancelled,
                    ..
                }
            ) {
                let _ = self.contacts.remove(id);
            }
        }

        changes
    }

    /// Removes every tracked contact, as if none had ever been started
    /// -- for a caller resetting the whole router (e.g. the window lost
    /// focus and every in-flight gesture must be abandoned at once).
    ///
    /// Unlike [`Router::cancel`], this does not resolve any tracked
    /// [`Arena`] as cancelled first; a caller that needs every losing
    /// recognizer properly notified should call [`Router::cancel`] for
    /// each tracked [`PointerId`] (see [`Router::tracked_ids`]) before
    /// this, if that distinction matters for its use case.
    pub fn clear(&mut self) {
        self.contacts.clear();
    }

    /// Returns every currently-tracked [`PointerId`], competing or
    /// captured.
    pub fn tracked_ids(&self) -> impl Iterator<Item = PointerId> + '_ {
        self.contacts.keys().copied()
    }
}

/// Coalesces a backlog of pointer-derived events for the same contact,
/// per [`taim_core::event::Coalescing`]: consecutive
/// [`Phase::Move`] events collapse to the latest one, while every
/// [`Phase::Down`]/[`Phase::Up`]/[`Phase::Cancel`] is preserved
/// individually and in order -- the concrete meaning of `phase.md`'s
/// "high-frequency moves may be coalesced, but down/up/cancel and
/// semantic boundary events may not be dropped."
///
/// `events` must already be sorted by [`Pointer::timestamp`] (the same
/// order they actually occurred in); this function only merges adjacent
/// mergeable events, it does not reorder anything.
pub fn coalesce(events: Vec<Pointer>) -> Vec<Pointer> {
    let mut result: Vec<Pointer> = Vec::with_capacity(events.len());

    for event in events {
        let can_merge_with_previous = event.phase == Phase::Move
            && result
                .last()
                .is_some_and(|previous| previous.id == event.id && previous.phase == Phase::Move);

        if can_merge_with_previous {
            *result.last_mut().unwrap() = event;
        } else {
            result.push(event);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::pointer::{DeviceKind, Metadata};
    use crate::core::time::{Duration, Instant};
    use taim_gesture::arena::{Priority, Team};
    use taim_gesture::recognizer::{Disposition, Recognizer, Verdict};

    fn epoch() -> Instant {
        static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        *EPOCH.get_or_init(Instant::now)
    }

    fn t(millis: u64) -> Instant {
        epoch() + Duration::from_millis(millis)
    }

    fn pointer(id: u64, phase: Phase, millis: u64) -> Pointer {
        Pointer {
            id: PointerId::from_raw(id),
            device: DeviceKind::Touch,
            phase,
            position: crate::core::Point::ORIGIN,
            timestamp: t(millis),
            metadata: Metadata::NONE,
            captured: false,
        }
    }

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

    fn path(indices: &[usize]) -> Path {
        // `hit_test::Path` has no public constructor besides the
        // oracle itself; build one via a trivial synthetic hit test.
        use crate::core::widget::visit::Visit;
        use crate::core::{Rectangle, Size};

        /// A synthetic chain shaped so the oracle's own descent produces
        /// exactly `target`: at each level it offers `target[depth] + 1`
        /// children and only the last of them contains the point, so
        /// `descend` selects index `target[depth]`.
        ///
        /// An earlier version of this helper offered a single child per
        /// level regardless of `target`, so it produced `[0, 0, ..]` for
        /// *every* input -- `path(&[1, 2])` silently meant `[0, 0]`, and
        /// two paths meant to differ compared equal.
        #[derive(Clone)]
        struct Node {
            depth: usize,
            target: Vec<usize>,
            hits: bool,
        }

        impl Visit for Node {
            fn semantics(&mut self) -> Option<crate::core::semantics::Node> {
                None
            }

            fn contains(&self, _point: crate::core::Point) -> bool {
                self.hits
            }

            fn bounds(&self) -> Rectangle {
                Rectangle::new(crate::core::Point::ORIGIN, Size::new(1.0, 1.0))
            }

            fn children(&mut self, visit: &mut dyn FnMut(&mut dyn Visit)) {
                let Some(&wanted) = self.target.get(self.depth) else {
                    return;
                };

                for index in 0..=wanted {
                    visit(&mut Node {
                        depth: self.depth + 1,
                        target: self.target.clone(),
                        hits: index == wanted,
                    });
                }
            }
        }

        let mut root = Node {
            depth: 0,
            target: indices.to_vec(),
            hits: true,
        };

        let found = crate::hit_test::hit_test(&mut root, crate::core::Point::ORIGIN)
            .expect("synthetic root always contains the origin");

        assert_eq!(
            found.indices(),
            indices,
            "the synthetic tree must produce the requested path"
        );

        found
    }

    #[test]
    fn a_lone_recognizer_resolves_on_the_first_routed_event() {
        let mut router = Router::new();
        let mut arena = Arena::new();
        let _ = arena.register(AlwaysAccepts, Team::solo(), Priority::NORMAL);

        router.start(PointerId::from_raw(1), path(&[0]), arena);

        let routing = router.route(pointer(1, Phase::Down, 0));

        assert!(matches!(routing, Routing::Resolved { .. }));
        assert!(router.is_captured(PointerId::from_raw(1)));
    }

    #[test]
    fn a_captured_pointer_routes_directly_without_an_arena() {
        let mut router = Router::new();
        let mut arena = Arena::new();
        let _ = arena.register(AlwaysAccepts, Team::solo(), Priority::NORMAL);

        router.start(PointerId::from_raw(1), path(&[0]), arena);
        let _ = router.route(pointer(1, Phase::Down, 0));

        let routing = router.route(pointer(1, Phase::Move, 10));

        assert!(matches!(routing, Routing::Captured { .. }));
    }

    #[test]
    fn a_terminal_event_removes_the_contact() {
        let mut router = Router::new();
        let mut arena = Arena::new();
        let _ = arena.register(AlwaysAccepts, Team::solo(), Priority::NORMAL);

        router.start(PointerId::from_raw(1), path(&[0]), arena);
        let _ = router.route(pointer(1, Phase::Down, 0));
        let _ = router.route(pointer(1, Phase::Up, 10));

        assert_eq!(router.route(pointer(1, Phase::Move, 20)), Routing::None);
        assert!(!router.is_captured(PointerId::from_raw(1)));
    }

    #[test]
    fn invalid_initial_down_cancels_the_prepared_pending_arena() {
        let mut router = Router::new();
        let mut arena = Arena::new();
        let _ = arena.register(AlwaysAccepts, Team::solo(), Priority::NORMAL);
        let id = PointerId::from_raw(5);
        router.start(id, path(&[0]), arena);

        let invalid_down = Pointer {
            position: crate::core::Point::new(f32::NAN, 0.0),
            ..pointer(5, Phase::Down, 0)
        };
        assert_eq!(router.route(invalid_down), Routing::None);
        assert_eq!(router.tracked_ids().count(), 0);
    }

    #[test]
    fn malformed_pointer_motion_never_poisoned_capture_and_invalid_up_cancels() {
        let mut router = Router::new();
        let mut arena = Arena::new();
        let _ = arena.register(AlwaysAccepts, Team::solo(), Priority::NORMAL);
        let id = PointerId::from_raw(1);
        router.start(id, path(&[0]), arena);
        let _ = router.route(pointer(1, Phase::Down, 0));
        assert!(router.is_captured(id));

        let malformed_motion = Pointer {
            position: crate::core::Point::new(f32::NAN, 0.0),
            ..pointer(1, Phase::Move, 1)
        };
        assert_eq!(router.route(malformed_motion), Routing::None);
        assert!(router.is_captured(id));

        let malformed_release = Pointer {
            position: crate::core::Point::new(f32::INFINITY, 0.0),
            ..pointer(1, Phase::Up, 2)
        };
        let result = router.route(malformed_release);
        assert!(matches!(result, Routing::Captured { .. }));
        assert!(!router.is_captured(id));
        assert_eq!(router.tracked_ids().count(), 0);
    }

    #[test]
    fn routing_an_untracked_pointer_returns_none() {
        let mut router = Router::new();

        assert_eq!(router.route(pointer(99, Phase::Move, 0)), Routing::None);
    }

    #[test]
    fn a_reused_mouse_id_can_start_again_after_a_terminal_event() {
        let mut router = Router::new();

        let mut first_arena = Arena::new();
        let _ = first_arena.register(AlwaysAccepts, Team::solo(), Priority::NORMAL);
        router.start(PointerId::MOUSE, path(&[0]), first_arena);
        let _ = router.route(Pointer {
            id: PointerId::MOUSE,
            ..pointer(0, Phase::Down, 0)
        });
        let _ = router.route(Pointer {
            id: PointerId::MOUSE,
            ..pointer(0, Phase::Up, 10)
        });

        let mut second_arena = Arena::new();
        let _ = second_arena.register(AlwaysAccepts, Team::solo(), Priority::NORMAL);

        // Must not panic (the earlier contact was already removed).
        router.start(PointerId::MOUSE, path(&[1]), second_arena);
    }

    #[test]
    #[should_panic(expected = "already tracked")]
    fn starting_an_already_tracked_pointer_panics() {
        let mut router = Router::new();
        let mut arena = Arena::new();
        let _ = arena.register(AlwaysAccepts, Team::solo(), Priority::NORMAL);
        router.start(PointerId::from_raw(1), path(&[0]), arena);

        let mut other_arena = Arena::new();
        let _ = other_arena.register(AlwaysAccepts, Team::solo(), Priority::NORMAL);
        router.start(PointerId::from_raw(1), path(&[0]), other_arena);
    }

    #[test]
    fn cancel_resolves_a_competing_arena_as_cancelled() {
        struct NeverResolves;

        impl Recognizer for NeverResolves {
            fn name(&self) -> &'static str {
                "never_resolves"
            }

            fn add_pointer(&mut self, _pointer: Pointer) {}
            fn remove_pointer(&mut self, _id: PointerId) {}

            fn on_event(&mut self, _pointer: Pointer) -> Disposition {
                Disposition::Pending
            }

            fn on_verdict(&mut self, _verdict: Verdict) {}
            fn reset(&mut self) {}
        }

        let mut router = Router::new();
        let mut arena = Arena::new();
        let _ = arena.register(NeverResolves, Team::solo(), Priority::NORMAL);
        let _ = arena.register(NeverResolves, Team::solo(), Priority::NORMAL);

        router.start(PointerId::from_raw(1), path(&[0]), arena);
        let _ = router.route(pointer(1, Phase::Down, 0));

        router.cancel(PointerId::from_raw(1));

        assert!(!router.is_captured(PointerId::from_raw(1)));
        assert_eq!(router.tracked_ids().count(), 0);
    }

    #[test]
    fn an_empty_arena_never_creates_pointer_capture() {
        let mut router = Router::new();
        router.start(PointerId::from_raw(1), path(&[0]), Arena::new());

        assert!(matches!(
            router.route(pointer(1, Phase::Down, 0)),
            Routing::Competing { .. }
        ));
        assert!(matches!(
            router.route(pointer(1, Phase::Up, 10)),
            Routing::Resolved {
                resolution: Resolution::NoWinner,
                ..
            }
        ));
        assert!(!router.is_captured(PointerId::from_raw(1)));
        assert_eq!(router.tracked_ids().count(), 0);
    }

    #[test]
    fn tick_resolves_a_deadline_based_arena() {
        struct DeadlineOnly {
            deadline: Instant,
        }

        impl Recognizer for DeadlineOnly {
            fn name(&self) -> &'static str {
                "deadline_only"
            }

            fn add_pointer(&mut self, _pointer: Pointer) {}
            fn remove_pointer(&mut self, _id: PointerId) {}

            fn on_event(&mut self, _pointer: Pointer) -> Disposition {
                Disposition::Pending
            }

            fn poll(&mut self, now: Instant) -> Disposition {
                if now >= self.deadline {
                    Disposition::Accepted
                } else {
                    Disposition::Pending
                }
            }

            fn on_verdict(&mut self, _verdict: Verdict) {}
            fn reset(&mut self) {}
        }

        let mut router = Router::new();
        let mut arena = Arena::new();
        let _ = arena.register(
            DeadlineOnly { deadline: t(500) },
            Team::solo(),
            Priority::NORMAL,
        );
        let _ = arena.register(
            DeadlineOnly {
                deadline: t(10_000),
            },
            Team::solo(),
            Priority::NORMAL,
        );

        router.start(PointerId::from_raw(1), path(&[0]), arena);
        let _ = router.route(pointer(1, Phase::Down, 0));

        assert!(router.tick(t(100)).is_empty());

        let changes = router.tick(t(500));
        assert_eq!(changes.len(), 1);
        assert!(router.is_captured(PointerId::from_raw(1)));
    }

    #[test]
    fn coalesce_merges_consecutive_moves_for_the_same_pointer() {
        let events = vec![
            pointer(1, Phase::Down, 0),
            pointer(1, Phase::Move, 10),
            pointer(1, Phase::Move, 20),
            pointer(1, Phase::Move, 30),
            pointer(1, Phase::Up, 40),
        ];

        let coalesced = coalesce(events);

        assert_eq!(coalesced.len(), 3);
        assert_eq!(coalesced[0].phase, Phase::Down);
        assert_eq!(coalesced[1].phase, Phase::Move);
        assert_eq!(coalesced[1].timestamp, t(30));
        assert_eq!(coalesced[2].phase, Phase::Up);
    }

    #[test]
    fn coalesce_does_not_merge_moves_from_different_pointers() {
        let events = vec![
            pointer(1, Phase::Down, 0),
            pointer(2, Phase::Down, 0),
            pointer(1, Phase::Move, 10),
            pointer(2, Phase::Move, 10),
        ];

        let coalesced = coalesce(events);

        assert_eq!(coalesced.len(), 4);
    }

    #[test]
    fn coalesce_never_drops_down_up_or_cancel() {
        let events = vec![
            pointer(1, Phase::Down, 0),
            pointer(1, Phase::Up, 10),
            pointer(2, Phase::Down, 10),
            pointer(2, Phase::Cancel, 20),
        ];

        let coalesced = coalesce(events.clone());

        assert_eq!(coalesced, events);
    }

    #[test]
    fn clear_removes_every_tracked_contact() {
        let mut router = Router::new();
        let mut arena = Arena::new();
        let _ = arena.register(AlwaysAccepts, Team::solo(), Priority::NORMAL);

        router.start(PointerId::from_raw(1), path(&[0]), arena);
        let _ = router.route(pointer(1, Phase::Down, 0));

        router.clear();

        assert_eq!(router.tracked_ids().count(), 0);
    }
}

//! The gesture competition/resolution/capture engine.
//!
//! An [`Arena`] holds every [`Recognizer`] currently competing for one
//! logical pointer sequence (usually: everything a single widget/subtree
//! registered for the contact(s) currently pressed on it). Each incoming
//! [`Pointer`] event is fanned out to every still-pending member; the
//! arena resolves to at most one winner as soon as the evidence allows,
//! following these rules (matching `phase.md`'s own correctness rules
//! verbatim):
//!
//! - Every pointer sequence ends in up or cancel for all participants.
//! - Losing recognizers receive cancellation exactly once.
//! - Captured pointers keep routing outside their original bounds.
//!
//! # Resolution order
//!
//! On every event, after every still-pending member has been polled:
//!
//! 1. If any member (or [`Team`]) reports [`Disposition::Accepted`],
//!    that member/team wins immediately -- this is the "eager win" rule
//!    real touch UIs need (a recognized long-press should not wait for
//!    every other candidate to also finish deciding).
//! 2. If two or more members report [`Disposition::Accepted`] in the
//!    very same resolution pass (they raced), the one with the highest
//!    [`Priority`] wins; a further tie is broken by earliest
//!    registration order, so resolution is always deterministic given
//!    the same registration sequence and event stream.
//! 3. If every member but one has reported [`Disposition::Rejected`],
//!    the last remaining pending member wins by default, without
//!    needing to itself report [`Disposition::Accepted`] -- exactly the
//!    "last one standing" rule that lets a single registered recognizer
//!    (the overwhelmingly common case) win as soon as its lone
//!    competitor concedes, or on the very first event if it was the only
//!    entrant to begin with.
//! 4. If every member reports [`Disposition::Rejected`] with none ever
//!    accepting, the arena resolves with no winner at all; every member
//!    already received its own [`Verdict::Lost`] as it individually
//!    rejected, so there is nothing further to notify.
//!
//! # Teams
//!
//! A [`Team`] groups recognizers that must win or lose *together* (e.g.
//! several recognizers internal to one composite widget that should
//! never compete against each other, only against gestures from other
//! widgets). Members of the same [`Team`] are still each polled and each
//! report their own [`Disposition`], but the team as a whole is only
//! rejected once *every* member of it has rejected, and any one member
//! accepting immediately accepts the whole team.
use crate::recognizer::{Disposition, Recognizer, Verdict};
use aura_core::pointer::{Id as PointerId, Pointer};
use aura_core::time::Instant;

/// The relative priority of a competing entrant, used only to break a
/// same-pass tie between multiple simultaneous
/// [`Disposition::Accepted`] reports (see this module's own resolution
/// order documentation, rule 2). Most competitions never need this --
/// they resolve via rule 1 or rule 3 with only one accepter -- but a
/// deliberately prioritized gesture (e.g. a system-reserved
/// [`crate::edge_swipe`] outranking an ordinary content pan starting at
/// the same edge) needs a way to say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Priority(pub i32);

impl Priority {
    /// The default priority ordinary content recognizers should use.
    pub const NORMAL: Self = Self(0);

    /// A higher-than-normal priority for system-reserved gestures (see
    /// [`crate::edge_swipe`]), which must win a same-pass tie against
    /// ordinary content gestures.
    pub const SYSTEM: Self = Self(100);
}

/// A team identifier grouping recognizers that must win or lose
/// together; see this module's own "Teams" documentation.
///
/// Two entrants with the same [`Team`] value are on the same team;
/// [`Team::solo`] (the default every entrant gets unless told
/// otherwise) guarantees a fresh, never-repeated value so an entrant
/// with no explicit team is never accidentally grouped with another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Team(u64);

impl Team {
    /// Creates a new, explicit [`Team`] identifier. Callers that want
    /// several entrants to share a team should register them all with
    /// the same value returned from one call to this function.
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns a fresh [`Team`] guaranteed not to equal any other
    /// [`Team`] returned by this function, for an entrant that competes
    /// entirely on its own.
    pub fn solo() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT: AtomicU64 = AtomicU64::new(1 << 63);

        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// The outcome of resolving an [`Arena`]'s competition, returned by
/// [`Arena::dispatch`] the moment resolution happens (`None` on every
/// call before then).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// The competition resolved with a winner at the given index (the
    /// same [`EntrantId`] [`Arena::register`] returned for it).
    Won(EntrantId),

    /// The competition resolved with no winner: every entrant rejected
    /// without any of them accepting.
    NoWinner,

    /// The underlying pointer sequence was cancelled before the
    /// competition could resolve on its own.
    Cancelled,
}

/// An opaque handle to a registered entrant, returned by
/// [`Arena::register`] and used to identify the winner in
/// [`Resolution::Won`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntrantId(usize);

struct Entrant {
    recognizer: Box<dyn Recognizer>,
    team: Team,
    priority: Priority,
    disposition: Disposition,
    /// Set once this entrant has received its final `Verdict`, so the
    /// "losing recognizers receive cancellation exactly once"
    /// correctness rule holds even if `resolve` were ever called more
    /// than once (it is not, today, but this makes that impossible to
    /// violate by construction rather than by care).
    notified: bool,
}

/// A single gesture competition: a bounded set of [`Recognizer`]s,
/// fanned an identical [`Pointer`] event stream, resolved down to at
/// most one winner.
///
/// An [`Arena`] is single-use: once it [`Arena::is_resolved`], it exists
/// only so callers can still read [`Arena::resolution`]; register a new
/// [`Arena`] for the next, unrelated contact rather than trying to reuse
/// this one (mirroring [`Recognizer::reset`]'s "reuse the recognizer,
/// not the competition" contract).
pub struct Arena {
    entrants: Vec<Entrant>,
    resolution: Option<Resolution>,
    tracked_pointers: rustc_hash::FxHashSet<PointerId>,
}

impl Default for Arena {
    fn default() -> Self {
        Self::new()
    }
}

impl Arena {
    /// Creates a new, empty [`Arena`] with no entrants yet.
    pub fn new() -> Self {
        Self {
            entrants: Vec::new(),
            resolution: None,
            tracked_pointers: rustc_hash::FxHashSet::default(),
        }
    }

    /// Registers a new [`Recognizer`] as a competing entrant, with the
    /// given [`Team`] and [`Priority`], returning its [`EntrantId`].
    ///
    /// # Panics
    /// Panics if the arena has already resolved (see
    /// [`Arena::is_resolved`]) -- entrants must all be registered before
    /// the first [`Arena::dispatch`] call, since a recognizer joining
    /// mid-competition would not have seen the events that led to
    /// whatever already happened.
    pub fn register(
        &mut self,
        recognizer: impl Recognizer + 'static,
        team: Team,
        priority: Priority,
    ) -> EntrantId {
        assert!(
            self.resolution.is_none(),
            "cannot register a new entrant into an already-resolved Arena"
        );

        let id = EntrantId(self.entrants.len());

        self.entrants.push(Entrant {
            recognizer: Box::new(recognizer),
            team,
            priority,
            disposition: Disposition::Pending,
            notified: false,
        });

        id
    }

    /// Returns whether this [`Arena`] has already resolved (see
    /// [`Arena::resolution`]).
    pub fn is_resolved(&self) -> bool {
        self.resolution.is_some()
    }

    /// Returns the [`Resolution`], if the arena has resolved.
    pub fn resolution(&self) -> Option<Resolution> {
        self.resolution
    }

    /// Dispatches one [`Pointer`] event to every still-pending entrant,
    /// registering/deregistering the pointer with every entrant on
    /// [`aura_core::pointer::Phase::Down`]/terminal phases, and resolves
    /// the competition if the evidence now allows it.
    ///
    /// Returns the [`Resolution`] the moment it happens (on the same
    /// call whose event caused it), and `None` on every call before or
    /// after that (query [`Arena::resolution`] for the latter).
    ///
    /// # Panics
    /// Panics if called after the arena has already resolved -- once
    /// resolved, an arena is done; a resolved widget/subtree should stop
    /// forwarding events to it (a captured winner keeps receiving events
    /// directly, bypassing the arena entirely, per "captured pointers
    /// keep routing outside original bounds").
    pub fn dispatch(&mut self, pointer: Pointer) -> Option<Resolution> {
        assert!(
            self.resolution.is_none(),
            "cannot dispatch into an already-resolved Arena"
        );

        if pointer.phase == aura_core::pointer::Phase::Down {
            let _ = self.tracked_pointers.insert(pointer.id);

            for entrant in &mut self.entrants {
                entrant.recognizer.add_pointer(pointer);
            }
        } else {
            for entrant in &mut self.entrants {
                if entrant.disposition == Disposition::Pending {
                    entrant.disposition = entrant.recognizer.on_event(pointer);
                }
            }
        }

        if pointer.is_terminal() {
            let _ = self.tracked_pointers.remove(&pointer.id);

            for entrant in &mut self.entrants {
                entrant.recognizer.remove_pointer(pointer.id);
            }

            if pointer.phase == aura_core::pointer::Phase::Cancel {
                return self.resolve(Resolution::Cancelled);
            }
        }

        self.try_resolve()
    }

    /// Forces this [`Arena`] to resolve as [`Resolution::Cancelled`]
    /// immediately, without waiting for a terminal event -- for a
    /// caller that knows the whole interaction is being abandoned for a
    /// reason no single [`Pointer`] event captures (e.g. the owning
    /// widget is being removed from the tree mid-gesture).
    pub fn cancel(&mut self) -> Option<Resolution> {
        if self.resolution.is_some() {
            return None;
        }

        self.resolve(Resolution::Cancelled)
    }

    /// Re-evaluates every still-pending entrant at `now` via
    /// [`Recognizer::poll`], without a new [`Pointer`] event having
    /// arrived, and resolves the competition if this now allows it.
    ///
    /// This is what lets a deadline-based recognizer (a
    /// [`crate::long_press`] whose hold duration elapsed with the
    /// pointer still perfectly still) actually win: no `Pointer` event
    /// happens at exactly the deadline on its own, so without a
    /// caller-driven `tick` the arena would wait forever for one. A
    /// caller (`runtime::pointer_router`, in this codebase) is expected
    /// to call this once per frame/timer tick for every unresolved
    /// arena, exactly as `runtime::frame_scheduler::Scheduler::tick`
    /// already does for frame production.
    ///
    /// A no-op (returns `None`, does not panic) on an already-resolved
    /// arena, unlike [`Arena::dispatch`]/[`Arena::register`] -- ticking
    /// a large set of arenas every frame should not require a caller to
    /// first filter out the ones that already resolved.
    pub fn tick(&mut self, now: Instant) -> Option<Resolution> {
        if self.resolution.is_some() {
            return None;
        }

        for entrant in &mut self.entrants {
            if entrant.disposition == Disposition::Pending {
                entrant.disposition = entrant.recognizer.poll(now);
            }
        }

        self.try_resolve()
    }

    fn try_resolve(&mut self) -> Option<Resolution> {
        if self.resolution.is_some() {
            return None;
        }

        // An empty arena is the runtime's passive/default routing mode. It
        // tracks the full pointer lifetime without inventing a winner, then
        // resolves cleanly once the final pointer leaves.
        if self.entrants.is_empty() {
            return self
                .tracked_pointers
                .is_empty()
                .then(|| self.resolve(Resolution::NoWinner))
                .flatten();
        }

        // Rule 1 + 2: collect every team with at least one accepting
        // member, then pick the highest-priority (then
        // earliest-registered) one.
        let accepted_team = self
            .entrants
            .iter()
            .enumerate()
            .filter(|(_, entrant)| entrant.disposition == Disposition::Accepted)
            .map(|(index, entrant)| (entrant.team, entrant.priority, index))
            .max_by(|a, b| a.1.cmp(&b.1).then(b.2.cmp(&a.2)))
            .map(|(team, ..)| team);

        if let Some(team) = accepted_team {
            let winner = self
                .entrants
                .iter()
                .enumerate()
                .find(|(_, entrant)| entrant.team == team)
                .map(|(index, _)| EntrantId(index))
                .expect("the accepted team must have at least one member");

            return self.resolve(Resolution::Won(winner));
        }

        // Rule 3: exactly one team remains with any pending member, and
        // every other entrant has rejected -- that team wins by
        // default.
        let pending_teams: rustc_hash::FxHashSet<Team> = self
            .entrants
            .iter()
            .filter(|entrant| entrant.disposition == Disposition::Pending)
            .map(|entrant| entrant.team)
            .collect();

        if pending_teams.len() == 1 && !self.entrants.is_empty() {
            let team = *pending_teams.iter().next().unwrap();

            let winner = self
                .entrants
                .iter()
                .enumerate()
                .find(|(_, entrant)| entrant.team == team)
                .map(|(index, _)| EntrantId(index))
                .unwrap();

            return self.resolve(Resolution::Won(winner));
        }

        // Rule 4: nothing pending and nothing accepted -- no winner.
        if pending_teams.is_empty() {
            return self.resolve(Resolution::NoWinner);
        }

        None
    }

    fn resolve(&mut self, resolution: Resolution) -> Option<Resolution> {
        self.resolution = Some(resolution);

        let winner_team = match resolution {
            Resolution::Won(EntrantId(index)) => Some(self.entrants[index].team),
            Resolution::NoWinner | Resolution::Cancelled => None,
        };

        for entrant in &mut self.entrants {
            if entrant.notified {
                continue;
            }

            let verdict = match resolution {
                Resolution::Cancelled => Verdict::Cancelled,
                Resolution::Won(_) if Some(entrant.team) == winner_team => Verdict::Won,
                Resolution::Won(_) | Resolution::NoWinner => Verdict::Lost,
            };

            entrant.recognizer.on_verdict(verdict);
            entrant.notified = true;
        }

        Some(resolution)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aura_core::Point;
    use aura_core::pointer::{DeviceKind, Metadata, Phase};
    use aura_core::time::{Duration, Instant};

    fn epoch() -> Instant {
        static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        *EPOCH.get_or_init(Instant::now)
    }

    fn t(millis: u64) -> Instant {
        epoch() + Duration::from_millis(millis)
    }

    #[test]
    fn empty_arena_passively_tracks_until_pointer_up() {
        let mut arena = Arena::new();

        assert_eq!(arena.dispatch(pointer(Phase::Down, Point::ORIGIN, 0)), None);
        assert_eq!(
            arena.dispatch(pointer(Phase::Move, Point::new(1.0, 0.0), 1)),
            None
        );
        assert_eq!(
            arena.dispatch(pointer(Phase::Up, Point::new(1.0, 0.0), 2)),
            Some(Resolution::NoWinner)
        );
    }

    fn pointer(phase: Phase, position: Point, millis: u64) -> Pointer {
        Pointer {
            id: PointerId::MOUSE,
            device: DeviceKind::Mouse,
            phase,
            position,
            timestamp: t(millis),
            metadata: Metadata::NONE,
            captured: false,
        }
    }

    /// A scripted recognizer for tests: returns a fixed sequence of
    /// [`Disposition`]s, one per call to `on_event`, and records every
    /// [`Verdict`] it receives.
    struct Scripted {
        dispositions: std::collections::VecDeque<Disposition>,
        verdicts: std::rc::Rc<std::cell::RefCell<Vec<Verdict>>>,
    }

    impl Scripted {
        fn new(
            dispositions: impl IntoIterator<Item = Disposition>,
        ) -> (Self, std::rc::Rc<std::cell::RefCell<Vec<Verdict>>>) {
            let verdicts = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));

            (
                Self {
                    dispositions: dispositions.into_iter().collect(),
                    verdicts: verdicts.clone(),
                },
                verdicts,
            )
        }
    }

    impl Recognizer for Scripted {
        fn name(&self) -> &'static str {
            "scripted"
        }

        fn add_pointer(&mut self, _pointer: Pointer) {}

        fn remove_pointer(&mut self, _id: PointerId) {}

        fn on_event(&mut self, _pointer: Pointer) -> Disposition {
            self.dispositions
                .pop_front()
                .unwrap_or(Disposition::Pending)
        }

        fn on_verdict(&mut self, verdict: Verdict) {
            self.verdicts.borrow_mut().push(verdict);
        }

        fn reset(&mut self) {
            self.dispositions.clear();
        }
    }

    #[test]
    fn a_lone_entrant_wins_immediately_on_the_first_event() {
        let mut arena = Arena::new();
        let (scripted, verdicts) = Scripted::new([]);
        let id = arena.register(scripted, Team::solo(), Priority::NORMAL);

        let resolution = arena.dispatch(pointer(Phase::Down, Point::ORIGIN, 0));

        assert_eq!(resolution, Some(Resolution::Won(id)));
        assert_eq!(*verdicts.borrow(), vec![Verdict::Won]);
    }

    #[test]
    fn the_second_of_two_entrants_wins_once_the_first_rejects() {
        let mut arena = Arena::new();

        let (loser, loser_verdicts) = Scripted::new([Disposition::Rejected]);
        let (winner, winner_verdicts) = Scripted::new([Disposition::Pending]);

        let winner_id = arena.register(winner, Team::solo(), Priority::NORMAL);
        let _loser_id = arena.register(loser, Team::solo(), Priority::NORMAL);

        // Down does not call `on_event` (only registers the pointer),
        // so both are still pending after it.
        assert_eq!(arena.dispatch(pointer(Phase::Down, Point::ORIGIN, 0)), None);

        let resolution = arena.dispatch(pointer(Phase::Move, Point::new(1.0, 0.0), 1));

        assert_eq!(resolution, Some(Resolution::Won(winner_id)));
        assert_eq!(*winner_verdicts.borrow(), vec![Verdict::Won]);
        assert_eq!(*loser_verdicts.borrow(), vec![Verdict::Lost]);
    }

    #[test]
    fn every_entrant_rejecting_resolves_with_no_winner() {
        let mut arena = Arena::new();

        let (a, a_verdicts) = Scripted::new([Disposition::Rejected]);
        let (b, b_verdicts) = Scripted::new([Disposition::Rejected]);

        let _ = arena.register(a, Team::solo(), Priority::NORMAL);
        let _ = arena.register(b, Team::solo(), Priority::NORMAL);

        let _ = arena.dispatch(pointer(Phase::Down, Point::ORIGIN, 0));
        let resolution = arena.dispatch(pointer(Phase::Move, Point::new(1.0, 0.0), 1));

        assert_eq!(resolution, Some(Resolution::NoWinner));
        assert_eq!(*a_verdicts.borrow(), vec![Verdict::Lost]);
        assert_eq!(*b_verdicts.borrow(), vec![Verdict::Lost]);
    }

    #[test]
    fn a_cancel_event_resolves_as_cancelled_for_every_entrant() {
        let mut arena = Arena::new();

        let (a, a_verdicts) = Scripted::new([]);
        let (b, b_verdicts) = Scripted::new([]);

        let _ = arena.register(a, Team::solo(), Priority::NORMAL);
        let _ = arena.register(b, Team::solo(), Priority::NORMAL);

        let _ = arena.dispatch(pointer(Phase::Down, Point::ORIGIN, 0));
        let resolution = arena.dispatch(pointer(Phase::Cancel, Point::ORIGIN, 1));

        assert_eq!(resolution, Some(Resolution::Cancelled));
        assert_eq!(*a_verdicts.borrow(), vec![Verdict::Cancelled]);
        assert_eq!(*b_verdicts.borrow(), vec![Verdict::Cancelled]);
    }

    #[test]
    fn higher_priority_wins_a_same_pass_tie() {
        let mut arena = Arena::new();

        let (low, low_verdicts) = Scripted::new([Disposition::Accepted]);
        let (high, high_verdicts) = Scripted::new([Disposition::Accepted]);

        let _low_id = arena.register(low, Team::solo(), Priority::NORMAL);
        let high_id = arena.register(high, Team::solo(), Priority::SYSTEM);

        let _ = arena.dispatch(pointer(Phase::Down, Point::ORIGIN, 0));
        let resolution = arena.dispatch(pointer(Phase::Move, Point::new(1.0, 0.0), 1));

        assert_eq!(resolution, Some(Resolution::Won(high_id)));
        assert_eq!(*high_verdicts.borrow(), vec![Verdict::Won]);
        assert_eq!(*low_verdicts.borrow(), vec![Verdict::Lost]);
    }

    #[test]
    fn team_members_win_together_when_any_one_accepts() {
        let mut arena = Arena::new();
        let team = Team::new(42);

        let (a, a_verdicts) = Scripted::new([Disposition::Pending]);
        let (b, b_verdicts) = Scripted::new([Disposition::Accepted]);
        let (c, c_verdicts) = Scripted::new([Disposition::Rejected]);

        let _ = arena.register(a, team, Priority::NORMAL);
        let _ = arena.register(b, team, Priority::NORMAL);
        let _ = arena.register(c, Team::solo(), Priority::NORMAL);

        let _ = arena.dispatch(pointer(Phase::Down, Point::ORIGIN, 0));
        let resolution = arena.dispatch(pointer(Phase::Move, Point::new(1.0, 0.0), 1));

        assert!(matches!(resolution, Some(Resolution::Won(_))));
        assert_eq!(*a_verdicts.borrow(), vec![Verdict::Won]);
        assert_eq!(*b_verdicts.borrow(), vec![Verdict::Won]);
        assert_eq!(*c_verdicts.borrow(), vec![Verdict::Lost]);
    }

    #[test]
    fn losing_recognizers_receive_a_verdict_exactly_once() {
        let mut arena = Arena::new();

        let (loser, loser_verdicts) = Scripted::new([Disposition::Rejected]);
        let (winner, _winner_verdicts) = Scripted::new([Disposition::Pending]);

        let _ = arena.register(winner, Team::solo(), Priority::NORMAL);
        let _ = arena.register(loser, Team::solo(), Priority::NORMAL);

        let _ = arena.dispatch(pointer(Phase::Down, Point::ORIGIN, 0));
        let _ = arena.dispatch(pointer(Phase::Move, Point::new(1.0, 0.0), 1));

        assert_eq!(loser_verdicts.borrow().len(), 1);
    }

    #[test]
    #[should_panic(expected = "already-resolved")]
    fn dispatching_into_a_resolved_arena_panics() {
        let mut arena = Arena::new();
        let (scripted, _) = Scripted::new([]);
        let _ = arena.register(scripted, Team::solo(), Priority::NORMAL);

        let _ = arena.dispatch(pointer(Phase::Down, Point::ORIGIN, 0));
        assert!(arena.is_resolved());

        let _ = arena.dispatch(pointer(Phase::Up, Point::ORIGIN, 1));
    }

    #[test]
    #[should_panic(expected = "already-resolved")]
    fn registering_into_a_resolved_arena_panics() {
        let mut arena = Arena::new();
        let (scripted, _) = Scripted::new([]);
        let _ = arena.register(scripted, Team::solo(), Priority::NORMAL);

        let _ = arena.dispatch(pointer(Phase::Down, Point::ORIGIN, 0));
        assert!(arena.is_resolved());

        let (late, _) = Scripted::new([]);
        let _ = arena.register(late, Team::solo(), Priority::NORMAL);
    }

    #[test]
    fn cancel_forces_resolution_without_a_terminal_event() {
        let mut arena = Arena::new();
        let (scripted, verdicts) = Scripted::new([]);
        let _ = arena.register(scripted, Team::solo(), Priority::NORMAL);

        let resolution = arena.cancel();

        assert_eq!(resolution, Some(Resolution::Cancelled));
        assert_eq!(*verdicts.borrow(), vec![Verdict::Cancelled]);
    }

    #[test]
    fn cancel_on_an_already_resolved_arena_is_a_harmless_no_op() {
        let mut arena = Arena::new();
        let (scripted, _) = Scripted::new([]);
        let _ = arena.register(scripted, Team::solo(), Priority::NORMAL);

        let _ = arena.dispatch(pointer(Phase::Down, Point::ORIGIN, 0));
        assert!(arena.is_resolved());

        assert_eq!(arena.cancel(), None);
    }

    #[test]
    fn solo_teams_never_collide() {
        let a = Team::solo();
        let b = Team::solo();

        assert_ne!(a, b);
    }

    /// A recognizer that only ever accepts via `poll` once `now` reaches
    /// a fixed deadline, ignoring every real `on_event` call -- a
    /// minimal stand-in for `long_press`, for testing `Arena::tick`
    /// without depending on that module.
    struct DeadlineOnly {
        deadline: Instant,
        verdicts: std::rc::Rc<std::cell::RefCell<Vec<Verdict>>>,
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

        fn on_verdict(&mut self, verdict: Verdict) {
            self.verdicts.borrow_mut().push(verdict);
        }

        fn reset(&mut self) {}
    }

    #[test]
    fn tick_resolves_a_deadline_based_recognizer_with_no_new_event() {
        let mut arena = Arena::new();
        let verdicts = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));

        // A second, indefinitely-pending competitor keeps the arena from
        // resolving via "last one standing" the instant the deadline
        // recognizer is registered, so `Arena::tick` is what is actually
        // under test here.
        let (other, other_verdicts) = Scripted::new([]);
        let _other_id = arena.register(other, Team::solo(), Priority::NORMAL);

        let id = arena.register(
            DeadlineOnly {
                deadline: t(500),
                verdicts: verdicts.clone(),
            },
            Team::solo(),
            Priority::NORMAL,
        );

        let _ = arena.dispatch(pointer(Phase::Down, Point::ORIGIN, 0));

        assert_eq!(arena.tick(t(100)), None, "not yet at the deadline");
        assert_eq!(arena.tick(t(500)), Some(Resolution::Won(id)));
        assert_eq!(*verdicts.borrow(), vec![Verdict::Won]);
        assert_eq!(*other_verdicts.borrow(), vec![Verdict::Lost]);
    }

    #[test]
    fn tick_on_an_already_resolved_arena_is_a_harmless_no_op() {
        let mut arena = Arena::new();
        let (scripted, _) = Scripted::new([]);
        let _ = arena.register(scripted, Team::solo(), Priority::NORMAL);

        let _ = arena.dispatch(pointer(Phase::Down, Point::ORIGIN, 0));
        assert!(arena.is_resolved());

        assert_eq!(arena.tick(t(1000)), None);
    }
}

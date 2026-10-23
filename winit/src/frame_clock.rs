//! A single, coalesced frame-production state machine.
//!
//! [`Scheduler`] is the "exactly one monotonic framework clock owns
//! animation time" and "coalesce deadlines; widgets never create
//! independent render loops" rules from `phase.md`'s Phase 6, made
//! concrete: every redraw request a widget produces (via
//! a view's paint, layout or animation request) and every
//! occlusion/resume/submission/feedback event a window backend observes
//! funnels through one [`Scheduler`] per window, which is the only thing
//! that decides whether a frame is actually produced right now.
//!
//! [`Scheduler`] takes every timestamp as an explicit `Instant`
//! parameter rather than reading a clock itself, so its entire state
//! machine is deterministically testable with a fake clock
//! (`tests/runtime/frame_scheduler.rs`) -- the same testability pattern
//! the rest of the frame pipeline already uses.
//!
//! # States
//!
//! ```text
//!                    request_redraw_at(deadline)
//!         +------------------------------------------+
//!         |                                          v
//!    +---------+  request_animation()          +-----------+
//!    |  Idle   |------------------------------->| Animating |
//!    +---------+                                +-----------+
//!         ^  ^                                        |
//!         |  |         request_redraw_at(deadline)     |
//!         |  +----------------+  +--------------------+
//!         |                   |  |
//!         |              +-----------+
//!         |              | Requested |
//!         |              +-----------+
//!         |                    |
//!         |     tick(now >= deadline): should_produce_frame() -> true
//!         |                    v
//!         |              +-----------+   feedback_received /
//!         +--------------| Submitted |   feedback_timed_out
//!    frame_completed()   +-----------+
//!                              ^
//!                              |
//!         suspend() from any state ----> +-----------+  resume()
//!                                        | Suspended |------> Idle
//!                                        +-----------+
//! ```
//!
//! [`State::Suspended`] pre-empts every other state and every other
//! state's deadline: while suspended, [`Scheduler::should_produce_frame`]
//! always returns `false` regardless of what was pending before
//! [`Scheduler::suspend`] was called, satisfying "stop producing frames
//! for fully occluded/suspended surfaces." The pending request itself is
//! not discarded -- [`Scheduler::resume`] restores whatever deadline (or
//! animating state) was in effect, so a redraw asked for while occluded
//! still happens once the surface becomes visible again, rather than
//! being silently dropped.

use crate::core::time::{Duration, Instant};

use rustc_hash::FxHashSet;

/// Stable identifier for a registered motion source.
///
/// A runtime assigns these per window. Registration is deliberately explicit
/// so the scheduler can stop immediately after the final motion completes,
/// rather than keeping an inferred perpetual animation loop alive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MotionId(pub u64);

/// How long [`Scheduler`] waits for presentation feedback
/// ([`Scheduler::feedback_received`]) after a frame is
/// [`Scheduler::submit`]ted before giving up and returning to normal
/// scheduling anyway.
///
/// "Handle missing/late feedback without spinning": a backend that never
/// calls back (an unsupported platform, a compositor that silently drops
/// the callback) must not permanently wedge the scheduler in
/// [`State::Submitted`], since that would mean no further frame is ever
/// ready to be produced again.
pub const FEEDBACK_TIMEOUT: Duration = Duration::from_millis(200);

/// A single window's frame-production state machine.
#[derive(Debug, Clone, PartialEq)]
pub struct Scheduler {
    state: State,
    active_motions: FxHashSet<MotionId>,
}

/// Work queued while a submitted frame waits for presentation feedback.
/// Kept inline in [`State`] so the steady animation path never allocates.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ResumeState {
    Idle,
    Requested { deadline: Instant },
    Animating,
}

/// A non-recursive snapshot retained while frame production is suspended.
/// Keeping this inline avoids a heap allocation on every minimize/occlusion
/// transition.
#[derive(Debug, Clone, Copy, PartialEq)]
enum SuspendedState {
    Idle,
    Requested {
        deadline: Instant,
    },
    Animating,
    Submitted {
        submitted_at: Instant,
        resume_to: ResumeState,
    },
}

impl SuspendedState {
    #[inline]
    fn restore(self) -> State {
        match self {
            Self::Idle => State::Idle,
            Self::Requested { deadline } => State::Requested { deadline },
            Self::Animating => State::Animating,
            Self::Submitted {
                submitted_at,
                resume_to,
            } => State::Submitted {
                submitted_at,
                resume_to,
            },
        }
    }
}

/// The current state of a [`Scheduler`].
#[derive(Debug, Clone, PartialEq)]
enum State {
    /// No frame is pending; nothing changed since the last completed
    /// frame.
    Idle,
    /// A frame has been requested for the given deadline (the frame
    /// should be produced once `now >= deadline`).
    Requested { deadline: Instant },
    /// A continuous redraw loop is in effect (e.g. an active animation
    /// subscription): a frame is produced every tick, with no fixed
    /// deadline to wait for.
    Animating,
    /// A frame's work has been submitted to the backend and the
    /// scheduler is waiting for presentation feedback (or
    /// [`FEEDBACK_TIMEOUT`]) before producing another.
    Submitted {
        submitted_at: Instant,
        resume_to: ResumeState,
    },
    /// The surface is fully occluded or the application is suspended;
    /// no frames are produced regardless of any pending request.
    /// Remembers what was pending so [`Scheduler::resume`] can restore
    /// it.
    Suspended { resume_to: SuspendedState },
}

impl Scheduler {
    /// Creates a new [`Scheduler`] in [`State::Idle`].
    pub fn new() -> Self {
        Self {
            state: State::Idle,
            active_motions: FxHashSet::default(),
        }
    }

    /// Requests a frame at the given `deadline`.
    ///
    /// Coalesces with any existing request: if a frame is already
    /// requested for an earlier deadline, that earlier deadline wins
    /// (matching `RedrawRequest`'s own
    /// `min`-based coalescing) -- multiple widgets asking for a redraw
    /// produces one frame at the earliest requested time, not one frame
    /// per request. Animating already implies another frame; a request made
    /// while submitted is retained for the first frame after feedback.
    pub fn request_redraw_at(&mut self, deadline: Instant) {
        match &mut self.state {
            State::Idle => self.state = State::Requested { deadline },
            State::Requested {
                deadline: existing, ..
            } => *existing = (*existing).min(deadline),
            State::Animating => {}
            State::Submitted { resume_to, .. } => {
                request_after_submit(resume_to, deadline);
            }
            State::Suspended { resume_to } => {
                request_while_suspended(resume_to, deadline);
            }
        }
    }

    /// Requests a continuous redraw loop (e.g. an active animation
    /// subscription is running).
    ///
    /// Once in [`State::Animating`], every [`Scheduler::tick`] produces
    /// a frame until [`Scheduler::stop_animating`] is called -- this is
    /// the single place a continuous redraw loop is allowed to exist
    /// ("widgets never create independent render loops": a widget
    /// signals *this* scheduler that it needs continuous frames, rather
    /// than running its own timer).
    pub fn request_animation(&mut self) {
        match &mut self.state {
            State::Idle | State::Requested { .. } => self.state = State::Animating,
            State::Animating => {}
            State::Submitted { resume_to, .. } => *resume_to = ResumeState::Animating,
            State::Suspended { resume_to } => request_animation_while_suspended(resume_to),
        }
    }

    /// Registers a currently active motion source.
    ///
    /// Registering the same source twice is idempotent. Motion sources still
    /// need to call this only through the runtime frame path; this method does
    /// not create a timer or a second render loop.
    pub fn register_motion(&mut self, motion: MotionId) {
        let _ = self.active_motions.insert(motion);
        self.request_animation();
    }

    /// Marks a motion source complete and stops scheduler-owned continuous
    /// frames once the final registered motion is gone.
    pub fn unregister_motion(&mut self, motion: MotionId) {
        let _ = self.active_motions.remove(&motion);
        if self.active_motions.is_empty() {
            self.stop_animating();
        }
    }

    /// Returns the bounded number of active registered motion sources.
    pub fn active_motion_count(&self) -> usize {
        self.active_motions.len()
    }

    /// Stops a continuous redraw loop previously started by
    /// [`Scheduler::request_animation`], returning to
    /// [`State::Idle`] (unless a frame is currently
    /// [`State::Submitted`], which is left alone until feedback
    /// arrives). Registered motion sources keep the loop active until
    /// each is explicitly unregistered.
    pub fn stop_animating(&mut self) {
        if !self.active_motions.is_empty() {
            return;
        }

        stop_animation_in(&mut self.state);
    }

    /// Suspends frame production: the surface is fully occluded or the
    /// application is suspended. [`Scheduler::should_produce_frame`]
    /// returns `false` unconditionally until [`Scheduler::resume`] is
    /// called, regardless of what was requested before or during
    /// suspension.
    ///
    /// A no-op if already suspended.
    pub fn suspend(&mut self) {
        if !matches!(self.state, State::Suspended { .. }) {
            let previous = std::mem::replace(&mut self.state, State::Idle);
            let resume_to = match previous {
                State::Idle => SuspendedState::Idle,
                State::Requested { deadline } => SuspendedState::Requested { deadline },
                State::Animating => SuspendedState::Animating,
                State::Submitted {
                    submitted_at,
                    resume_to,
                } => SuspendedState::Submitted {
                    submitted_at,
                    resume_to,
                },
                State::Suspended { .. } => unreachable!(),
            };

            self.state = State::Suspended { resume_to };
        }
    }

    /// Resumes frame production after [`Scheduler::suspend`], restoring
    /// whatever state was in effect before suspension (a pending
    /// deadline, an active animation, or idle).
    ///
    /// A no-op if not currently suspended.
    pub fn resume(&mut self) {
        let state = std::mem::replace(&mut self.state, State::Idle);

        if let State::Suspended { resume_to } = state {
            self.state = resume_to.restore();
        } else {
            self.state = state;
        }
    }

    /// Returns whether this [`Scheduler`] is currently suspended.
    pub fn is_suspended(&self) -> bool {
        matches!(self.state, State::Suspended { .. })
    }

    /// Marks a frame's work as submitted to the backend at `now`,
    /// transitioning to [`State::Submitted`]. The scheduler will not
    /// consider producing another frame (via
    /// [`Scheduler::should_produce_frame`]) until
    /// [`Scheduler::feedback_received`] or
    /// [`Scheduler::tick`] observes [`FEEDBACK_TIMEOUT`] has elapsed.
    ///
    /// A no-op if suspended (nothing should be submitted for a suspended
    /// surface in the first place; this guards against a caller that
    /// raced past a suspend event).
    pub fn submit(&mut self, now: Instant) {
        let resume_to = match self.state {
            State::Idle => ResumeState::Idle,
            State::Requested { deadline } => ResumeState::Requested { deadline },
            State::Animating => ResumeState::Animating,
            State::Submitted { .. } | State::Suspended { .. } => return,
        };

        self.state = State::Submitted {
            submitted_at: now,
            resume_to,
        };
    }

    /// Reports that presentation feedback for the most recently submitted
    /// frame has arrived. Work requested while the frame was being built is
    /// restored without waiting out [`FEEDBACK_TIMEOUT`].
    ///
    /// A no-op if not currently [`State::Submitted`] (e.g. feedback
    /// arriving after a timeout already recovered the scheduler, or a
    /// spurious/duplicate callback).
    pub fn feedback_received(&mut self) {
        if matches!(self.state, State::Submitted { .. }) {
            self.resume_after_submission();
        }
    }

    /// Advances the scheduler's notion of "now," recovering from a
    /// [`State::Submitted`] frame whose feedback never arrived within
    /// [`FEEDBACK_TIMEOUT`] ("handle missing/late feedback without
    /// spinning": this is what prevents a dropped feedback callback from
    /// wedging the scheduler forever).
    ///
    /// Call this once per event-loop iteration, before checking
    /// [`Scheduler::should_produce_frame`].
    pub fn tick(&mut self, now: Instant) {
        if let State::Submitted { submitted_at, .. } = self.state
            && now.saturating_duration_since(submitted_at) >= FEEDBACK_TIMEOUT
        {
            self.resume_after_submission();
        }
    }

    /// Returns whether a frame should be produced right now, given the
    /// current state and `now`.
    ///
    /// `false` while [`State::Suspended`], while
    /// [`State::Submitted`] (until feedback or timeout, via
    /// [`Scheduler::tick`]), or while [`State::Requested`] with a
    /// deadline still in the future. `true` for [`State::Animating`]
    /// (every tick produces a frame) or [`State::Requested`] whose
    /// deadline has passed.
    pub fn should_produce_frame(&self, now: Instant) -> bool {
        match self.state {
            State::Idle | State::Submitted { .. } | State::Suspended { .. } => false,
            State::Animating => true,
            State::Requested { deadline } => now >= deadline,
        }
    }

    /// Claims the frame that is ready at `now`.
    ///
    /// A due one-shot deadline is consumed before UI work starts, so redraws
    /// requested by that UI work remain queued for the following frame instead
    /// of being confused with the request that triggered the current one.
    pub fn begin_frame(&mut self, now: Instant) -> bool {
        match self.state {
            State::Requested { deadline } if now >= deadline => {
                self.state = State::Idle;
                true
            }
            State::Animating => true,
            State::Idle
            | State::Requested { .. }
            | State::Submitted { .. }
            | State::Suspended { .. } => false,
        }
    }

    /// Returns whether no visible frame work is pending.
    pub fn is_idle(&self) -> bool {
        matches!(self.state, State::Idle | State::Suspended { .. })
    }

    /// Returns the deadline a caller's event loop should wait until
    /// before checking [`Scheduler::should_produce_frame`] again (e.g.
    /// to compute a `ControlFlow::WaitUntil` target), or `None` if no
    /// deadline applies ([`State::Idle`] with nothing pending, or
    /// [`State::Animating`], which wants to run as soon as possible on
    /// every tick rather than waiting for a specific instant).
    ///
    /// [`State::Submitted`]'s deadline is when its
    /// [`FEEDBACK_TIMEOUT`] will elapse, so a caller that only wakes up
    /// for scheduled deadlines still recovers a dropped feedback
    /// callback instead of waiting forever.
    pub fn deadline(&self) -> Option<Instant> {
        match self.state {
            State::Idle | State::Animating => None,
            State::Requested { deadline } => Some(deadline),
            State::Submitted { submitted_at, .. } => Some(submitted_at + FEEDBACK_TIMEOUT),
            State::Suspended { .. } => None,
        }
    }

    /// Restores work queued while the submitted frame was being built.
    fn resume_after_submission(&mut self) {
        let State::Submitted { resume_to, .. } = self.state else {
            return;
        };

        self.state = if !self.active_motions.is_empty() {
            State::Animating
        } else {
            match resume_to {
                ResumeState::Idle => State::Idle,
                ResumeState::Requested { deadline } => State::Requested { deadline },
                ResumeState::Animating => State::Animating,
            }
        };
    }
}

#[inline]
fn request_after_submit(resume_to: &mut ResumeState, deadline: Instant) {
    match resume_to {
        ResumeState::Idle => *resume_to = ResumeState::Requested { deadline },
        ResumeState::Requested { deadline: existing } => *existing = (*existing).min(deadline),
        ResumeState::Animating => {}
    }
}

#[inline]
fn request_while_suspended(state: &mut SuspendedState, deadline: Instant) {
    match state {
        SuspendedState::Idle => *state = SuspendedState::Requested { deadline },
        SuspendedState::Requested { deadline: existing } => *existing = (*existing).min(deadline),
        SuspendedState::Animating => {}
        SuspendedState::Submitted { resume_to, .. } => {
            request_after_submit(resume_to, deadline);
        }
    }
}

#[inline]
fn request_animation_while_suspended(state: &mut SuspendedState) {
    match state {
        SuspendedState::Idle | SuspendedState::Requested { .. } => {
            *state = SuspendedState::Animating;
        }
        SuspendedState::Animating => {}
        SuspendedState::Submitted { resume_to, .. } => *resume_to = ResumeState::Animating,
    }
}

fn stop_animation_in(state: &mut State) {
    match state {
        State::Animating => *state = State::Idle,
        State::Submitted { resume_to, .. } if matches!(*resume_to, ResumeState::Animating) => {
            *resume_to = ResumeState::Idle;
        }
        State::Suspended { resume_to } => stop_animation_while_suspended(resume_to),
        State::Idle | State::Requested { .. } | State::Submitted { .. } => {}
    }
}

#[inline]
fn stop_animation_while_suspended(state: &mut SuspendedState) {
    match state {
        SuspendedState::Animating => *state = SuspendedState::Idle,
        SuspendedState::Submitted { resume_to, .. }
            if matches!(*resume_to, ResumeState::Animating) =>
        {
            *resume_to = ResumeState::Idle;
        }
        SuspendedState::Idle
        | SuspendedState::Requested { .. }
        | SuspendedState::Submitted { .. } => {}
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(millis: u64) -> Instant {
        // A single epoch shared by every call within a test (not a
        // fresh `Instant::now()` per call, which would drift by a few
        // hundred nanoseconds between calls and break exact-equality
        // assertions like `two_requests_coalesce_to_the_earliest_deadline`'s).
        static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

        *EPOCH.get_or_init(Instant::now) + Duration::from_millis(millis)
    }

    #[test]
    fn a_fresh_scheduler_is_idle_and_produces_no_frame() {
        let scheduler = Scheduler::new();

        assert!(!scheduler.should_produce_frame(t(0)));
        assert_eq!(scheduler.deadline(), None);
    }

    #[test]
    fn a_request_produces_a_frame_only_once_its_deadline_passes() {
        let mut scheduler = Scheduler::new();
        scheduler.request_redraw_at(t(10));

        assert!(!scheduler.should_produce_frame(t(5)));
        assert!(scheduler.should_produce_frame(t(10)));
        assert!(scheduler.should_produce_frame(t(20)));
    }

    #[test]
    fn two_requests_coalesce_to_the_earliest_deadline() {
        let mut scheduler = Scheduler::new();
        scheduler.request_redraw_at(t(20));
        scheduler.request_redraw_at(t(10));

        assert_eq!(scheduler.deadline(), Some(t(10)));

        // A later request must not push the deadline back out, either.
        scheduler.request_redraw_at(t(30));
        assert_eq!(scheduler.deadline(), Some(t(10)));
    }

    #[test]
    fn animating_produces_a_frame_at_any_time_with_no_fixed_deadline() {
        let mut scheduler = Scheduler::new();
        scheduler.request_animation();

        assert!(scheduler.should_produce_frame(t(0)));
        assert_eq!(scheduler.deadline(), None);
    }

    #[test]
    fn stop_animating_returns_to_idle() {
        let mut scheduler = Scheduler::new();
        scheduler.request_animation();
        scheduler.stop_animating();

        assert!(!scheduler.should_produce_frame(t(100)));
    }

    #[test]
    fn requesting_animation_while_a_deadline_is_pending_upgrades_to_animating() {
        let mut scheduler = Scheduler::new();
        scheduler.request_redraw_at(t(1000));
        scheduler.request_animation();

        // Animating means "produce every tick," which must win over a
        // future one-shot deadline that hasn't arrived yet.
        assert!(scheduler.should_produce_frame(t(0)));
    }

    #[test]
    fn submitting_a_frame_blocks_then_restores_continuous_animation() {
        let mut scheduler = Scheduler::new();
        scheduler.request_animation();
        assert!(scheduler.begin_frame(t(0)));
        scheduler.submit(t(0));

        assert!(!scheduler.should_produce_frame(t(1)));

        scheduler.feedback_received();

        assert!(scheduler.should_produce_frame(t(2)));

        scheduler.stop_animating();
        assert!(!scheduler.should_produce_frame(t(3)));
    }

    #[test]
    fn a_redraw_requested_during_a_frame_survives_feedback() {
        let mut scheduler = Scheduler::new();
        scheduler.request_redraw_at(t(10));

        assert!(scheduler.begin_frame(t(10)));
        scheduler.request_redraw_at(t(20));
        scheduler.submit(t(10));
        scheduler.feedback_received();

        assert!(!scheduler.should_produce_frame(t(19)));
        assert!(scheduler.should_produce_frame(t(20)));
    }

    #[test]
    fn a_missing_feedback_callback_recovers_after_the_timeout_via_tick() {
        let mut scheduler = Scheduler::new();
        scheduler.submit(t(0));

        scheduler.tick(t(50));
        assert!(scheduler.deadline().is_some(), "still waiting on feedback");

        scheduler.tick(t(200));

        assert!(
            !matches!(scheduler.state, State::Submitted { .. }),
            "the scheduler must recover from a dropped feedback callback, not wedge forever"
        );
    }

    #[test]
    fn a_late_feedback_call_after_timeout_recovery_is_a_harmless_no_op() {
        let mut scheduler = Scheduler::new();
        scheduler.submit(t(0));
        scheduler.tick(t(200));

        // The backend's feedback callback finally fires, long after
        // `tick` already recovered the scheduler.
        scheduler.feedback_received();

        assert!(!scheduler.should_produce_frame(t(201)));
    }

    #[test]
    fn suspending_stops_frame_production_regardless_of_a_pending_request() {
        let mut scheduler = Scheduler::new();
        scheduler.request_redraw_at(t(0));
        scheduler.suspend();

        assert!(!scheduler.should_produce_frame(t(1000)));
        assert!(scheduler.is_suspended());
    }

    #[test]
    fn suspending_during_an_animation_stops_it_too() {
        let mut scheduler = Scheduler::new();
        scheduler.request_animation();
        scheduler.suspend();

        assert!(!scheduler.should_produce_frame(t(0)));
    }

    #[test]
    fn resuming_restores_the_pending_deadline_that_was_in_effect_before_suspend() {
        let mut scheduler = Scheduler::new();
        scheduler.request_redraw_at(t(10));
        scheduler.suspend();
        scheduler.resume();

        assert!(!scheduler.should_produce_frame(t(5)));
        assert!(scheduler.should_produce_frame(t(10)));
    }

    #[test]
    fn resuming_restores_an_animating_state() {
        let mut scheduler = Scheduler::new();
        scheduler.request_animation();
        scheduler.suspend();
        scheduler.resume();

        assert!(scheduler.should_produce_frame(t(0)));
    }

    #[test]
    fn a_request_while_suspended_is_remembered_and_applies_after_resume() {
        let mut scheduler = Scheduler::new();
        scheduler.suspend();
        scheduler.request_redraw_at(t(10));

        assert!(
            !scheduler.should_produce_frame(t(20)),
            "still suspended: the request must not leak through"
        );

        scheduler.resume();

        assert!(!scheduler.should_produce_frame(t(5)));
        assert!(scheduler.should_produce_frame(t(10)));
    }

    #[test]
    fn resuming_when_not_suspended_is_a_harmless_no_op() {
        let mut scheduler = Scheduler::new();
        scheduler.request_redraw_at(t(10));
        scheduler.resume();

        assert!(scheduler.should_produce_frame(t(10)));
    }

    #[test]
    fn suspending_twice_keeps_the_first_pre_suspend_state() {
        let mut scheduler = Scheduler::new();
        scheduler.request_redraw_at(t(10));
        scheduler.suspend();
        scheduler.suspend();
        scheduler.resume();

        assert!(scheduler.should_produce_frame(t(10)));
    }

    #[test]
    fn a_fresh_scheduler_default_matches_new() {
        assert_eq!(Scheduler::default(), Scheduler::new());
    }

    #[test]
    fn registered_motion_stops_the_scheduler_as_soon_as_the_last_one_completes() {
        let mut scheduler = Scheduler::new();
        scheduler.register_motion(MotionId(1));
        scheduler.register_motion(MotionId(2));

        assert!(scheduler.should_produce_frame(t(0)));
        assert_eq!(scheduler.active_motion_count(), 2);

        scheduler.unregister_motion(MotionId(1));
        assert!(scheduler.should_produce_frame(t(1)));

        scheduler.unregister_motion(MotionId(2));
        assert!(!scheduler.should_produce_frame(t(2)));
        assert_eq!(scheduler.active_motion_count(), 0);
    }

    #[test]
    fn registered_motion_survives_a_submit_feedback_boundary() {
        let mut scheduler = Scheduler::new();
        scheduler.register_motion(MotionId(1));
        scheduler.submit(t(0));
        scheduler.feedback_received();

        assert!(scheduler.should_produce_frame(t(1)));
    }
}

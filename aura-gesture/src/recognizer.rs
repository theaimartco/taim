//! The contract every gesture recognizer implements, and the shared
//! vocabulary [`arena::Arena`](crate::arena::Arena) uses to resolve
//! competitions between them.
use aura_core::pointer::Pointer;
use aura_core::time::Instant;

/// What a [`Recognizer`] wants to happen to its place in the arena after
/// observing one [`Pointer`] event.
///
/// A fresh [`Recognizer`] always starts in a state the arena treats as
/// [`Disposition::Pending`] (competing, undecided); every subsequent
/// event moves it forward through this small state machine, which never
/// goes backward -- once a recognizer reports [`Disposition::Accepted`]
/// or [`Disposition::Rejected`], the arena never asks it for a
/// [`Disposition`] again for that contact (see
/// [`Recognizer::reset`] for how it becomes eligible for a *new*
/// contact).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// Still gathering evidence; has not yet ruled itself out or
    /// claimed victory. The arena keeps this recognizer active and
    /// keeps routing events to it.
    Pending,

    /// This recognizer has definitively recognized its gesture and
    /// wants to win the arena, claiming (capturing) every pointer it
    /// has been tracking, provided no competing recognizer at a higher
    /// or equal priority also accepts in the same resolution pass (see
    /// [`crate::arena`]'s module documentation for the exact tie-break
    /// rule).
    Accepted,

    /// This recognizer has ruled itself out (the movement pattern
    /// stopped matching its own gesture) and concedes the arena to any
    /// other competitor. A rejected recognizer is not itself
    /// "cancelled" in the [`aura_core::pointer::Phase::Cancel`] sense
    /// (it was never accepted, so it never captured anything to cancel)
    /// -- rejection and cancellation are related but distinct outcomes;
    /// see [`Verdict`].
    Rejected,
}

/// The final outcome the arena reports back to a [`Recognizer`] once a
/// competition resolves, distinct from the [`Disposition`] a recognizer
/// itself reports *to* the arena.
///
/// This is the vocabulary behind `phase.md`'s "losing recognizers
/// receive cancellation exactly once": every recognizer that entered a
/// competition receives exactly one [`Verdict`] once it resolves, no
/// matter how many [`Pointer`] events it was fed along the way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// This recognizer won the arena: its gesture is now active and it
    /// has captured every pointer it was tracking.
    Won,

    /// This recognizer lost the arena -- either because it reported
    /// [`Disposition::Rejected`] itself, or because a competitor
    /// reported [`Disposition::Accepted`] first (or at equal priority,
    /// per the arena's tie-break rule) and claimed the contact instead.
    /// A recognizer that had not yet reported anything when the arena
    /// resolved also receives this.
    Lost,

    /// The underlying pointer sequence was cancelled by the platform
    /// (e.g. the window lost focus, the OS reassigned the touch)
    /// before the arena could resolve at all. Distinct from [`Lost`]
    /// (a `Won`/`Lost` resolution) because nothing else won either --
    /// there is no gesture to hand off to.
    ///
    /// [`Lost`]: Verdict::Lost
    Cancelled,
}

/// A gesture recognizer: something that watches a bounded set of
/// [`Pointer`] contacts and decides, incrementally, whether they add up
/// to its own gesture.
///
/// # State invariants
///
/// - A [`Recognizer`] only ever tracks contacts the arena explicitly
///   assigned to it via [`Recognizer::add_pointer`]; it must not invent
///   pointers of its own.
/// - [`Recognizer::on_event`] is called once per relevant [`Pointer`]
///   event, in the order those events actually occurred (timestamps
///   monotonically non-decreasing), and must return a [`Disposition`]
///   reflecting only the events it has been shown so far -- never
///   events from the future.
/// - Once [`Recognizer::on_event`] has returned
///   [`Disposition::Accepted`] or [`Disposition::Rejected`] for a given
///   competition, the arena stops calling it until
///   [`Recognizer::reset`], which must fully clear any tracked state so
///   the same [`Recognizer`] instance can be reused for an unrelated,
///   later contact without leaking state between them.
pub trait Recognizer {
    /// A short, human-readable name for this recognizer, used in
    /// diagnostics and deterministic tie-breaking when two recognizers
    /// would otherwise resolve identically (see [`crate::arena`]).
    fn name(&self) -> &'static str;

    /// Registers a new pointer contact with this recognizer, before any
    /// [`Recognizer::on_event`] call for it. Recognizers that only
    /// support a fixed number of simultaneous pointers (e.g. [`tap`]
    /// supporting exactly one) may choose to immediately report
    /// [`Disposition::Rejected`] on the next [`Recognizer::on_event`]
    /// call if handed more contacts than they can track.
    ///
    /// [`tap`]: crate::tap
    fn add_pointer(&mut self, pointer: Pointer);

    /// Removes a pointer contact this recognizer was tracking, e.g.
    /// because it reported [`Pointer::is_terminal`] and its lifetime
    /// has ended. Never called for a pointer that was not previously
    /// passed to [`Recognizer::add_pointer`].
    fn remove_pointer(&mut self, id: aura_core::pointer::Id);

    /// Processes one [`Pointer`] event for a contact this recognizer is
    /// tracking, returning its updated [`Disposition`].
    fn on_event(&mut self, pointer: Pointer) -> Disposition;

    /// Re-evaluates this recognizer's [`Disposition`] at `now`, without
    /// a new [`Pointer`] event having arrived.
    ///
    /// Most recognizers only change their mind in response to actual
    /// pointer motion and never need this (the default implementation
    /// returns [`Disposition::Pending`] unconditionally, a no-op from
    /// the arena's point of view). A deadline-based recognizer (most
    /// notably [`crate::long_press`], which must accept once its hold
    /// duration elapses even if the pointer never moves again) overrides
    /// this so [`crate::arena::Arena::tick`] can still resolve the
    /// competition without waiting for an event that may never come.
    fn poll(&mut self, now: Instant) -> Disposition {
        let _ = now;
        Disposition::Pending
    }

    /// Notifies this recognizer of the arena's final [`Verdict`] for
    /// the competition it was part of.
    ///
    /// The default implementation does nothing; recognizers that need
    /// to reset internal timers/counters on loss (most of them, since
    /// they should not carry state into their next, unrelated
    /// competition) do so in [`Recognizer::reset`] instead, which is
    /// always called after this regardless of the `verdict`.
    fn on_verdict(&mut self, verdict: Verdict) {
        let _ = verdict;
    }

    /// Clears all tracked pointers and internal state, so this
    /// [`Recognizer`] instance can be reused for a new, unrelated
    /// competition.
    fn reset(&mut self);
}

/// A monotonic deadline check shared by every recognizer that cares
/// about elapsed time (e.g. [`crate::long_press`]'s hold duration,
/// [`crate::tap`]'s multi-tap window).
///
/// Kept as a free function (not a method on some shared base type,
/// since [`Recognizer`] implementors do not share a common struct) so
/// every recognizer computes "has `duration` elapsed since `start`"
/// identically, rather than each reimplementing its own
/// subtraction/comparison.
pub fn has_elapsed(start: Instant, now: Instant, duration: aura_core::time::Duration) -> bool {
    now.saturating_duration_since(start) >= duration
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_elapsed_is_false_before_the_duration() {
        let start = Instant::now();
        let duration = aura_core::time::Duration::from_millis(100);

        assert!(!has_elapsed(
            start,
            start + aura_core::time::Duration::from_millis(50),
            duration
        ));
    }

    #[test]
    fn has_elapsed_is_true_at_and_after_the_duration() {
        let start = Instant::now();
        let duration = aura_core::time::Duration::from_millis(100);

        assert!(has_elapsed(start, start + duration, duration));
        assert!(has_elapsed(
            start,
            start + aura_core::time::Duration::from_millis(150),
            duration
        ));
    }
}

//! Frame identity and timing context.
//!
//! [`Id`] and [`Context`] give every phase from here on a single, shared
//! way to refer to "which frame" and "when," instead of each phase
//! inventing its own frame counter or clock. Phase 6's frame scheduler
//! is the eventual owner of producing real [`Context`] values from a
//! live presentation loop; Phase 2 only defines the shape.

use crate::time::{Duration, Instant};

/// A monotonically increasing identifier for a frame.
///
/// Two [`Id`]s from the same running process can always be compared with
/// [`Id::wrapping_after`] even after the counter wraps around, which is
/// expected to happen only after billions of frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Id(u64);

impl Id {
    /// The first [`Id`], produced by the first frame of a run.
    pub const FIRST: Self = Self(0);

    /// Returns the [`Id`] of the frame after this one.
    ///
    /// Wraps around on overflow rather than panicking; frame counting
    /// must never be a source of a crash.
    pub const fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }

    /// Returns whether `self` comes strictly after `other`, correctly
    /// accounting for one wraparound of the underlying counter.
    ///
    /// This is the wrapping-safe comparison [`Id`] uses instead of
    /// [`Ord`]: a plain numeric comparison would falsely report the
    /// frame right after a wraparound as "before" the frame right before
    /// it. [`Id`] deliberately does not implement [`Ord`]/[`PartialOrd`]
    /// so callers cannot reach for the wrong comparison by mistake.
    pub const fn wrapping_after(self, other: Self) -> bool {
        self.0.wrapping_sub(other.0) as i64 > 0
    }
}

/// The timing and presentation context of a single frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Context {
    /// The [`Id`] of this frame.
    pub id: Id,
    /// The monotonic instant this frame's work began.
    pub started_at: Instant,
    /// The instant this frame is targeted to be presented at, if known.
    ///
    /// This is a target, not a guarantee: a real presentation loop
    /// (Phase 6) may miss it under load, and a headless context (e.g.
    /// `taim_bench`, Phase 1) may never populate it at all.
    pub target_present_at: Option<Instant>,
    /// The device scale factor (pixels per logical point) this frame is
    /// being produced for.
    pub scale_factor: f32,
    /// The rendering [`Quality`] tier this frame should target.
    pub quality: Quality,
}

impl Context {
    /// Creates the first [`Context`] of a run, with no known target
    /// present time, unit scale factor, and [`Quality::Ultra`].
    pub fn first(started_at: Instant) -> Self {
        Self {
            id: Id::FIRST,
            started_at,
            target_present_at: None,
            scale_factor: 1.0,
            quality: Quality::Ultra,
        }
    }

    /// Returns a new [`Context`] for the next frame, started at `now`,
    /// preserving this context's scale factor and quality tier.
    pub fn next(&self, now: Instant) -> Self {
        Self {
            id: self.id.next(),
            started_at: now,
            target_present_at: None,
            scale_factor: self.scale_factor,
            quality: self.quality,
        }
    }

    /// Returns how long this frame has been in progress as of `now`.
    ///
    /// Returns [`Duration::ZERO`] if `now` is before [`Context::started_at`]
    /// (e.g. due to clock coarsening), rather than panicking on
    /// subtraction underflow.
    pub fn elapsed(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.started_at)
    }

    /// Returns a copy of this [`Context`] with [`Context::target_present_at`]
    /// set to `started_at + refresh_interval` -- the predicted next
    /// vertical-blank instant, assuming presentation cadence matches
    /// `refresh_interval` exactly.
    ///
    /// This is a prediction, not a guarantee: Phase 6's frame scheduler
    /// (`runtime::frame_scheduler`) is the eventual owner of comparing
    /// this target against real [`Feedback`] and adjusting future
    /// predictions; [`Context`] only defines where the prediction is
    /// recorded.
    pub fn with_predicted_target(mut self, refresh_interval: Duration) -> Self {
        self.target_present_at = Some(self.started_at + refresh_interval);
        self
    }
}

/// A sample of real presentation feedback for one frame, as reported by
/// a windowing/compositor backend (when available) after it has actually
/// been shown on screen.
///
/// Not every backend or platform reports every field: a backend that has
/// no presentation-feedback API at all (or one that only reports some of
/// these) should leave the fields it cannot populate as `None`/`false`
/// (see [`Feedback::dropped`] for the "we don't even know if it
/// presented" case), rather than inventing a value -- an absent
/// measurement must stay visibly absent, not silently become a
/// misleading zero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Feedback {
    /// The [`Id`] of the frame this feedback is for.
    pub frame: Id,
    /// The instant the frame was actually presented, if the backend
    /// reports it.
    pub presented_at: Option<Instant>,
    /// How far `presented_at` was from [`Context::target_present_at`],
    /// if both are known. Positive means the frame presented later than
    /// predicted (missed its target); negative means earlier.
    pub target_error: Option<Duration>,
    /// Whether the backend reported this frame as dropped/skipped
    /// (presented late enough that the compositor showed a repeated
    /// previous frame instead of waiting for this one).
    pub dropped: bool,
}

impl Feedback {
    /// Creates a [`Feedback`] recording that `frame` presented at
    /// `presented_at`, computing [`Feedback::target_error`] against
    /// `target_present_at` if given.
    pub fn presented(frame: Id, presented_at: Instant, target_present_at: Option<Instant>) -> Self {
        let target_error = target_present_at.map(|target| {
            if presented_at >= target {
                presented_at.saturating_duration_since(target)
            } else {
                Duration::ZERO
            }
        });

        Self {
            frame,
            presented_at: Some(presented_at),
            target_error,
            dropped: false,
        }
    }

    /// Creates a [`Feedback`] recording that `frame` was dropped
    /// (presented late enough that the compositor showed a repeated
    /// previous frame instead).
    pub fn dropped(frame: Id) -> Self {
        Self {
            frame,
            presented_at: None,
            target_error: None,
            dropped: true,
        }
    }

    /// Returns whether this frame missed its predicted target (a
    /// nonzero, known [`Feedback::target_error`]), treating a
    /// [`Feedback::dropped`] frame as always missed.
    pub fn missed_target(&self) -> bool {
        self.dropped
            || self
                .target_error
                .is_some_and(|error| error > Duration::ZERO)
    }
}

/// The rendering quality tier a frame should target.
///
/// Phase 12's adaptive-quality controller (`runtime::quality_controller`)
/// is the eventual owner of transitioning between tiers under thermal,
/// power, or frame-pressure signals; Phase 2 only defines the tiers
/// themselves so [`Context`] has somewhere to record the current one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Quality {
    /// The minimal tier: no optional effects, prioritizing correctness
    /// and responsiveness above all else (e.g. after a GPU-loss recovery).
    Recovery,
    /// Reduced effects to preserve frame budget under sustained pressure.
    Efficient,
    /// The default tier for typical conditions.
    Balanced,
    /// Every optional effect enabled; used when there is budget to spare.
    Ultra,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_increment_and_wrap_safely() {
        let first = Id::FIRST;
        let second = first.next();

        assert!(second.wrapping_after(first));
        assert!(!first.wrapping_after(second));
    }

    #[test]
    fn wrapping_after_handles_counter_wraparound() {
        let near_max = Id(u64::MAX);
        let wrapped = near_max.next();

        assert_eq!(wrapped, Id(0));
        assert!(wrapped.wrapping_after(near_max));
    }

    #[test]
    fn quality_tiers_are_ordered_from_recovery_to_ultra() {
        assert!(Quality::Recovery < Quality::Efficient);
        assert!(Quality::Efficient < Quality::Balanced);
        assert!(Quality::Balanced < Quality::Ultra);
    }

    #[test]
    fn context_next_preserves_scale_and_quality() {
        let now = Instant::now();
        let first = Context::first(now);

        let mut scaled = first;
        scaled.scale_factor = 2.0;
        scaled.quality = Quality::Efficient;

        let next = scaled.next(now);

        assert_eq!(next.id, first.id.next());
        assert_eq!(next.scale_factor, 2.0);
        assert_eq!(next.quality, Quality::Efficient);
    }

    #[test]
    fn elapsed_never_underflows() {
        let now = Instant::now();
        let context = Context::first(now);

        // `now` here is not before `context.started_at`, but the method
        // must not panic even if a caller passes an earlier `now`
        // (e.g. due to a coarse or adjusted clock source).
        assert_eq!(context.elapsed(now), Duration::ZERO);
    }

    #[test]
    fn with_predicted_target_offsets_from_started_at() {
        let now = Instant::now();
        let context = Context::first(now).with_predicted_target(Duration::from_millis(16));

        assert_eq!(
            context.target_present_at,
            Some(now + Duration::from_millis(16))
        );
    }

    #[test]
    fn feedback_presented_on_target_has_zero_error() {
        let now = Instant::now();
        let feedback = Feedback::presented(Id::FIRST, now, Some(now));

        assert_eq!(feedback.target_error, Some(Duration::ZERO));
        assert!(!feedback.missed_target());
    }

    #[test]
    fn feedback_presented_late_reports_a_positive_error_and_counts_as_missed() {
        let target = Instant::now();
        let presented = target + Duration::from_millis(5);

        let feedback = Feedback::presented(Id::FIRST, presented, Some(target));

        assert_eq!(feedback.target_error, Some(Duration::from_millis(5)));
        assert!(feedback.missed_target());
    }

    #[test]
    fn feedback_presented_early_reports_zero_error_not_a_negative_duration() {
        let target = Instant::now() + Duration::from_millis(5);
        let presented = target - Duration::from_millis(2);

        let feedback = Feedback::presented(Id::FIRST, presented, Some(target));

        assert_eq!(feedback.target_error, Some(Duration::ZERO));
        assert!(!feedback.missed_target());
    }

    #[test]
    fn feedback_with_no_target_reports_no_error() {
        let feedback = Feedback::presented(Id::FIRST, Instant::now(), None);

        assert_eq!(feedback.target_error, None);
        assert!(!feedback.missed_target());
    }

    #[test]
    fn a_dropped_frame_always_counts_as_missed() {
        let feedback = Feedback::dropped(Id::FIRST);

        assert!(feedback.dropped);
        assert!(feedback.presented_at.is_none());
        assert!(feedback.missed_target());
    }
}

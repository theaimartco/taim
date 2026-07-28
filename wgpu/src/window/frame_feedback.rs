//! A backend-neutral adapter for real presentation feedback.
//!
//! `wgpu` (at least at the version this crate targets) exposes no
//! cross-platform, per-frame "was this frame presented on time" API --
//! only [`wgpu::Adapter::get_presentation_timestamp`], which correlates
//! the presentation engine's clock with the local one, not a report of
//! when any specific frame actually hit the screen. Real per-frame
//! feedback (Vulkan's `VK_GOOGLE_display_timing`, DXGI's
//! `IDXGISwapChain::GetFrameStatistics`, `CVDisplayLink` on macOS) is
//! backend- and platform-specific and not something this crate can
//! source uniformly today.
//!
//! [`FeedbackSource`] is the seam a real, platform-specific feedback
//! source is expected to plug into once one exists (a later phase or a
//! backend-specific follow-up), without requiring
//! `runtime::frame_scheduler::Scheduler` or any of its callers to change.
//! [`Synchronous`] is the only implementation this crate ships today: it
//! reports feedback immediately upon a successful, blocking
//! `present`/`get_current_texture` call returning, treating "the
//! synchronous present call returned" as the best available
//! approximation of "the frame was presented" -- correct in spirit
//! (nothing later claims a frame presented before its work was
//! actually submitted) but unable to detect a frame that was *accepted*
//! by the presentation engine yet still displayed late or dropped after
//! the call returned control to this process.

use crate::core::frame;
use crate::core::time::Instant;

/// A source of [`frame::Feedback`] for presented frames.
///
/// Implementations are not required to be exact -- see
/// [`Synchronous`]'s own documentation for the approximation this crate
/// ships today -- but must never report a frame as presented before it
/// was actually submitted, and must eventually report *something* (even
/// [`frame::Feedback::dropped`]) for every frame a caller reports via
/// [`FeedbackSource::frame_submitted`], so a caller relying on feedback
/// to drive `runtime::frame_scheduler::Scheduler::feedback_received`
/// never waits indefinitely (beyond
/// `runtime::frame_scheduler::FEEDBACK_TIMEOUT`, which the scheduler
/// itself already guards against regardless of what any
/// [`FeedbackSource`] does).
pub trait FeedbackSource {
    /// Records that `frame`'s work was submitted at `submitted_at`, so a
    /// later [`FeedbackSource::poll`] can report feedback for it.
    fn frame_submitted(&mut self, frame: frame::Id, submitted_at: Instant);

    /// Returns feedback for any frame(s) that have resolved since the
    /// last call, if any are available yet.
    ///
    /// Returns an empty [`Vec`] (not a blocking wait) if nothing has
    /// resolved yet; callers are expected to call this once per event-loop
    /// iteration, the same way they already poll
    /// `runtime::frame_scheduler::Scheduler::tick`.
    fn poll(&mut self, now: Instant) -> Vec<frame::Feedback>;
}

/// The default, synchronous [`FeedbackSource`]: reports a submitted
/// frame as presented as soon as [`Synchronous::present_completed`] is
/// called (i.e. immediately after the blocking `present` call that
/// submitted it returns), with no queueing or delay.
///
/// This is exactly as accurate as the blocking `present` call it wraps:
/// on a backend/platform where `present`/`get_current_texture` blocks
/// until the previous frame has actually left the presentation queue
/// (which FIFO present modes generally do), "the call returned" is a
/// reasonable proxy for "presented." It cannot distinguish a frame that
/// presented exactly on time from one that presented one vblank late,
/// since it has no independent clock to compare against beyond the
/// caller's own `now`.
#[derive(Debug, Default)]
pub struct Synchronous {
    pending: Vec<(frame::Id, Instant)>,
}

impl Synchronous {
    /// Creates a new [`Synchronous`] feedback source with nothing
    /// pending.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reports that the most recently submitted frame's blocking
    /// `present` call has returned successfully at `completed_at`,
    /// resolving it as presented (via [`frame::Feedback::presented`],
    /// with no known target to compare against -- see
    /// [`Synchronous`]'s own documentation for why no `target_error` is
    /// computed here).
    ///
    /// A caller with a target deadline in hand (e.g. from
    /// `frame::Context::target_present_at`) should prefer constructing
    /// its own [`frame::Feedback::presented`] directly with that target,
    /// rather than relying on this convenience, which cannot know it.
    pub fn present_completed(&mut self, completed_at: Instant) -> Vec<frame::Feedback> {
        self.pending
            .drain(..)
            .map(|(frame, _submitted_at)| frame::Feedback::presented(frame, completed_at, None))
            .collect()
    }

    /// Reports that the most recently submitted frame's `present` call
    /// failed or was skipped (e.g. the surface was occluded), so it
    /// should be reported as [`frame::Feedback::dropped`] rather than
    /// left to time out.
    pub fn present_failed(&mut self) -> Vec<frame::Feedback> {
        self.pending
            .drain(..)
            .map(|(frame, _submitted_at)| frame::Feedback::dropped(frame))
            .collect()
    }
}

impl FeedbackSource for Synchronous {
    fn frame_submitted(&mut self, frame: frame::Id, submitted_at: Instant) {
        self.pending.push((frame, submitted_at));
    }

    /// Always returns an empty [`Vec`]: [`Synchronous`] only resolves
    /// frames via [`Synchronous::present_completed`]/
    /// [`Synchronous::present_failed`], called explicitly at the point a
    /// blocking `present` call returns, not on a poll-driven schedule.
    fn poll(&mut self, _now: Instant) -> Vec<frame::Feedback> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_source_has_nothing_pending() {
        let mut source = Synchronous::new();

        assert!(source.present_completed(Instant::now()).is_empty());
    }

    #[test]
    fn present_completed_resolves_every_submitted_frame() {
        let mut source = Synchronous::new();
        let now = Instant::now();

        source.frame_submitted(frame::Id::FIRST, now);
        source.frame_submitted(frame::Id::FIRST.next(), now);

        let feedback = source.present_completed(now);

        assert_eq!(feedback.len(), 2);
        assert!(feedback.iter().all(|f| f.presented_at == Some(now)));
        assert!(feedback.iter().all(|f| !f.dropped));
    }

    #[test]
    fn present_completed_clears_pending_frames() {
        let mut source = Synchronous::new();
        let now = Instant::now();

        source.frame_submitted(frame::Id::FIRST, now);
        let _ = source.present_completed(now);

        assert!(source.present_completed(now).is_empty());
    }

    #[test]
    fn present_failed_reports_dropped_frames() {
        let mut source = Synchronous::new();
        let now = Instant::now();

        source.frame_submitted(frame::Id::FIRST, now);

        let feedback = source.present_failed();

        assert_eq!(feedback.len(), 1);
        assert!(feedback[0].dropped);
        assert!(feedback[0].missed_target());
    }

    #[test]
    fn poll_never_reports_anything_on_its_own() {
        let mut source = Synchronous::new();
        source.frame_submitted(frame::Id::FIRST, Instant::now());

        assert!(source.poll(Instant::now()).is_empty());
    }
}

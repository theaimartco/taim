//! TAIM frame presentation intent.
//!
//! A widget does not own the event loop: it declares whether a frame should
//! arrive immediately, at an earliest deadline, or not at all. Independent
//! requests may be merged without allocating or spawning a timer.

use crate::time::Instant;

/// Earliest requested work for a single window.
///
/// Variant ordering is retained for existing callers, but prefer
/// RedrawRequest::merge to express frame-scheduling intent explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RedrawRequest {
    /// Dispatch as soon as the platform permits the next frame.
    NextFrame,
    /// Dispatch no later than the supplied monotonic deadline.
    At(Instant),
    /// No work to present.
    Wait,
}

impl RedrawRequest {
    /// Combine independent widget requests into one window request.
    ///
    /// A next-frame request always wins over a delayed request. Among
    /// delayed requests, the earliest monotonic deadline wins. Wait is the
    /// identity, so streams with no visible work never schedule a frame.
    #[must_use]
    pub fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::NextFrame, _) | (_, Self::NextFrame) => Self::NextFrame,
            (Self::Wait, next) | (next, Self::Wait) => next,
            (Self::At(left), Self::At(right)) => Self::At(left.min(right)),
        }
    }

    /// Deadline the TAIM per-window scheduler should own.
    ///
    /// Returns None for Wait. The event loop samples the current time
    /// once for NextFrame so widgets cannot create competing clocks.
    #[must_use]
    pub fn deadline(self, now: Instant) -> Option<Instant> {
        match self {
            Self::NextFrame => Some(now),
            Self::At(time) => Some(time),
            Self::Wait => None,
        }
    }

    /// Whether the OS should be explicitly notified of imminent redraw.
    ///
    /// Delayed requests are scheduled by the framework's shared clock;
    /// waking the window immediately would waste work before the deadline.
    #[must_use]
    pub const fn wake_now(self) -> bool {
        matches!(self, Self::NextFrame)
    }
}

impl From<Instant> for RedrawRequest {
    fn from(time: Instant) -> Self {
        Self::At(time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::Duration;

    #[test]
    fn all_request_pairs_merge_to_the_earliest_frame() {
        let now = Instant::now();
        let late = now + Duration::from_millis(200);
        let earlier = now + Duration::from_millis(30);
        let requests = [
            RedrawRequest::Wait,
            RedrawRequest::NextFrame,
            RedrawRequest::At(earlier),
            RedrawRequest::At(late),
        ];

        for &left in &requests {
            assert_eq!(left.merge(RedrawRequest::Wait), left);
            assert_eq!(left.merge(left), left);
            for &right in &requests {
                assert_eq!(left.merge(right), right.merge(left));
                assert_eq!(left.merge(right), left.min(right));
                for &third in &requests {
                    assert_eq!(
                        left.merge(right).merge(third),
                        left.merge(right.merge(third))
                    );
                }
            }
        }
    }

    #[test]
    fn deadline_policy_does_not_wake_for_idle_or_future_work() {
        let now = Instant::now();
        let future = now + Duration::from_millis(20);

        assert_eq!(RedrawRequest::Wait.deadline(now), None);
        assert!(!RedrawRequest::Wait.wake_now());
        assert_eq!(RedrawRequest::At(future).deadline(now), Some(future));
        assert!(!RedrawRequest::At(future).wake_now());
        assert_eq!(RedrawRequest::NextFrame.deadline(now), Some(now));
        assert!(RedrawRequest::NextFrame.wake_now());
    }
}

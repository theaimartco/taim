//! Presentation feedback correlation.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::surface::SurfaceId;

/// A frame submitted to the compositor.
#[derive(Debug, Clone, Copy)]
pub struct Submission {
    /// Owning surface.
    pub surface: SurfaceId,
    /// Framework frame identifier.
    pub frame: u64,
    /// Monotonic submission timestamp.
    pub submitted_at: Instant,
    /// Predicted target presentation timestamp.
    pub target_present_at: Instant,
}

/// Resolved presentation timing.
#[derive(Debug, Clone, Copy)]
pub struct Feedback {
    /// Resolved submission.
    pub submission: Submission,
    /// Compositor presentation timestamp.
    pub presented_at: Instant,
    /// Reported display refresh interval.
    pub refresh: Duration,
    /// Whether the compositor discarded the frame.
    pub discarded: bool,
}

impl Feedback {
    /// Returns lateness relative to the predicted target.
    pub fn target_error(self) -> Duration {
        self.presented_at
            .saturating_duration_since(self.submission.target_present_at)
    }
}

/// Bounded in-flight presentation queue.
#[derive(Debug)]
pub struct FrameClock {
    pending: VecDeque<Submission>,
    capacity: usize,
}

impl FrameClock {
    /// Creates a bounded queue with at least one slot.
    pub fn new(capacity: usize) -> Self {
        Self {
            pending: VecDeque::with_capacity(capacity.min(8)),
            capacity: capacity.max(1),
        }
    }

    /// Tracks a submission, returning an evicted oldest entry.
    pub fn submit(&mut self, submission: Submission) -> Option<Submission> {
        let dropped = (self.pending.len() == self.capacity)
            .then(|| self.pending.pop_front())
            .flatten();
        self.pending.push_back(submission);
        dropped
    }

    /// Resolves matching compositor feedback.
    pub fn resolve(
        &mut self,
        surface: SurfaceId,
        frame: u64,
        presented_at: Instant,
        refresh: Duration,
        discarded: bool,
    ) -> Option<Feedback> {
        let index = self
            .pending
            .iter()
            .position(|item| item.surface == surface && item.frame == frame)?;
        let submission = self.pending.remove(index)?;
        Some(Feedback {
            submission,
            presented_at,
            refresh,
            discarded,
        })
    }

    /// Removes all frames belonging to a destroyed surface.
    pub fn discard_surface(&mut self, surface: SurfaceId) -> usize {
        let before = self.pending.len();
        self.pending.retain(|item| item.surface != surface);
        before - self.pending.len()
    }

    /// Returns the current in-flight count.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_is_bounded_and_drops_the_oldest() {
        let now = Instant::now();
        let id = SurfaceId::new(1).unwrap();
        let mut clock = FrameClock::new(2);
        let make = |frame| Submission {
            surface: id,
            frame,
            submitted_at: now,
            target_present_at: now,
        };

        assert!(clock.submit(make(1)).is_none());
        assert!(clock.submit(make(2)).is_none());
        assert_eq!(clock.submit(make(3)).unwrap().frame, 1);
        assert_eq!(clock.pending(), 2);
    }
}

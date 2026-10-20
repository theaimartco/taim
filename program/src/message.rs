//! Traits for the message type of a [`Program`](crate::Program).

/// A trait alias for [`Clone`], but only when the `time-travel`
/// feature is enabled.
#[cfg(feature = "time-travel")]
pub trait MaybeClone: Clone {}

#[cfg(feature = "time-travel")]
impl<T> MaybeClone for T where T: Clone {}

/// A trait alias for [`Clone`], but only when the `time-travel`
/// feature is enabled.
#[cfg(not(feature = "time-travel"))]
pub trait MaybeClone {}

#[cfg(not(feature = "time-travel"))]
impl<T> MaybeClone for T {}

/// A trait alias for [`Debug`](std::fmt::Debug), but only when the
/// `debug` feature is enabled.
#[cfg(feature = "debug")]
pub trait MaybeDebug: std::fmt::Debug {}

#[cfg(feature = "debug")]
impl<T> MaybeDebug for T where T: std::fmt::Debug {}

/// A trait alias for [`Debug`](std::fmt::Debug), but only when the
/// `debug` feature is enabled.
#[cfg(not(feature = "debug"))]
pub trait MaybeDebug {}

#[cfg(not(feature = "debug"))]
impl<T> MaybeDebug for T {}

/// Monotonic message identity for optional TAIM reactive tracing.
///
/// A sequence never silently wraps or reuses a previously issued identity;
/// callers can detect exhaustion rather than correlating unrelated events.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MessageSequence {
    next: u64,
}

impl MessageSequence {
    /// Create a new sequence, initially holding no issued identities.
    pub const fn new() -> Self {
        Self { next: 0 }
    }

    /// Allocate the next nonzero message ID or report exhaustion.
    pub fn issue(&mut self) -> Option<u64> {
        self.next = self.next.checked_add(1)?;
        Some(self.next)
    }

    /// Return the last successfully issued message ID.
    pub const fn last_issued(&self) -> u64 {
        self.next
    }
}

#[cfg(test)]
mod taim_message_sequence_tests {
    use super::*;

    #[test]
    fn emits_stable_nonzero_ids_without_wraparound() {
        let mut ids = MessageSequence::new();
        assert_eq!(ids.issue(), Some(1));
        assert_eq!(ids.issue(), Some(2));
        assert_eq!(ids.last_issued(), 2);
        ids.next = u64::MAX;
        assert_eq!(ids.issue(), None);
        assert_eq!(ids.last_issued(), u64::MAX);
    }
}

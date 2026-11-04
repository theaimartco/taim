//! Live-region announcements: queueing, coalescing, and privacy.
//!
//! A live region announces a change the user did not cause and is not
//! looking at -- a validation error, a "message sent" status, a battery
//! warning. That makes it the one part of the accessibility surface that
//! **speaks without being asked**, which is exactly why it is also the
//! easiest place to leak a secret.
//!
//! `phase.md` Phase 13's required behavior is blunt about it:
//!
//! > Live regions do not leak secure/private text.
//!
//! [`LiveRegions::announce`] is therefore the enforcement point, not a
//! convenience: it refuses to queue anything a
//! [`Role::SecureField`](taim_core::semantics::Role::SecureField) node
//! produced, and refuses anything explicitly marked
//! [`Announcement::private`]. There is no path around it, because the
//! queue is private and this is the only way in.

use crate::node::Entry;

use taim_core::semantics::{Live, Role};
use taim_core::widget::Id;

use smol_str::SmolStr;

/// How urgently an announcement interrupts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Urgency {
    /// Wait for a natural pause (a status message).
    Polite,
    /// Interrupt immediately (an error that demands attention).
    Assertive,
}

impl From<Live> for Urgency {
    fn from(live: Live) -> Self {
        match live {
            Live::Polite => Self::Polite,
            Live::Assertive => Self::Assertive,
        }
    }
}

/// One queued announcement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Announcement {
    /// The node that produced it, for coalescing and for an adapter that
    /// wants to point at the source.
    pub source: Id,
    /// The text to speak.
    pub message: SmolStr,
    /// How urgently it interrupts.
    pub urgency: Urgency,
}

/// Why an announcement was refused.
///
/// Refusals are reported rather than silently dropped so a caller can
/// tell "nothing to say" from "you tried to say something you must not",
/// and so the security tests can assert on the reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The source node is a secure field. Its content must never be
    /// spoken, regardless of what the caller passed.
    Secure,
    /// The caller marked this announcement private (e.g. a notification
    /// whose privacy level withholds its body on a locked screen).
    Private,
    /// The source node is not a live region at all, so it has no standing
    /// to announce anything.
    NotLive,
    /// The message was empty or whitespace.
    Empty,
}

/// The bounded announcement queue for one interface.
///
/// Bounded on purpose: a live region driven by a runaway update loop
/// would otherwise grow without limit, and a screen reader that is
/// thousands of announcements behind is useless anyway. When the queue is
/// full the **oldest polite** announcement is dropped first -- stale
/// status is the least valuable thing present.
#[derive(Debug, Clone)]
pub struct LiveRegions {
    queue: Vec<Announcement>,
    capacity: usize,
}

impl Default for LiveRegions {
    fn default() -> Self {
        Self::new(Self::DEFAULT_CAPACITY)
    }
}

impl LiveRegions {
    /// The default queue bound.
    pub const DEFAULT_CAPACITY: usize = 32;

    /// Creates a queue with the given hard bound (at least 1).
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: Vec::new(),
            capacity: capacity.max(1),
        }
    }

    /// Queues an announcement from `source`, or returns why it was
    /// refused.
    ///
    /// # Privacy
    ///
    /// A [`Role::SecureField`] source is refused unconditionally. This is
    /// deliberately checked against the *node's role* rather than trusting
    /// the caller to pass `private`: the whole point is that a caller who
    /// forgets cannot leak anything.
    pub fn announce(
        &mut self,
        source: &Entry,
        message: impl Into<SmolStr>,
        private: bool,
    ) -> Result<(), Refusal> {
        if source.role == Role::SecureField {
            return Err(Refusal::Secure);
        }

        if private {
            return Err(Refusal::Private);
        }

        let Some(live) = source.live else {
            return Err(Refusal::NotLive);
        };

        let message: SmolStr = message.into();

        if message.trim().is_empty() {
            return Err(Refusal::Empty);
        }

        let announcement = Announcement {
            source: source.id.clone(),
            message,
            urgency: live.into(),
        };

        self.push(announcement);

        Ok(())
    }

    /// Queues an announcement, coalescing against the same source.
    ///
    /// A live region that updates repeatedly (a progress message, a
    /// character counter) must not queue every intermediate value: by the
    /// time the reader reaches them they are all wrong but the last. So a
    /// pending announcement from the same source is *replaced*, not
    /// appended -- while its queue position is kept, so a region that
    /// keeps chattering cannot starve one that spoke earlier.
    fn push(&mut self, announcement: Announcement) {
        if let Some(existing) = self
            .queue
            .iter_mut()
            .find(|queued| queued.source == announcement.source)
        {
            *existing = announcement;
            return;
        }

        if self.queue.len() >= self.capacity {
            // Drop the oldest polite announcement; only if there is none
            // do we drop the oldest assertive one.
            let victim = self
                .queue
                .iter()
                .position(|queued| queued.urgency == Urgency::Polite)
                .unwrap_or(0);

            let _ = self.queue.remove(victim);
        }

        self.queue.push(announcement);
    }

    /// Takes every pending announcement, assertive ones first, clearing
    /// the queue.
    ///
    /// Within an urgency, order is preserved: an adapter must not reorder
    /// two status messages relative to each other, or the narration stops
    /// making sense.
    pub fn drain(&mut self) -> Vec<Announcement> {
        let mut pending = std::mem::take(&mut self.queue);

        // Keyed on urgency alone, descending, and *stable* -- which is
        // load-bearing, not incidental: two status messages must keep the
        // order they arrived in, or the narration stops making sense.
        pending.sort_by_key(|announcement| std::cmp::Reverse(announcement.urgency));

        pending
    }

    /// The number of pending announcements.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Whether nothing is pending.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// The hard queue bound.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::node::Order;
    use taim_core::semantics::{Actions, State};
    use taim_core::{Rectangle, Size};

    fn entry(role: Role, live: Option<Live>) -> Entry {
        Entry {
            id: Id::unique(),
            parent: None,
            order: Order::root(0),
            bounds: Rectangle::with_size(Size::new(10.0, 10.0)),
            role,
            state: State::empty(),
            name: None,
            description: None,
            value: None,
            actions: Actions::empty(),
            live,
            autofill: None,
        }
    }

    #[test]
    fn a_secure_field_can_never_announce() {
        let mut regions = LiveRegions::default();
        let secure = entry(Role::SecureField, Some(Live::Assertive));

        assert_eq!(
            regions.announce(&secure, "hunter2", false),
            Err(Refusal::Secure),
            "a secure field must be refused even when it claims to be a live region"
        );
        assert!(regions.is_empty());
    }

    #[test]
    fn a_private_announcement_is_refused() {
        let mut regions = LiveRegions::default();
        let node = entry(Role::Group, Some(Live::Polite));

        assert_eq!(
            regions.announce(&node, "Bank code 1234", true),
            Err(Refusal::Private)
        );
        assert!(regions.is_empty());
    }

    #[test]
    fn a_node_that_is_not_a_live_region_cannot_announce() {
        let mut regions = LiveRegions::default();
        let node = entry(Role::Button, None);

        assert_eq!(
            regions.announce(&node, "hello", false),
            Err(Refusal::NotLive)
        );
    }

    #[test]
    fn an_empty_message_is_refused() {
        let mut regions = LiveRegions::default();
        let node = entry(Role::Group, Some(Live::Polite));

        assert_eq!(regions.announce(&node, "   ", false), Err(Refusal::Empty));
    }

    #[test]
    fn an_ordinary_live_region_announces() {
        let mut regions = LiveRegions::default();
        let node = entry(Role::Group, Some(Live::Polite));

        assert_eq!(regions.announce(&node, "Saved", false), Ok(()));
        assert_eq!(regions.len(), 1);

        let drained = regions.drain();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].message, "Saved");
        assert_eq!(drained[0].urgency, Urgency::Polite);
        assert!(regions.is_empty(), "drain must clear the queue");
    }

    #[test]
    fn assertive_announcements_come_first() {
        let mut regions = LiveRegions::default();
        let status = entry(Role::Group, Some(Live::Polite));
        let error = entry(Role::Group, Some(Live::Assertive));

        assert_eq!(regions.announce(&status, "Saving", false), Ok(()));
        assert_eq!(regions.announce(&error, "Failed", false), Ok(()));

        let drained = regions.drain();

        assert_eq!(
            drained[0].message, "Failed",
            "an error must interrupt a status"
        );
        assert_eq!(drained[1].message, "Saving");
    }

    #[test]
    fn repeated_updates_from_one_source_coalesce_to_the_latest() {
        // A progress message must not queue every intermediate value: by
        // the time the reader reaches them they are all wrong but the last.
        let mut regions = LiveRegions::default();
        let progress = entry(Role::Group, Some(Live::Polite));

        for percent in [10, 20, 30, 90] {
            assert_eq!(
                regions.announce(&progress, format!("{percent}%"), false),
                Ok(())
            );
        }

        assert_eq!(regions.len(), 1, "one source, one pending announcement");

        let drained = regions.drain();
        assert_eq!(drained[0].message, "90%");
    }

    #[test]
    fn coalescing_keeps_the_original_queue_position() {
        // A chatty region must not jump the queue past one that spoke
        // earlier just by updating again.
        let mut regions = LiveRegions::default();
        let chatty = entry(Role::Group, Some(Live::Polite));
        let quiet = entry(Role::Group, Some(Live::Polite));

        assert_eq!(regions.announce(&chatty, "first", false), Ok(()));
        assert_eq!(regions.announce(&quiet, "second", false), Ok(()));
        assert_eq!(regions.announce(&chatty, "updated", false), Ok(()));

        let drained = regions.drain();

        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].message, "updated");
        assert_eq!(drained[1].message, "second");
    }

    #[test]
    fn the_queue_is_bounded_and_drops_stale_polite_first() {
        let mut regions = LiveRegions::new(2);

        let a = entry(Role::Group, Some(Live::Polite));
        let b = entry(Role::Group, Some(Live::Assertive));
        let c = entry(Role::Group, Some(Live::Polite));

        assert_eq!(regions.announce(&a, "old status", false), Ok(()));
        assert_eq!(regions.announce(&b, "error", false), Ok(()));
        assert_eq!(regions.announce(&c, "new status", false), Ok(()));

        assert_eq!(regions.len(), 2, "the queue must stay bounded");

        let drained = regions.drain();
        let messages: Vec<&str> = drained.iter().map(|a| a.message.as_str()).collect();

        assert!(
            messages.contains(&"error"),
            "an assertive error must outlive a stale status: {messages:?}"
        );
        assert!(!messages.contains(&"old status"));
    }

    #[test]
    fn a_runaway_loop_cannot_grow_the_queue_without_bound() {
        let mut regions = LiveRegions::new(8);

        for i in 0..10_000 {
            let node = entry(Role::Group, Some(Live::Polite));
            let _ = regions.announce(&node, format!("update {i}"), false);
            assert!(regions.len() <= regions.capacity());
        }

        assert!(regions.len() <= 8);
    }
}

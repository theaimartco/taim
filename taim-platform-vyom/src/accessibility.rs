//! Bounded platform accessibility transport.

use std::collections::VecDeque;

use taim_accessibility::adapter::{Adapter, Capabilities};
use taim_accessibility::{Announcement, Change};
use taim_core::widget::Id;

/// Message delivered to the compositor accessibility service.
#[derive(Debug, Clone)]
pub enum Message {
    /// Incremental semantic-tree changes.
    Update(Vec<Change>),
    /// Privacy-checked live-region announcements.
    Announce(Vec<Announcement>),
    /// Current semantic focus.
    Focus(Option<Id>),
}

/// Adapter used by the VYOM transport. Queue growth is bounded; when the
/// consumer stalls, the oldest incremental message is replaced by newer
/// state instead of growing memory without limit.
#[derive(Debug)]
pub struct Transport {
    queue: VecDeque<Message>,
    capacity: usize,
    dropped: u64,
}

impl Transport {
    /// Creates a transport with a non-zero bounded queue.
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: VecDeque::with_capacity(capacity.min(8)),
            capacity: capacity.max(1),
            dropped: 0,
        }
    }

    fn push(&mut self, message: Message) {
        if self.queue.len() == self.capacity {
            let _ = self.queue.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
        self.queue.push_back(message);
    }

    /// Removes the oldest pending platform message.
    pub fn pop(&mut self) -> Option<Message> {
        self.queue.pop_front()
    }

    /// Returns messages dropped under backpressure.
    pub const fn dropped(&self) -> u64 {
        self.dropped
    }
}

impl Adapter for Transport {
    fn capabilities(&self) -> Capabilities {
        Capabilities::ALL
    }

    fn update(&mut self, changes: &[Change]) {
        if !changes.is_empty() {
            self.push(Message::Update(changes.to_vec()));
        }
    }

    fn announce(&mut self, announcements: &[Announcement]) {
        if !announcements.is_empty() {
            self.push(Message::Announce(announcements.to_vec()));
        }
    }

    fn focus_changed(&mut self, focused: Option<&Id>) {
        self.push(Message::Focus(focused.cloned()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stalled_consumer_never_grows_the_queue() {
        let mut transport = Transport::new(2);
        for _ in 0..10 {
            Adapter::focus_changed(&mut transport, None);
        }
        assert_eq!(transport.queue.len(), 2);
        assert_eq!(transport.dropped(), 8);
    }
}

//! The [`Adapter`] half of the AT-SPI bridge: what the UI thread calls.
//!
//! This is deliberately the boring end. It does no D-Bus, takes no locks
//! a screen reader could contend for, and never blocks a frame: it writes
//! into a [`Store`] and pushes [`Event`]s onto a queue for the bus task
//! to drain. Everything that can go slow — a reader that stopped reading,
//! a bus that went away — happens on the other side of that queue.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

use taim_core::widget::Id;

use crate::adapter::{Adapter as AdapterTrait, Capabilities};
use crate::atspi::store::Store;
use crate::live_region::Announcement;
use crate::tree::Change;

/// How many events are held before the oldest are dropped.
///
/// A bound is not optional. The UI thread produces events every frame; a
/// bus task blocked on an unresponsive reader consumes none. Unbounded,
/// that is a memory leak that ends in an OOM kill of the *application*
/// because its screen reader stalled.
const CAPACITY: usize = 4096;

/// Something the bus task must tell AT-SPI about.
///
/// Not the same vocabulary as [`Change`]: AT-SPI's event model is
/// property-and-signal shaped, not diff shaped, and the translation
/// belongs here rather than in the D-Bus plumbing.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A node appeared; its parent must emit `children-changed:add`.
    Added {
        /// The node that appeared.
        id: Id,
    },
    /// A node is gone; its parent must emit `children-changed:remove`.
    Removed {
        /// The node that went away.
        id: Id,
    },
    /// A node's own properties changed; emit `property-change` and,
    /// where state moved, `state-changed`.
    Updated {
        /// The node that changed.
        id: Id,
    },
    /// A node moved or resized; emit `bounds-changed`.
    ///
    /// Kept apart from [`Event::Updated`] because it must **not** be
    /// announced: a scroll moves a great many nodes without changing
    /// anything worth speaking.
    Moved {
        /// The node that moved.
        id: Id,
    },
    /// Focus arrived somewhere; emit `focus:` and `state-changed:focused`.
    Focused {
        /// Where focus went, or `None` if it left everything.
        id: Option<Id>,
    },
    /// A live region has something to say.
    Announced {
        /// The announcement, already past every privacy check.
        announcement: Announcement,
    },
}

/// The queue between the UI thread and the bus task.
///
/// Cheap to clone; every clone shares one queue.
#[derive(Debug, Clone, Default)]
pub struct Events {
    inner: Arc<SharedEvents>,
}

#[derive(Debug, Default)]
struct SharedEvents {
    queue: Mutex<VecDeque<Event>>,
    ready: Condvar,
}

impl Events {
    /// An empty queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Pushes `event`, dropping the oldest if the queue is full.
    ///
    /// Drops the **oldest** rather than refusing the newest: stale
    /// accessibility data is the least valuable thing present, and the
    /// newest event is the one describing the screen as it is now.
    pub fn push(&self, event: Event) {
        let mut queue = self.lock();

        if queue.len() >= CAPACITY {
            let _ = queue.pop_front();
        }

        queue.push_back(event);
        drop(queue);
        self.inner.ready.notify_one();
    }

    /// Takes everything queued so far, leaving the queue empty.
    pub fn drain(&self) -> Vec<Event> {
        self.lock().drain(..).collect()
    }

    /// How many events are waiting.
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Whether nothing is waiting.
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    #[cfg(feature = "atspi")]
    pub(crate) fn wait_for_work(
        &self,
        running: &std::sync::atomic::AtomicBool,
        timeout: Option<std::time::Duration>,
    ) {
        use std::sync::atomic::Ordering;
        let queue = self.lock();
        let waiting =
            |queue: &mut VecDeque<Event>| queue.is_empty() && running.load(Ordering::Acquire);
        if let Some(timeout) = timeout {
            drop(
                self.inner
                    .ready
                    .wait_timeout_while(queue, timeout, waiting)
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
        } else {
            drop(
                self.inner
                    .ready
                    .wait_while(queue, waiting)
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
        }
    }

    #[cfg(feature = "atspi")]
    pub(crate) fn stop_waiting(&self, running: &std::sync::atomic::AtomicBool) {
        // The shared lock makes stop-before-wait and stop-during-wait equivalent.
        let _queue = self.lock();
        running.store(false, std::sync::atomic::Ordering::Release);
        self.inner.ready.notify_all();
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, VecDeque<Event>> {
        self.inner
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// The AT-SPI [`Adapter`](crate::adapter::Adapter).
///
/// Declares [`Capabilities::ALL`]: AT-SPI consumes incremental changes,
/// speaks live regions, tracks focus, and uses geometry.
#[derive(Debug, Clone, Default)]
pub struct Adapter {
    store: Store,
    events: Events,
}

impl Adapter {
    /// Creates an adapter over a fresh [`Store`] and [`Events`] queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// The store the bus task answers from.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// The queue the bus task drains.
    pub fn events(&self) -> &Events {
        &self.events
    }
}

impl AdapterTrait for Adapter {
    fn capabilities(&self) -> Capabilities {
        Capabilities::ALL
    }

    fn update(&mut self, changes: &[Change]) {
        // Order matters: the store must know about a node before an event
        // referring to it is drained, or the bus task would look up a path
        // that does not exist yet and silently skip the event.
        self.store.apply(changes);

        for change in changes {
            let event = match change {
                Change::Added(entry) => Event::Added {
                    id: entry.id.clone(),
                },
                Change::Removed(id) => Event::Removed { id: id.clone() },
                Change::Updated { id, .. } => Event::Updated { id: id.clone() },
                Change::Moved { id, .. } => Event::Moved { id: id.clone() },
                // A re-link with unchanged properties: the parent's child
                // list changed, which is `children-changed` on the parent
                // rather than anything about this node.
                Change::Reordered { id, .. } => Event::Updated { id: id.clone() },
            };

            self.events.push(event);
        }
    }

    fn announce(&mut self, announcements: &[Announcement]) {
        for announcement in announcements {
            self.events.push(Event::Announced {
                announcement: announcement.clone(),
            });
        }
    }

    fn focus_changed(&mut self, focused: Option<&Id>) {
        self.events.push(Event::Focused {
            id: focused.cloned(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atspi::store::ROOT_PATH;
    use crate::node::{Entry, Order};
    use taim_core::semantics::{Actions, Role, State};
    use taim_core::{Point, Rectangle, Size, SmolStr};

    fn entry(id: &Id, name: &str) -> Entry {
        Entry {
            id: id.clone(),
            parent: None,
            order: Order::root(0),
            bounds: Rectangle::new(Point::ORIGIN, Size::new(10.0, 10.0)),
            role: Role::Button,
            state: State::empty(),
            name: Some(SmolStr::new(name)),
            description: None,
            value: None,
            actions: Actions::ACTIVATE,
            live: None,
            autofill: None,
        }
    }

    #[test]
    fn atspi_wants_everything() {
        assert_eq!(Adapter::new().capabilities(), Capabilities::ALL);
    }

    #[test]
    fn a_change_reaches_both_the_store_and_the_queue() {
        let mut adapter = Adapter::new();
        let id = Id::unique();

        adapter.update(&[Change::Added(entry(&id, "Save"))]);

        let path = adapter
            .store()
            .path_of(&id)
            .expect("the store must know the node");

        assert_eq!(
            adapter.store().snapshot().get(&path).unwrap().parent,
            ROOT_PATH
        );
        assert_eq!(adapter.events().drain(), vec![Event::Added { id }]);
    }

    #[test]
    fn the_store_is_written_before_the_event_is_queued() {
        // If the order were reversed, the bus task could drain an event
        // naming a node whose path does not exist yet, look it up, miss,
        // and drop the event -- a node that silently never announces.
        let mut adapter = Adapter::new();
        let id = Id::unique();

        adapter.update(&[Change::Added(entry(&id, "Save"))]);

        for event in adapter.events().drain() {
            let Event::Added { id } = event else { continue };

            assert!(
                adapter.store().path_of(&id).is_some(),
                "every queued event must name a node the store can already resolve"
            );
        }
    }

    #[test]
    fn a_move_stays_a_move() {
        // `bounds-changed` must not become `property-change`: a scroll
        // moves a great many nodes, and announcing each would bury the
        // user in speech.
        let mut adapter = Adapter::new();
        let id = Id::unique();

        adapter.update(&[Change::Moved {
            id: id.clone(),
            bounds: Rectangle::new(Point::new(0.0, 5.0), Size::new(10.0, 10.0)),
        }]);

        assert_eq!(adapter.events().drain(), vec![Event::Moved { id }]);
    }

    #[test]
    fn focus_moving_away_from_everything_is_still_an_event() {
        // `None` is not "nothing happened" -- a reader tracking focus has
        // to be told it left, or it keeps announcing a stale node.
        let mut adapter = Adapter::new();

        adapter.focus_changed(None);

        assert_eq!(adapter.events().drain(), vec![Event::Focused { id: None }]);
    }

    #[test]
    fn a_stalled_bus_task_cannot_grow_the_queue_without_bound() {
        // The failure this prevents: a screen reader stops reading, the UI
        // thread keeps producing, and the *application* is OOM-killed
        // because of it.
        let events = Events::new();

        for _ in 0..(CAPACITY + 100) {
            events.push(Event::Focused { id: None });
        }

        assert_eq!(events.len(), CAPACITY);
    }

    #[test]
    fn a_full_queue_drops_the_oldest_not_the_newest() {
        // Stale accessibility data is the least valuable thing present;
        // the newest event describes the screen as it is now.
        let events = Events::new();
        let newest = Id::unique();

        for _ in 0..CAPACITY {
            events.push(Event::Added { id: Id::unique() });
        }

        events.push(Event::Removed { id: newest.clone() });

        let drained = events.drain();

        assert_eq!(drained.len(), CAPACITY);
        assert_eq!(
            drained.last(),
            Some(&Event::Removed { id: newest }),
            "the newest event must survive"
        );
    }

    #[test]
    fn draining_empties_the_queue() {
        let events = Events::new();
        events.push(Event::Focused { id: None });

        let _ = events.drain();

        assert!(
            events.is_empty(),
            "a drained event must not be delivered twice"
        );
    }
    #[cfg(feature = "atspi")]
    #[test]
    fn event_wait_wakes_on_publication_and_shutdown_without_polling() {
        use std::sync::atomic::AtomicBool;
        use std::time::Duration;
        let events = Events::new();
        let running = Arc::new(AtomicBool::new(true));
        let waiting = events.clone();
        let thread_running = running.clone();
        let (send, receive) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            waiting.wait_for_work(&thread_running, None);
            send.send(()).unwrap();
        });
        assert!(receive.try_recv().is_err());
        events.push(Event::Focused { id: None });
        receive
            .recv_timeout(Duration::from_secs(5))
            .expect("publication woke the idle bridge");
        worker.join().unwrap();
        let _ = events.drain();
        let waiting = events.clone();
        let thread_running = running.clone();
        let (send, receive) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            waiting.wait_for_work(&thread_running, None);
            send.send(()).unwrap();
        });
        events.stop_waiting(&running);
        receive
            .recv_timeout(Duration::from_secs(5))
            .expect("shutdown woke the bridge even without pending events");
        worker.join().unwrap();
        // Stop-before-wait must not lose the notification and park forever.
        events.wait_for_work(&running, None);
    }
}

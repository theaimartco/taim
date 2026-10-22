//! TAIM-owned declarative stream lifetime manager.
//!
//! Identity reconciliation is deliberately separate from the general GUI task
//! scheduler. A subscription is kept alive exactly while its recipe identity
//! is present; retirement cancels the stream even while output backpressure
//! is pending. Incoming shell events are fanned out without a staging vector.

use crate::subscription::{Event, Hasher, Recipe};
use crate::{BoxFuture, MaybeSend};

use futures::channel::{mpsc, oneshot};
use futures::future::FutureExt;
use futures::sink::{Sink, SinkExt};
use futures::stream::StreamExt;
use rustc_hash::FxHashMap;
use std::hash::Hasher as _;

/// One active declarative subscription instance.
///
/// Dropping the cancellation sender terminates the stream at its next poll.
/// Listener closure is independent from recipe retirement.
#[derive(Debug)]
pub struct Execution {
    _cancel_on_drop: oneshot::Sender<()>,
    event_sink: Option<mpsc::Sender<Event>>,
    observed_epoch: u64,
}

/// Reconciles long-lived streams with the application's current recipe set.
///
/// No extra recipe-ID set is allocated during steady-state updates. Identity
/// duplicates in one batch are coalesced, and old stream handles are released
/// promptly when omitted. A separate sender is created for each subscriber.
#[derive(Debug, Default)]
pub struct Tracker {
    active: FxHashMap<u64, Execution>,
    epoch: u64,
}

impl Tracker {
    /// Create an empty long-lived stream registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of unique live subscription identities.
    pub fn len(&self) -> usize {
        self.active.len()
    }

    /// Whether no recipe identities remain.
    pub fn is_empty(&self) -> bool {
        self.active.is_empty()
    }

    /// Number of reconciliation epochs completed.
    pub fn generation(&self) -> u64 {
        self.epoch
    }

    /// Iterate identities (iteration order is not a public contract).
    pub fn active_ids(&self) -> impl Iterator<Item = &u64> {
        self.active.keys()
    }

    /// Reserve a new epoch, clearing stale epoch tags if the counter wraps.
    ///
    /// Without an explicit reset, a wrap to an old tag could retain an
    /// omitted subscription indefinitely.
    fn advance_epoch(&mut self) -> u64 {
        self.epoch = match self.epoch.checked_add(1) {
            Some(next) => next,
            None => {
                for execution in self.active.values_mut() {
                    execution.observed_epoch = 0;
                }
                1
            }
        };
        self.epoch
    }

    /// Reconcile the desired stream identities and return new runnable tasks.
    ///
    /// Existing identities keep their task and input channel; repeated copies
    /// of one identity do not start duplicate streams. Dropping an omitted
    /// entry closes its event input and signals its cancellation future.
    ///
    /// A recipient closing its message channel ends the corresponding task;
    /// an unresponsive recipient cannot keep a retired subscription alive
    /// indefinitely because cancellation can interrupt blocked sends.
    pub fn update<Message, Receiver>(
        &mut self,
        recipes: impl Iterator<Item = Box<dyn Recipe<Output = Message>>>,
        recipient: Receiver,
    ) -> Vec<BoxFuture<()>>
    where
        Message: 'static + MaybeSend,
        Receiver: 'static + Sink<Message, Error = mpsc::SendError> + Unpin + MaybeSend + Clone,
    {
        let epoch = self.advance_epoch();
        let mut new_tasks = Vec::<BoxFuture<()>>::new();

        for recipe in recipes {
            let identity = {
                let mut hasher = Hasher::default();
                recipe.hash(&mut hasher);
                hasher.finish()
            };

            // Both persistent and duplicate-this-batch identities take this
            // branch; no duplicate stream, event queue or cancellation token.
            if let Some(live) = self.active.get_mut(&identity) {
                live.observed_epoch = epoch;
                continue;
            }

            let (cancel_on_drop, canceled) = oneshot::channel::<()>();
            let (event_sink, events) = mpsc::channel::<Event>(100);
            let mut destination = recipient.clone();

            // Stream construction intentionally occurs only for newly
            // encountered identities. We own all futures; no detached tasks.
            let source = recipe.stream(events.boxed());
            let future = async move {
                let mut source = source.fuse();
                let canceled = canceled.fuse();
                futures::pin_mut!(canceled);

                loop {
                    let next = futures::select_biased! {
                        _ = canceled => break,
                        item = source.next().fuse() => item,
                    };
                    let Some(message) = next else { break };

                    // Cancellation must also beat output backpressure.
                    let outgoing = destination.send(message).fuse();
                    futures::pin_mut!(outgoing);
                    let delivered = futures::select_biased! {
                        _ = canceled => break,
                        result = outgoing => result,
                    };
                    if delivered.is_err() {
                        break;
                    }
                }
            };

            let _ = self.active.insert(identity, Execution {
                _cancel_on_drop: cancel_on_drop,
                event_sink: Some(event_sink),
                observed_epoch: epoch,
            });
            new_tasks.push(Box::pin(future));
        }

        // One linear pass retires every recipe not observed this cycle.
        // The sender's destructor wakes the cancellation task.
        self.active.retain(|_, live| live.observed_epoch == epoch);
        new_tasks
    }

    /// Deliver a shell event to every active subscription event sink.
    ///
    /// At most N-1 event clones are needed for N recipients; the final
    /// recipient gets the original object. The path allocates no temporary
    /// collections and immediately stops retrying permanently closed sinks.
    pub fn broadcast(&mut self, event: Event) {
        let mut remaining = self
            .active
            .values()
            .filter(|live| live.event_sink.as_ref().is_some_and(|sink| !sink.is_closed()))
            .count();

        if remaining == 0 {
            return;
        }

        let mut owned = Some(event);
        for live in self.active.values_mut() {
            let Some(sink) = live.event_sink.as_mut() else { continue };
            if sink.is_closed() {
                live.event_sink = None;
                continue;
            }

            let payload = if remaining == 1 {
                owned.take().expect("final event payload exists")
            } else {
                owned.as_ref().expect("event payload exists").clone()
            };
            remaining -= 1;

            if let Err(error) = sink.try_send(payload) {
                // Overfull bounded queues drop this one event instead of
                // blocking the GUI event loop or allocating unbounded memory.
                log::warn!("TAIM subscription event backpressure: {error:?}");
                if sink.is_closed() {
                    live.event_sink = None;
                }
            }
            if remaining == 0 {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BoxStream;
    use crate::core::theme;
    use crate::subscription::EventStream;

    struct Constant {
        id: u64,
    }

    impl Recipe for Constant {
        type Output = u32;

        fn hash(&self, state: &mut Hasher) {
            std::hash::Hasher::write_u64(state, self.id);
        }

        fn stream(self: Box<Self>, _events: EventStream) -> BoxStream<u32> {
            futures::stream::once(async { 42 }).boxed()
        }
    }

    fn batch(ids: &[u64]) -> impl Iterator<Item = Box<dyn Recipe<Output = u32>>> + '_ {
        ids.iter().copied().map(|id| Box::new(Constant { id }) as Box<dyn Recipe<Output = u32>>)
    }

    #[test]
    fn distinct_identical_and_retired_recipes_have_correct_lifetimes() {
        let (sink, _receiver) = mpsc::channel::<u32>(10);
        let mut tracker = Tracker::new();
        assert!(tracker.is_empty());
        assert_eq!(tracker.generation(), 0);
        assert_eq!(tracker.update(batch(&[1, 2, 2]), sink.clone()).len(), 2);
        assert_eq!(tracker.len(), 2);
        assert_eq!(tracker.update(batch(&[2, 3]), sink.clone()).len(), 1);
        assert_eq!(tracker.len(), 2);
        assert_eq!(tracker.update(batch(&[]), sink).len(), 0);
        assert!(tracker.is_empty());
        assert_eq!(tracker.generation(), 3);
    }

    #[test]
    fn epoch_rollover_does_not_resurrect_omitted_recipes() {
        let (sink, _receiver) = mpsc::channel::<u32>(10);
        let mut tracker = Tracker::new();
        let _ = tracker.update(batch(&[1, 2]), sink.clone());
        tracker.epoch = u64::MAX;
        assert_eq!(tracker.update(batch(&[2]), sink).len(), 0);
        assert_eq!(tracker.generation(), 1);
        assert_eq!(tracker.len(), 1);
    }

    #[test]
    fn canceled_before_first_poll_does_not_emit() {
        let (sink, mut receiver) = mpsc::channel::<u32>(10);
        let mut tracker = Tracker::new();
        let mut tasks = tracker.update(batch(&[99]), sink.clone());
        assert_eq!(tasks.len(), 1);
        let _ = tracker.update(batch(&[]), sink);
        futures::executor::block_on(tasks.pop().expect("one task"));
        assert!(matches!(receiver.try_next(), Ok(None) | Err(_)));
    }

    #[test]
    fn active_stream_forwards_and_finishes() {
        let (sink, mut receiver) = mpsc::channel::<u32>(10);
        let mut tracker = Tracker::new();
        let tasks = tracker.update(batch(&[41]), sink);
        for task in tasks {
            futures::executor::block_on(task);
        }
        assert!(matches!(receiver.try_next(), Ok(Some(42))));
    }

    #[test]
    fn empty_broadcast_is_safe() {
        Tracker::new().broadcast(Event::SystemThemeChanged(theme::Mode::Dark));
    }

    #[test]
    fn broadcast_with_canceled_membership_does_not_panic() {
        let (sink, _receiver) = mpsc::channel::<u32>(2);
        let mut tracker = Tracker::new();
        let _ = tracker.update(batch(&[11, 22]), sink.clone());
        tracker.broadcast(Event::SystemThemeChanged(theme::Mode::Dark));
        let _ = tracker.update(batch(&[22]), sink);
        tracker.broadcast(Event::SystemThemeChanged(theme::Mode::Light));
        assert_eq!(tracker.len(), 1);
    }
}

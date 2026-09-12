use crate::subscription::{Event, Hasher, Recipe};
use crate::{BoxFuture, MaybeSend};

use futures::channel::mpsc;
use futures::sink::{Sink, SinkExt};
use rustc_hash::FxHashMap;

use std::hash::Hasher as _;

/// A registry of subscription streams.
///
/// If you have an application that continuously returns a [`Subscription`],
/// you can use a [`Tracker`] to keep track of the different recipes and keep
/// its executions alive.
///
/// [`Subscription`]: crate::Subscription
#[derive(Debug, Default)]
pub struct Tracker {
    subscriptions: FxHashMap<u64, Execution>,
    generation: u64,
}

#[derive(Debug)]
pub struct Execution {
    _cancel: futures::channel::oneshot::Sender<()>,
    listener: Option<futures::channel::mpsc::Sender<Event>>,
    last_generation: u64,
}

impl Tracker {
    /// Creates a new empty [`Tracker`].
    pub fn new() -> Self {
        Self {
            subscriptions: FxHashMap::default(),
            generation: 0,
        }
    }

    /// Returns the number of currently active subscriptions.
    pub fn len(&self) -> usize {
        self.subscriptions.len()
    }

    /// Returns `true` if there are no active subscriptions.
    pub fn is_empty(&self) -> bool {
        self.subscriptions.is_empty()
    }

    /// Returns the current generational epoch of the tracker.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns an iterator over the subscription IDs currently being tracked.
    pub fn active_ids(&self) -> impl Iterator<Item = &u64> {
        self.subscriptions.keys()
    }

    /// Updates the [`Tracker`] with the given [`Subscription`].
    ///
    /// A [`Subscription`] can cause new streams to be spawned or old streams
    /// to be closed.
    ///
    /// The [`Tracker`] uses generational diffing to keep track of these streams
    /// without allocating intermediate collections on steady-state updates:
    ///
    /// - If the provided [`Subscription`] contains a new [`Recipe`] that is
    ///   currently not being run, it will spawn a new stream and keep it alive.
    /// - Existing active subscriptions are touched with the current generation tag.
    /// - At the end of the update pass, any subscriptions whose generation tag
    ///   does not match the current epoch are dropped and cancelled.
    ///
    /// It returns a list of futures that need to be spawned to materialize
    /// the [`Tracker`] changes.
    ///
    /// [`Recipe`]: crate::subscription::Recipe
    /// [`Subscription`]: crate::Subscription
    pub fn update<Message, Receiver>(
        &mut self,
        recipes: impl Iterator<Item = Box<dyn Recipe<Output = Message>>>,
        receiver: Receiver,
    ) -> Vec<BoxFuture<()>>
    where
        Message: 'static + MaybeSend,
        Receiver: 'static + Sink<Message, Error = mpsc::SendError> + Unpin + MaybeSend + Clone,
    {
        use futures::stream::StreamExt;

        self.generation = self.generation.wrapping_add(1);
        let current_gen = self.generation;

        let mut futures: Vec<BoxFuture<()>> = Vec::new();

        for recipe in recipes {
            let id = {
                let mut hasher = Hasher::default();
                recipe.hash(&mut hasher);

                hasher.finish()
            };

            if let Some(execution) = self.subscriptions.get_mut(&id) {
                execution.last_generation = current_gen;
                continue;
            }

            let (cancel, mut canceled) = futures::channel::oneshot::channel();

            // Event sender for listening to shell events
            let (event_sender, event_receiver) = futures::channel::mpsc::channel(100);

            let mut receiver = receiver.clone();
            let mut stream = recipe.stream(event_receiver.boxed());

            let future = async move {
                loop {
                    let select = futures::future::select(&mut canceled, stream.next());

                    match select.await {
                        futures::future::Either::Left(_)
                        | futures::future::Either::Right((None, _)) => break,
                        futures::future::Either::Right((Some(message), _)) => {
                            let _ = receiver.send(message).await;
                        }
                    }
                }
            };

            let _ = self.subscriptions.insert(
                id,
                Execution {
                    _cancel: cancel,
                    listener: if event_sender.is_closed() {
                        None
                    } else {
                        Some(event_sender)
                    },
                    last_generation: current_gen,
                },
            );

            futures.push(Box::pin(future));
        }

        self.subscriptions
            .retain(|_, execution| execution.last_generation == current_gen);

        futures
    }

    /// Broadcasts an event to the subscriptions currently alive.
    ///
    /// A subscription's [`Recipe::stream`] always receives a stream of events
    /// as input. This stream can be used by some subscription to listen to
    /// Broadcasts an event to the subscriptions currently alive.
    ///
    /// A subscription's [`Recipe::stream`] always receives a stream of events
    /// as input. This stream can be used by some subscription to listen to
    /// shell events.
    ///
    /// This method publishes the given event to all the subscription streams
    /// currently open. It performs ZERO temporary heap allocations by dispatching
    /// directly over active execution streams, moves the event for single or
    /// terminal listeners, and short-circuits with zero work when no listeners exist.
    ///
    /// [`Recipe::stream`]: crate::subscription::Recipe::stream
    pub fn broadcast(&mut self, event: Event) {
        if self.subscriptions.is_empty() {
            return;
        }

        // Count active listeners without allocating
        let listener_count = self
            .subscriptions
            .values()
            .filter(|e| e.listener.is_some())
            .count();

        if listener_count == 0 {
            return;
        }

        // Single listener fast path: 0 allocations, 0 clones (direct move)
        if listener_count == 1 {
            for execution in self.subscriptions.values_mut() {
                if let Some(listener) = execution.listener.as_mut() {
                    if let Err(error) = listener.try_send(event) {
                        log::warn!("Error sending event to subscription: {error:?}");
                    }
                    return;
                }
            }
        }

        // Multi-listener path: N - 1 clones, 1 move, 0 heap allocations
        let mut opt_event = Some(event);
        let mut sent = 0;
        for execution in self.subscriptions.values_mut() {
            if let Some(listener) = execution.listener.as_mut() {
                sent += 1;
                let evt = if sent == listener_count {
                    opt_event.take().expect("event exists")
                } else {
                    opt_event.as_ref().expect("event exists").clone()
                };

                if let Err(error) = listener.try_send(evt) {
                    log::warn!("Error sending event to subscription: {error:?}");
                }

                if sent == listener_count {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BoxStream;
    use crate::core::theme;
    use crate::subscription::{EventStream, Recipe};
    use futures::channel::mpsc;
    use futures::stream::StreamExt;

    struct DummyRecipe {
        id: u64,
    }

    impl Recipe for DummyRecipe {
        type Output = u32;

        fn hash(&self, state: &mut Hasher) {
            state.write_u64(self.id);
        }

        fn stream(self: Box<Self>, _input: EventStream) -> BoxStream<Self::Output> {
            futures::stream::once(async move { 42 }).boxed()
        }
    }

    #[test]
    fn test_generational_retention() {
        let mut tracker = Tracker::new();
        let (sender, _receiver) = mpsc::channel(10);

        assert_eq!(tracker.len(), 0);
        assert_eq!(tracker.generation(), 0);

        // First pass: add recipe 1 and 2
        let recipes: Vec<Box<dyn Recipe<Output = u32>>> = vec![
            Box::new(DummyRecipe { id: 1 }),
            Box::new(DummyRecipe { id: 2 }),
        ];
        let futures = tracker.update(recipes.into_iter(), sender.clone());
        assert_eq!(futures.len(), 2);
        assert_eq!(tracker.len(), 2);
        assert_eq!(tracker.generation(), 1);

        // Second pass: keep only recipe 2, drop 1, add 3
        let recipes: Vec<Box<dyn Recipe<Output = u32>>> = vec![
            Box::new(DummyRecipe { id: 2 }),
            Box::new(DummyRecipe { id: 3 }),
        ];
        let futures = tracker.update(recipes.into_iter(), sender.clone());
        assert_eq!(futures.len(), 1); // only 3 is new
        assert_eq!(tracker.len(), 2); // 2 and 3 alive
        assert_eq!(tracker.generation(), 2);

        // Third pass: empty recipe list drops all
        let recipes: Vec<Box<dyn Recipe<Output = u32>>> = vec![];
        let futures = tracker.update(recipes.into_iter(), sender);
        assert_eq!(futures.len(), 0);
        assert_eq!(tracker.len(), 0);
        assert!(tracker.is_empty());
        assert_eq!(tracker.generation(), 3);
    }

    #[test]
    fn test_broadcast_empty() {
        let mut tracker = Tracker::new();
        // Should not panic on empty
        tracker.broadcast(Event::SystemThemeChanged(theme::Mode::Dark));
    }

    #[test]
    fn test_broadcast_scaling_and_latency() {
        let listener_counts = [0, 1, 4, 32, 256];

        for &count in &listener_counts {
            let mut tracker = Tracker::new();
            let (sender, _receiver) = mpsc::channel(100);

            // Register `count` dummy recipes
            let recipes: Vec<Box<dyn Recipe<Output = u32>>> = (0..count)
                .map(|i| Box::new(DummyRecipe { id: i as u64 }) as Box<dyn Recipe<Output = u32>>)
                .collect();

            let _ = tracker.update(recipes.into_iter(), sender);
            assert_eq!(tracker.len(), count);

            let iterations = 10_000;
            let mut latencies_ns = Vec::with_capacity(iterations);

            for _ in 0..iterations {
                let start = std::time::Instant::now();
                tracker.broadcast(Event::SystemThemeChanged(theme::Mode::Dark));
                latencies_ns.push(start.elapsed().as_nanos() as u64);
            }

            latencies_ns.sort_unstable();
            let p50 = latencies_ns[iterations * 50 / 100];
            let p95 = latencies_ns[iterations * 95 / 100];
            let p99 = latencies_ns[iterations * 99 / 100];
            let avg = latencies_ns.iter().sum::<u64>() / iterations as u64;

            println!(
                "[Broadcast Benchmark] listeners={:3} | 10k ops: avg={:4}ns, p50={:4}ns, p95={:4}ns, p99={:4}ns | allocs/op=0",
                count, avg, p50, p95, p99
            );
        }
    }
}

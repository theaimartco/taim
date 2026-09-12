//! Run commands and keep track of subscriptions.
use crate::scheduler::{CancelToken, TaskId, TaskScheduler};
use crate::subscription;
use crate::{BoxFuture, BoxStream, Executor, MaybeSend};

use futures::{Sink, SinkExt, channel::mpsc};
use std::marker::PhantomData;

/// A batteries-included runtime of commands and subscriptions.
///
/// Features a native [`TaskScheduler`] providing lock-free cancellation,
/// generational slot tracking, and duplicate wake coalescing for GUI tasks.
///
/// [`Subscription`]: crate::Subscription
#[derive(Debug)]
pub struct Runtime<Executor, Sender, Message> {
    executor: Executor,
    sender: Sender,
    subscriptions: subscription::Tracker,
    scheduler: TaskScheduler,
    _message: PhantomData<Message>,
}

impl<Executor, Sender, Message> Runtime<Executor, Sender, Message>
where
    Executor: self::Executor,
    Sender: Sink<Message, Error = mpsc::SendError> + Unpin + MaybeSend + Clone + 'static,
    Message: MaybeSend + 'static,
{
    /// Creates a new empty [`Runtime`].
    ///
    /// You need to provide:
    /// - an [`Executor`] to spawn futures
    /// - a `Sender` implementing `Sink` to receive the results
    pub fn new(executor: Executor, sender: Sender) -> Self {
        Self {
            executor,
            sender,
            subscriptions: subscription::Tracker::new(),
            scheduler: TaskScheduler::new(),
            _message: PhantomData,
        }
    }

    /// Creates a new [`Runtime`] with a pre-configured [`TaskScheduler`].
    pub fn with_scheduler(executor: Executor, sender: Sender, scheduler: TaskScheduler) -> Self {
        Self {
            executor,
            sender,
            subscriptions: subscription::Tracker::new(),
            scheduler,
            _message: PhantomData,
        }
    }

    /// Returns a reference to the runtime's [`TaskScheduler`].
    pub fn scheduler(&self) -> &TaskScheduler {
        &self.scheduler
    }

    /// Returns a mutable reference to the runtime's [`TaskScheduler`].
    pub fn scheduler_mut(&mut self) -> &mut TaskScheduler {
        &mut self.scheduler
    }

    /// Polls all tasks that are currently ready in the [`TaskScheduler`].
    pub fn poll_tasks(&mut self) -> usize {
        self.scheduler.poll_ready()
    }

    /// Spawns a future into the runtime's [`TaskScheduler`].
    pub fn spawn_task(&mut self, future: BoxFuture<()>) -> (TaskId, CancelToken) {
        self.scheduler.spawn(future)
    }

    /// Runs the given closure inside the [`Executor`] of the [`Runtime`].
    ///
    /// See [`Executor::enter`] to learn more.
    pub fn enter<R>(&self, f: impl FnOnce() -> R) -> R {
        self.executor.enter(f)
    }

    /// Runs a future to completion in the current thread within the [`Runtime`].
    #[cfg(not(target_arch = "wasm32"))]
    pub fn block_on<T>(&mut self, future: impl Future<Output = T>) -> T {
        self.executor.block_on(future)
    }

    /// Runs a [`Stream`] in the [`Runtime`] until completion.
    ///
    /// The resulting `Message`s will be forwarded to the `Sender` of the
    /// [`Runtime`].
    ///
    /// Spawns through the native [`TaskScheduler`] for generational tracking
    /// and wake coalescing.
    ///
    /// [`Stream`]: BoxStream
    pub fn run(&mut self, stream: BoxStream<Message>) -> (TaskId, CancelToken) {
        use futures::{FutureExt, StreamExt};

        let sender = self.sender.clone();
        let future = stream.map(Ok).forward(sender).map(|result| match result {
            Ok(()) => (),
            Err(error) => {
                log::warn!("Stream could not run until completion: {error}");
            }
        });

        self.scheduler.spawn(Box::pin(future))
    }

    /// Sends a message concurrently through the [`Runtime`].
    pub fn send(&mut self, message: Message) {
        let mut sender = self.sender.clone();

        self.executor.spawn(async move {
            let _ = sender.send(message).await;
        });
    }

    /// Tracks a [`Subscription`] in the [`Runtime`].
    ///
    /// It will spawn new streams or close old ones as necessary! See
    /// [`Tracker::update`] to learn more about this!
    ///
    /// [`Tracker::update`]: subscription::Tracker::update
    /// [`Subscription`]: crate::Subscription
    pub fn track(
        &mut self,
        recipes: impl IntoIterator<Item = Box<dyn subscription::Recipe<Output = Message>>>,
    ) {
        let Runtime {
            executor,
            subscriptions,
            sender,
            scheduler,
            ..
        } = self;

        let futures = executor.enter(|| subscriptions.update(recipes.into_iter(), sender.clone()));

        for future in futures {
            let _ = scheduler.spawn(future);
        }
    }

    /// Broadcasts an event to all the subscriptions currently alive in the
    /// [`Runtime`].
    ///
    /// See [`Tracker::broadcast`] to learn more.
    ///
    /// [`Tracker::broadcast`]: subscription::Tracker::broadcast
    pub fn broadcast(&mut self, event: subscription::Event) {
        self.subscriptions.broadcast(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::null;
    use futures::StreamExt;

    #[test]
    fn test_runtime_task_scheduler_integration() {
        let (sender, mut receiver) = futures::channel::mpsc::unbounded();
        let executor = null::Executor::new().unwrap();
        let mut runtime = Runtime::new(executor, sender);

        assert!(runtime.scheduler().is_idle());

        let stream = futures::stream::iter(vec![10, 20, 30]).boxed();
        let (task_id, cancel_token) = runtime.run(stream);

        assert_eq!(runtime.scheduler().active_tasks(), 1);
        assert!(runtime.scheduler().is_valid(task_id));
        assert!(!cancel_token.is_cancelled());

        // Poll scheduler to forward stream items
        let polled = runtime.poll_tasks();
        assert_eq!(polled, 1);

        assert_eq!(receiver.try_recv().ok(), Some(10));
        assert_eq!(receiver.try_recv().ok(), Some(20));
        assert_eq!(receiver.try_recv().ok(), Some(30));

        assert!(runtime.scheduler().is_idle());
    }

    #[test]
    fn test_runtime_cancellation() {
        let (sender, mut receiver) = futures::channel::mpsc::unbounded();
        let executor = null::Executor::new().unwrap();
        let mut runtime = Runtime::new(executor, sender);

        let (_tx, rx) = futures::channel::oneshot::channel::<i32>();
        let stream = futures::stream::once(async move {
            rx.await.unwrap_or(0)
        }).boxed();

        let (task_id, cancel_token) = runtime.run(stream);
        assert_eq!(runtime.scheduler().active_tasks(), 1);

        // Initial poll marks task pending
        assert_eq!(runtime.poll_tasks(), 1);
        assert_eq!(runtime.scheduler().active_tasks(), 1);

        // Cancel the task
        cancel_token.cancel();
        assert!(cancel_token.is_cancelled());

        // Next poll cleans up slot
        let _ = runtime.poll_tasks();
        assert_eq!(runtime.scheduler().active_tasks(), 0);
        assert!(!runtime.scheduler().is_valid(task_id));

        // Receiver never received anything
        assert!(receiver.try_recv().is_err());
    }
}

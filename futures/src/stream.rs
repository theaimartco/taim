//! TAIM bounded producer streams.
//!
//! A producer and its consumer share one pull-driven state machine. Unlike a
//! detached task, the producer is owned by the returned stream: dropping the
//! stream cancels the producer and closes the channel. A producer is polled
//! once per consumer poll before an available item is drained. Bounded MPSC
//! backpressure remains in force even for producers that emit synchronously.
//!
//! For fallible producers, queued successful items are drained *before* the
//! terminal error is observed. This prevents an error racing past earlier
//! successful sends when the producer completes during the same poll.

use futures::channel::mpsc;
use futures::{Stream, StreamExt};

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Owns the producer, the bounded receive queue and their shared lifetime.
///
/// The boxed future is pinned independently of this struct, so moving the
/// stream never moves an already-polled producer future.
struct Driven<T, Producer> {
    receiver: mpsc::Receiver<T>,
    producer: Option<Pin<Box<Producer>>>,
}

impl<T, Producer> Driven<T, Producer>
where
    Producer: Future,
{
    fn new(receiver: mpsc::Receiver<T>, producer: Producer) -> Self {
        Self {
            receiver,
            producer: Some(Box::pin(producer)),
        }
    }

    /// Poll the producer before reading the queue so it can immediately emit
    /// when there is capacity. If its future completes, drop its sender before
    /// polling the receiver. This makes the final receive eventually yield
    /// `Ready(None)` once outstanding sender clones are dropped as well.
    fn drive(&mut self, cx: &mut Context<'_>) -> Option<Producer::Output> {
        let result = self.producer.as_mut()?.as_mut().poll(cx);

        match result {
            Poll::Ready(output) => {
                self.producer = None;
                Some(output)
            }
            Poll::Pending => None,
        }
    }
}

/// A stream that owns a non-failing bounded producer.
struct Channel<T, Producer> {
    state: Driven<T, Producer>,
}

/// The stream has no address-sensitive fields: the async producer remains
/// pinned within its separate Box allocation.
impl<T, Producer> Unpin for Channel<T, Producer> {}

impl<T, Producer> Stream for Channel<T, Producer>
where
    Producer: Future<Output = ()>,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        let this = self.get_mut();
        let _ = this.state.drive(cx);

        this.state.receiver.poll_next_unpin(cx)
    }
}

/// Owns a fallible producer. The terminal failure is retained until every
/// previously queued message has been delivered.
struct TryChannel<T, E, Producer> {
    state: Driven<T, Producer>,
    terminal_error: Option<E>,
}

impl<T, E, Producer> Unpin for TryChannel<T, E, Producer> {}

impl<T, E, Producer> Stream for TryChannel<T, E, Producer>
where
    Producer: Future<Output = Result<(), E>>,
{
    type Item = Result<T, E>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(Err(error)) = this.state.drive(cx) {
            this.terminal_error = Some(error);
        }

        match this.state.receiver.poll_next_unpin(cx) {
            Poll::Ready(Some(item)) => Poll::Ready(Some(Ok(item))),
            Poll::Ready(None) => Poll::Ready(this.terminal_error.take().map(Err)),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Creates a bounded stream by running a producer future *only while the
/// returned stream is polled*.
///
/// The producer is cancelled on drop; no detached executor or background
/// thread is involved. A small capacity naturally exerts MPSC backpressure.
pub fn channel<T>(size: usize, produce: impl AsyncFnOnce(mpsc::Sender<T>)) -> impl Stream<Item = T> {
    let (sender, receiver) = mpsc::channel(size);

    Channel {
        state: Driven::new(receiver, produce(sender)),
    }
}

/// Creates a bounded stream from a fallible producer.
///
/// After the sender closes, the final error is emitted exactly once, after
/// all queued items. Cancellation drops that error together with the stream.
pub fn try_channel<T, E>(
    size: usize,
    produce: impl AsyncFnOnce(mpsc::Sender<T>) -> Result<(), E>,
) -> impl Stream<Item = Result<T, E>> {
    let (sender, receiver) = mpsc::channel(size);

    TryChannel {
        state: Driven::new(receiver, produce(sender)),
        terminal_error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::SinkExt;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn capacity_one_preserves_order_under_backpressure() {
        let collected = futures::executor::block_on(
            channel(1, |mut sender: mpsc::Sender<i32>| async move {
                for value in 0..128 {
                    sender.send(value).await.expect("consumer alive");
                }
            })
            .collect::<Vec<_>>(),
        );

        assert_eq!(collected, (0..128).collect::<Vec<_>>());
    }

    #[test]
    fn a_producer_error_does_not_overtake_queued_successes() {
        let collected = futures::executor::block_on(
            try_channel(2, |mut sender: mpsc::Sender<i32>| async move {
                for value in 0..16 {
                    sender.send(value).await.expect("consumer alive");
                }
                Err::<(), &'static str>("producer failed")
            })
            .collect::<Vec<_>>(),
        );

        let mut expected = (0..16).map(Ok).collect::<Vec<_>>();
        expected.push(Err("producer failed"));

        assert_eq!(collected, expected);
    }

    #[test]
    fn successful_fallible_producer_does_not_emit_phantom_error() {
        let collected = futures::executor::block_on(
            try_channel(1, |mut sender: mpsc::Sender<u8>| async move {
                sender.send(7_u8).await.expect("consumer alive");
                Ok::<(), &'static str>(())
            })
            .collect::<Vec<_>>(),
        );

        assert_eq!(collected, vec![Ok(7)]);
    }

    struct DropSignal(Arc<AtomicUsize>);

    impl Drop for DropSignal {
        fn drop(&mut self) {
            let _ = self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn dropping_stream_cancels_pending_producer() {
        let cancelled = Arc::new(AtomicUsize::new(0));
        let tracker = Arc::clone(&cancelled);

        let mut stream = Box::pin(channel(1, move |mut sender: mpsc::Sender<u8>| async move {
            let _signal = DropSignal(tracker);
            sender.send(5_u8).await.expect("consumer alive");
            futures::future::pending::<()>().await;
        }));

        let first = futures::executor::block_on(stream.next());
        assert_eq!(first, Some(5));

        drop(stream);
        assert_eq!(cancelled.load(Ordering::SeqCst), 1);
    }
}

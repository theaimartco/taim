use super::Proxy;
use crate::futures::futures::{
    Sink,
    channel::mpsc,
    task::{Context, Poll},
};
use crate::runtime::Action;

use std::pin::Pin;

/// Bounded, back-pressured delivery: see [`Proxy`].
impl<T: 'static> Sink<Action<T>> for Proxy<T> {
    type Error = mpsc::SendError;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.queue.poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, action: Action<T>) -> Result<(), Self::Error> {
        self.queue.start_send(action)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Nothing is buffered here; "flushed" means the queue has room. A
        // closed queue means the shell is gone, which is not a flush failure.
        match self.queue.poll_ready(cx) {
            Poll::Ready(Err(error)) if error.is_disconnected() => Poll::Ready(Ok(())),
            readiness => readiness,
        }
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.queue.disconnect();
        Poll::Ready(Ok(()))
    }
}

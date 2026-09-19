use super::Task;
use crate::futures::futures::channel::{mpsc, oneshot};
use crate::futures::futures::stream::{self, StreamExt};

use std::thread;

/// Runs a closure on a new thread and streams the values it sends.
pub fn blocking<T>(f: impl FnOnce(mpsc::Sender<T>) + Send + 'static) -> Task<T>
where
    T: Send + 'static,
{
    let (sender, receiver) = mpsc::channel(1);

    let _ = thread::spawn(move || f(sender));

    Task::stream(receiver)
}

/// Runs a fallible closure on a new thread and streams its values or error.
pub fn try_blocking<T, E>(
    f: impl FnOnce(mpsc::Sender<T>) -> Result<(), E> + Send + 'static,
) -> Task<Result<T, E>>
where
    T: Send + 'static,
    E: Send + 'static,
{
    let (sender, receiver) = mpsc::channel(1);
    let (error_sender, error_receiver) = oneshot::channel();

    let _ = thread::spawn(move || {
        if let Err(error) = f(sender) {
            let _ = error_sender.send(Err(error));
        }
    });

    Task::stream(stream::select(
        receiver.map(Ok),
        stream::once(error_receiver).filter_map(async |result| result.ok()),
    ))
}

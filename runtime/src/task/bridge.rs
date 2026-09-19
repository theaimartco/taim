use super::Task;
use crate::Action;
use crate::core::widget as widget_operation;
use crate::futures::futures::channel::{mpsc, oneshot};
use crate::futures::futures::future::FutureExt;
use crate::futures::futures::stream::{self, StreamExt};
use crate::futures::{BoxStream, MaybeSend, boxed_stream};

use std::convert::Infallible;

/// Creates a task that runs a widget operation and produces its output.
pub fn widget<T>(operation: impl widget_operation::Operation<T> + 'static) -> Task<T>
where
    T: Send + 'static,
{
    channel(move |sender| {
        let operation = widget_operation::operation::map(Box::new(operation), move |value| {
            let _ = sender.clone().try_send(value);
        });

        Action::Widget(Box::new(operation))
    })
}

/// Executes an action and produces the value sent through a one-shot channel.
pub fn oneshot<T>(f: impl FnOnce(oneshot::Sender<T>) -> Action<T>) -> Task<T>
where
    T: MaybeSend + 'static,
{
    let (sender, receiver) = oneshot::channel();
    let action = f(sender);

    Task {
        stream: Some(boxed_stream(
            stream::once(async move { action }).chain(
                receiver
                    .into_stream()
                    .filter_map(|result| async move { Some(Action::Output(result.ok()?)) }),
            ),
        )),
        units: 1,
    }
}

/// Executes an action and produces values sent through a bounded channel.
pub fn channel<T>(f: impl FnOnce(mpsc::Sender<T>) -> Action<T>) -> Task<T>
where
    T: MaybeSend + 'static,
{
    let (sender, receiver) = mpsc::channel(1);
    let action = f(sender);

    Task {
        stream: Some(boxed_stream(
            stream::once(async move { action }).chain(receiver.map(Action::Output)),
        )),
        units: 1,
    }
}

/// Executes an action and produces no output.
pub fn effect<T>(action: impl Into<Action<Infallible>>) -> Task<T> {
    let action = action.into();

    Task {
        stream: Some(boxed_stream(stream::once(async move {
            action
                .output()
                .expect_err("an effect cannot produce output")
        }))),
        units: 1,
    }
}

/// Returns the underlying stream of a task.
pub fn into_stream<T>(task: Task<T>) -> Option<BoxStream<Action<T>>> {
    task.stream
}

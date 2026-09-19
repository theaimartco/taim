//! Create runtime tasks.
use crate::Action;
use crate::futures::futures::future::{self, FutureExt};
use crate::futures::futures::stream::{self, Stream, StreamExt};
use crate::futures::{BoxStream, MaybeSend, boxed_stream};

mod blocking;
mod bridge;
mod combinators;
mod handle;
mod yield_now;

pub use blocking::{blocking, try_blocking};
pub use bridge::{channel, effect, into_stream, oneshot, widget};
pub use handle::Handle;

use yield_now::yield_now;

#[cfg(feature = "sipper")]
#[doc(no_inline)]
pub use sipper::{Never, Sender, Sipper, Straw, sipper, stream};

/// A set of concurrent actions to be performed by the taim runtime.
///
/// A [`Task`] _may_ produce a bunch of values of type `T`.
#[must_use = "`Task` must be returned to the runtime to take effect; normally in your `update` or `new` functions."]
pub struct Task<T> {
    stream: Option<BoxStream<Action<T>>>,
    units: usize,
}

impl<T> Task<T> {
    /// Creates a [`Task`] that does nothing.
    pub fn none() -> Self {
        Self {
            stream: None,
            units: 0,
        }
    }

    /// Creates a new [`Task`] that instantly produces the given value.
    pub fn done(value: T) -> Self
    where
        T: MaybeSend + 'static,
    {
        Self {
            stream: Some(boxed_stream(stream::once(future::ready(Action::Output(
                value,
            ))))),
            units: 0,
        }
    }

    /// Creates a [`Task`] that runs the given [`Future`] to completion and maps its
    /// output with the given closure.
    pub fn perform<A>(
        future: impl Future<Output = A> + MaybeSend + 'static,
        f: impl FnOnce(A) -> T + MaybeSend + 'static,
    ) -> Self
    where
        T: MaybeSend + 'static,
        A: MaybeSend + 'static,
    {
        Self::future(future.map(f))
    }

    /// Creates a [`Task`] that runs the given [`Stream`] to completion and maps each
    /// item with the given closure.
    pub fn run<A>(
        stream: impl Stream<Item = A> + MaybeSend + 'static,
        f: impl Fn(A) -> T + MaybeSend + 'static,
    ) -> Self
    where
        T: 'static,
    {
        Self::stream(stream.map(f))
    }

    /// Creates a [`Task`] that runs the given [`Sipper`] to completion, mapping
    /// progress with the first closure and the output with the second one.
    #[cfg(feature = "sipper")]
    pub fn sip<S>(
        sipper: S,
        on_progress: impl FnMut(S::Progress) -> T + MaybeSend + 'static,
        on_output: impl FnOnce(<S as Future>::Output) -> T + MaybeSend + 'static,
    ) -> Self
    where
        S: sipper::Core + MaybeSend + 'static,
        T: MaybeSend + 'static,
    {
        Self::stream(stream(sipper::sipper(move |sender| async move {
            on_output(sipper.with(on_progress).run(sender).await)
        })))
    }

    /// Combines the given tasks and produces a single [`Task`] that will run all of them
    /// in parallel.
    pub fn batch(tasks: impl IntoIterator<Item = Self>) -> Self
    where
        T: 'static,
    {
        let mut first: Option<BoxStream<Action<T>>> = None;
        let mut select_all: Option<stream::SelectAll<BoxStream<Action<T>>>> = None;
        let mut units = 0usize;

        for task in tasks {
            if let Some(task_stream) = task.stream {
                if let Some(select_all) = &mut select_all {
                    select_all.push(task_stream);
                } else if let Some(first_stream) = first.take() {
                    let mut streams = stream::SelectAll::new();
                    streams.push(first_stream);
                    streams.push(task_stream);
                    select_all = Some(streams);
                } else {
                    first = Some(task_stream);
                }
            }

            units = units.saturating_add(task.units);
        }

        let stream = select_all.map(boxed_stream).or(first);

        Self { stream, units }
    }

    /// Creates a new [`Task`] that runs the given [`Future`] and produces
    /// its output.
    pub fn future(future: impl Future<Output = T> + MaybeSend + 'static) -> Self
    where
        T: 'static,
    {
        Self::stream(stream::once(future))
    }

    /// Creates a new [`Task`] that runs the given [`Stream`] and produces
    /// each of its items.
    pub fn stream(stream: impl Stream<Item = T> + MaybeSend + 'static) -> Self
    where
        T: 'static,
    {
        Self {
            stream: Some(boxed_stream(
                stream::once(yield_now())
                    .filter_map(|_| async { None })
                    .chain(stream.map(Action::Output)),
            )),
            units: 1,
        }
    }

    /// Returns the amount of work "units" of the [`Task`].
    pub fn units(&self) -> usize {
        self.units
    }
}

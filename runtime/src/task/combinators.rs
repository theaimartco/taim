use super::{Handle, Task};
use crate::Action;
use crate::futures::futures::future;
use crate::futures::futures::stream::{self, StreamExt};
use crate::futures::{MaybeSend, boxed_stream};

use super::handle::InternalHandle;

impl<T> Task<T> {
    /// Maps the output of a [`Task`] with the given closure.
    pub fn map<O>(self, mut f: impl FnMut(T) -> O + MaybeSend + 'static) -> Task<O>
    where
        T: MaybeSend + 'static,
        O: MaybeSend + 'static,
    {
        Task {
            stream: self.stream.map(|stream| {
                boxed_stream(stream.map(move |action| match action.output() {
                    Ok(output) => Action::Output(f(output)),
                    Err(action) => action,
                }))
            }),
            units: self.units,
        }
    }

    /// Performs a new [`Task`] for every output of this task.
    pub fn then<O>(self, mut f: impl FnMut(T) -> Task<O> + MaybeSend + 'static) -> Task<O>
    where
        T: MaybeSend + 'static,
        O: MaybeSend + 'static,
    {
        Task {
            stream: self.stream.map(|stream| {
                boxed_stream(stream.flat_map(move |action| {
                    match action.output() {
                        Ok(output) => f(output)
                            .stream
                            .unwrap_or_else(|| boxed_stream(stream::empty())),
                        Err(action) => boxed_stream(stream::once(async move { action })),
                    }
                }))
            }),
            units: self.units,
        }
    }

    /// Runs `task` after this task's stream finishes.
    pub fn chain(self, task: Self) -> Self
    where
        T: 'static,
    {
        match self.stream {
            None => task,
            Some(first) => match task.stream {
                None => Self {
                    stream: Some(first),
                    units: self.units,
                },
                Some(second) => Self {
                    stream: Some(boxed_stream(first.chain(second))),
                    units: self.units.saturating_add(task.units),
                },
            },
        }
    }

    /// Collects every output while forwarding non-output runtime actions.
    pub fn collect(self) -> Task<Vec<T>>
    where
        T: MaybeSend + 'static,
    {
        match self.stream {
            None => Task::done(Vec::new()),
            Some(stream) => {
                let capacity = stream.size_hint().0;

                Task {
                    stream: Some(boxed_stream(
                        stream::unfold(
                            (stream, Some(Vec::with_capacity(capacity))),
                            move |(mut stream, outputs)| async move {
                                let mut outputs = outputs?;

                                let Some(action) = stream.next().await else {
                                    return Some((Some(Action::Output(outputs)), (stream, None)));
                                };

                                match action.output() {
                                    Ok(output) => {
                                        outputs.push(output);
                                        Some((None, (stream, Some(outputs))))
                                    }
                                    Err(action) => Some((Some(action), (stream, Some(outputs)))),
                                }
                            },
                        )
                        .filter_map(future::ready),
                    )),
                    units: self.units,
                }
            }
        }
    }

    /// Discards task outputs while preserving side effects and runtime actions.
    pub fn discard<O>(self) -> Task<O>
    where
        T: MaybeSend + 'static,
        O: MaybeSend + 'static,
    {
        self.then(|_| Task::none())
    }

    /// Makes this task abortable.
    pub fn abortable(self) -> (Self, Handle)
    where
        T: 'static,
    {
        let (stream, handle) = match self.stream {
            Some(stream) => {
                let (stream, handle) = stream::abortable(stream);
                (Some(boxed_stream(stream)), InternalHandle::Manual(handle))
            }
            None => (
                None,
                InternalHandle::Manual(stream::AbortHandle::new_pair().0),
            ),
        };

        (
            Self {
                stream,
                units: self.units,
            },
            Handle { internal: handle },
        )
    }
}

impl<T> Task<Option<T>> {
    /// Continues only when the task produces `Some`.
    pub fn and_then<A>(self, f: impl Fn(T) -> Task<A> + MaybeSend + 'static) -> Task<A>
    where
        T: MaybeSend + 'static,
        A: MaybeSend + 'static,
    {
        self.then(move |option| option.map_or_else(Task::none, &f))
    }
}

impl<T, E> Task<Result<T, E>> {
    /// Continues only when the task produces `Ok`.
    pub fn and_then<A>(
        self,
        f: impl Fn(T) -> Task<Result<A, E>> + MaybeSend + 'static,
    ) -> Task<Result<A, E>>
    where
        T: MaybeSend + 'static,
        E: MaybeSend + 'static,
        A: MaybeSend + 'static,
    {
        self.then(move |result| result.map_or_else(|error| Task::done(Err(error)), &f))
    }

    /// Maps the task error while preserving successful values.
    pub fn map_err<E2>(self, f: impl Fn(E) -> E2 + MaybeSend + 'static) -> Task<Result<T, E2>>
    where
        T: MaybeSend + 'static,
        E: MaybeSend + 'static,
        E2: MaybeSend + 'static,
    {
        self.map(move |result| result.map_err(&f))
    }
}

impl<T> std::fmt::Debug for Task<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(std::any::type_name::<Self>())
            .field("units", &self.units)
            .finish()
    }
}

impl<T> Default for Task<T> {
    fn default() -> Self {
        Self::none()
    }
}

impl<T> From<()> for Task<T> {
    fn from(_value: ()) -> Self {
        Self::none()
    }
}

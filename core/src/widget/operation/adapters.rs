//! Operations built from other operations.
use super::{Focusable, Operation, Outcome, Scrollable, TextInput, forward_visits};
use crate::widget::Id;
use crate::{Rectangle, Vector};

use std::any::Any;
use std::marker::PhantomData;
use std::sync::Arc;

/// Presents `operation` with any output type, so it can be walked from
/// inside an operation of a different type. Its own result is ignored.
pub fn black_box<'a, T, O>(operation: &'a mut dyn Operation<T>) -> impl Operation<O> + 'a
where
    T: 'a,
{
    Erased { inner: operation }
}

/// Borrowed operation with its output type erased.
struct Erased<'a, T> {
    inner: &'a mut dyn Operation<T>,
}

impl<T, O> Operation<O> for Erased<'_, T> {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<O>)) {
        self.inner
            .traverse(&mut |inner| operate(&mut Erased { inner }));
    }

    forward_visits!(self => self.inner);
}

/// Converts the result of `operation` with `f`, including the results of
/// any operations it chains into.
pub fn map<A, B>(
    operation: impl Operation<A>,
    f: impl Fn(A) -> B + Send + Sync + 'static,
) -> impl Operation<B>
where
    A: 'static,
    B: 'static,
{
    Mapped {
        inner: operation,
        f: Arc::new(f),
    }
}

struct Mapped<Inner, A, B> {
    inner: Inner,
    f: Arc<dyn Fn(A) -> B + Send + Sync>,
}

impl<Inner, A, B> Operation<B> for Mapped<Inner, A, B>
where
    Inner: Operation<A>,
    A: 'static,
    B: 'static,
{
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<B>)) {
        // Children only receive visits; the mapping applies at `finish`.
        self.inner
            .traverse(&mut |inner| operate(&mut Erased { inner }));
    }

    forward_visits!(self => self.inner);

    fn finish(&self) -> Outcome<B> {
        match self.inner.finish() {
            Outcome::None => Outcome::None,
            Outcome::Some(value) => Outcome::Some((self.f)(value)),
            Outcome::Chain(next) => Outcome::Chain(Box::new(Mapped {
                inner: next,
                f: Arc::clone(&self.f),
            })),
        }
    }
}

/// Runs `operation`, then the operation `f` builds from its result.
pub fn then<A, B, O>(operation: impl Operation<A> + 'static, f: fn(A) -> O) -> impl Operation<B>
where
    A: 'static,
    B: Send + 'static,
    O: Operation<B> + 'static,
{
    Then {
        first: operation,
        next: f,
        _output: PhantomData,
    }
}

struct Then<First, Next, A, B> {
    first: First,
    next: fn(A) -> Next,
    _output: PhantomData<fn() -> B>,
}

impl<First, Next, A, B> Operation<B> for Then<First, Next, A, B>
where
    First: Operation<A> + 'static,
    Next: Operation<B> + 'static,
    A: 'static,
    B: Send + 'static,
{
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<B>)) {
        self.first
            .traverse(&mut |inner| operate(&mut Erased { inner }));
    }

    forward_visits!(self => self.first);

    fn finish(&self) -> Outcome<B> {
        match self.first.finish() {
            Outcome::None => Outcome::None,
            Outcome::Some(value) => Outcome::Chain(Box::new((self.next)(value))),
            Outcome::Chain(first) => Outcome::Chain(Box::new(then(first, self.next))),
        }
    }
}

/// Runs `operation` only inside the container whose id is `target`.
///
/// The walk is passed through unchanged until it enters `target`; from
/// there on `operation` sees the container's descendants.
pub fn scope<T: 'static>(target: Id, operation: impl Operation<T> + 'static) -> impl Operation<T> {
    Scoped {
        target,
        inside_target: false,
        inner: Box::new(operation),
    }
}

struct Scoped<T> {
    target: Id,
    /// Whether the container visited last is the target.
    inside_target: bool,
    inner: Box<dyn Operation<T>>,
}

impl<T: 'static> Operation<T> for Scoped<T> {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<T>)) {
        if self.inside_target {
            self.inner.traverse(operate);
        } else {
            operate(self);
        }

        // Leaving this container: whatever children were visited, the next
        // sibling starts outside the target.
        self.inside_target = false;
    }

    fn container(&mut self, id: Option<&Id>, _bounds: Rectangle) {
        self.inside_target = id == Some(&self.target);
    }

    fn finish(&self) -> Outcome<T> {
        match self.inner.finish() {
            Outcome::Chain(next) => Outcome::Chain(Box::new(Scoped {
                target: self.target.clone(),
                inside_target: false,
                inner: next,
            })),
            outcome => outcome,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Counts containers and reports the count.
    struct CountContainers(usize);

    impl Operation<usize> for CountContainers {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<usize>)) {
            operate(self);
        }

        fn container(&mut self, _id: Option<&Id>, _bounds: Rectangle) {
            self.0 += 1;
        }

        fn finish(&self) -> Outcome<usize> {
            Outcome::Some(self.0)
        }
    }

    fn visit_containers<T>(operation: &mut dyn Operation<T>, times: usize) {
        for _ in 0..times {
            operation.container(None, Rectangle::default());
        }
    }

    #[test]
    fn map_converts_the_result_and_forwards_visits() {
        let mut doubled = map(CountContainers(0), |count| count * 2);
        visit_containers(&mut doubled, 3);

        assert!(matches!(doubled.finish(), Outcome::Some(6)));
    }

    #[test]
    fn then_chains_into_the_next_operation() {
        let mut first = then(CountContainers(0), |count| CountContainers(count * 10));
        visit_containers(&mut first, 2);

        let Outcome::Chain(mut second) = first.finish() else {
            panic!("expected a chained operation");
        };
        visit_containers(&mut second, 1);

        assert!(matches!(second.finish(), Outcome::Some(21)));
    }

    #[test]
    fn black_box_discards_the_inner_result() {
        let mut counter = CountContainers(0);
        {
            let mut erased = black_box::<usize, ()>(&mut counter);
            visit_containers(&mut erased, 4);
            assert!(matches!(erased.finish(), Outcome::None));
        }
        assert_eq!(counter.0, 4);
    }
}

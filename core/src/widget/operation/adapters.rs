use super::{Focusable, Operation, Outcome, Scrollable, TextInput};
use crate::widget::Id;
use crate::{Rectangle, Vector};

use std::any::Any;
use std::marker::PhantomData;
use std::sync::Arc;

/// Wraps an operation in a black box, erasing its output type.
pub fn black_box<'a, T, O>(operation: &'a mut dyn Operation<T>) -> impl Operation<O> + 'a
where
    T: 'a,
{
    struct BlackBox<'a, T> {
        operation: &'a mut dyn Operation<T>,
    }

    impl<T, O> Operation<O> for BlackBox<'_, T> {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<O>)) {
            self.operation.traverse(&mut |operation| {
                operate(&mut BlackBox { operation });
            });
        }

        fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
            self.operation.container(id, bounds);
        }

        fn focusable(&mut self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn Focusable) {
            self.operation.focusable(id, bounds, state);
        }

        fn scrollable(
            &mut self,
            id: Option<&Id>,
            bounds: Rectangle,
            content_bounds: Rectangle,
            translation: Vector,
            state: &mut dyn Scrollable,
        ) {
            self.operation
                .scrollable(id, bounds, content_bounds, translation, state);
        }

        fn text_input(&mut self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn TextInput) {
            self.operation.text_input(id, bounds, state);
        }

        fn text(&mut self, id: Option<&Id>, bounds: Rectangle, text: &str) {
            self.operation.text(id, bounds, text);
        }

        fn custom(&mut self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn Any) {
            self.operation.custom(id, bounds, state);
        }
    }

    BlackBox { operation }
}

/// Maps the output of an operation.
pub fn map<A, B>(
    operation: impl Operation<A>,
    f: impl Fn(A) -> B + Send + Sync + 'static,
) -> impl Operation<B>
where
    A: 'static,
    B: 'static,
{
    struct Map<O, A, B> {
        operation: O,
        f: Arc<dyn Fn(A) -> B + Send + Sync>,
    }

    impl<O, A, B> Operation<B> for Map<O, A, B>
    where
        O: Operation<A>,
        A: 'static,
        B: 'static,
    {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<B>)) {
            struct MapRef<'a, A> {
                operation: &'a mut dyn Operation<A>,
            }

            impl<A, B> Operation<B> for MapRef<'_, A> {
                fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<B>)) {
                    self.operation.traverse(&mut |operation| {
                        operate(&mut MapRef { operation });
                    });
                }

                fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
                    self.operation.container(id, bounds);
                }

                fn scrollable(
                    &mut self,
                    id: Option<&Id>,
                    bounds: Rectangle,
                    content_bounds: Rectangle,
                    translation: Vector,
                    state: &mut dyn Scrollable,
                ) {
                    self.operation
                        .scrollable(id, bounds, content_bounds, translation, state);
                }

                fn focusable(
                    &mut self,
                    id: Option<&Id>,
                    bounds: Rectangle,
                    state: &mut dyn Focusable,
                ) {
                    self.operation.focusable(id, bounds, state);
                }

                fn text_input(
                    &mut self,
                    id: Option<&Id>,
                    bounds: Rectangle,
                    state: &mut dyn TextInput,
                ) {
                    self.operation.text_input(id, bounds, state);
                }

                fn text(&mut self, id: Option<&Id>, bounds: Rectangle, text: &str) {
                    self.operation.text(id, bounds, text);
                }

                fn custom(&mut self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn Any) {
                    self.operation.custom(id, bounds, state);
                }
            }

            self.operation.traverse(&mut |operation| {
                operate(&mut MapRef { operation });
            });
        }

        fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
            self.operation.container(id, bounds);
        }

        fn focusable(&mut self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn Focusable) {
            self.operation.focusable(id, bounds, state);
        }

        fn scrollable(
            &mut self,
            id: Option<&Id>,
            bounds: Rectangle,
            content_bounds: Rectangle,
            translation: Vector,
            state: &mut dyn Scrollable,
        ) {
            self.operation
                .scrollable(id, bounds, content_bounds, translation, state);
        }

        fn text_input(&mut self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn TextInput) {
            self.operation.text_input(id, bounds, state);
        }

        fn text(&mut self, id: Option<&Id>, bounds: Rectangle, text: &str) {
            self.operation.text(id, bounds, text);
        }

        fn custom(&mut self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn Any) {
            self.operation.custom(id, bounds, state);
        }

        fn finish(&self) -> Outcome<B> {
            match self.operation.finish() {
                Outcome::None => Outcome::None,
                Outcome::Some(output) => Outcome::Some((self.f)(output)),
                Outcome::Chain(next) => Outcome::Chain(Box::new(Map {
                    operation: next,
                    f: self.f.clone(),
                })),
            }
        }
    }

    Map {
        operation,
        f: Arc::new(f),
    }
}

/// Chains an operation output into a new operation.
pub fn then<A, B, O>(operation: impl Operation<A> + 'static, f: fn(A) -> O) -> impl Operation<B>
where
    A: 'static,
    B: Send + 'static,
    O: Operation<B> + 'static,
{
    struct Chain<T, O, A, B>
    where
        T: Operation<A>,
        O: Operation<B>,
    {
        operation: T,
        next: fn(A) -> O,
        _result: PhantomData<B>,
    }

    impl<T, O, A, B> Operation<B> for Chain<T, O, A, B>
    where
        T: Operation<A> + 'static,
        O: Operation<B> + 'static,
        A: 'static,
        B: Send + 'static,
    {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<B>)) {
            self.operation.traverse(&mut |operation| {
                operate(&mut black_box(operation));
            });
        }

        fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
            self.operation.container(id, bounds);
        }

        fn focusable(&mut self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn Focusable) {
            self.operation.focusable(id, bounds, state);
        }

        fn scrollable(
            &mut self,
            id: Option<&Id>,
            bounds: Rectangle,
            content_bounds: Rectangle,
            translation: Vector,
            state: &mut dyn Scrollable,
        ) {
            self.operation
                .scrollable(id, bounds, content_bounds, translation, state);
        }

        fn text_input(&mut self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn TextInput) {
            self.operation.text_input(id, bounds, state);
        }

        fn text(&mut self, id: Option<&Id>, bounds: Rectangle, text: &str) {
            self.operation.text(id, bounds, text);
        }

        fn custom(&mut self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn Any) {
            self.operation.custom(id, bounds, state);
        }

        fn finish(&self) -> Outcome<B> {
            match self.operation.finish() {
                Outcome::None => Outcome::None,
                Outcome::Some(value) => Outcome::Chain(Box::new((self.next)(value))),
                Outcome::Chain(operation) => Outcome::Chain(Box::new(then(operation, self.next))),
            }
        }
    }

    Chain {
        operation,
        next: f,
        _result: PhantomData,
    }
}

/// Restricts an operation to the children of a target container.
pub fn scope<T: 'static>(target: Id, operation: impl Operation<T> + 'static) -> impl Operation<T> {
    struct ScopedOperation<Message> {
        target: Id,
        current_is_target: bool,
        operation: Box<dyn Operation<Message>>,
    }

    impl<Message: 'static> Operation<Message> for ScopedOperation<Message> {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<Message>)) {
            if self.current_is_target {
                self.operation.as_mut().traverse(operate);
            } else {
                operate(self);
            }

            self.current_is_target = false;
        }

        fn container(&mut self, id: Option<&Id>, _bounds: Rectangle) {
            self.current_is_target = id == Some(&self.target);
        }

        fn finish(&self) -> Outcome<Message> {
            match self.operation.finish() {
                Outcome::Chain(next) => Outcome::Chain(Box::new(ScopedOperation {
                    target: self.target.clone(),
                    current_is_target: false,
                    operation: next,
                })),
                outcome => outcome,
            }
        }
    }

    ScopedOperation {
        target,
        current_is_target: false,
        operation: Box::new(operation),
    }
}

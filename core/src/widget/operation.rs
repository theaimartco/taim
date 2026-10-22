//! Walking the widget tree to query or update widget state.
pub mod focusable;
pub mod scrollable;
pub mod text_input;

mod adapters;

pub use adapters::{black_box, map, scope, then};
pub use focusable::Focusable;
pub use scrollable::Scrollable;
pub use text_input::TextInput;

use crate::widget::Id;
use crate::{Rectangle, Vector};

use std::any::Any;
use std::fmt;

/// A visitor that walks the widget tree to read or change widget state:
/// focus, scroll offsets, text-input cursors, or anything custom.
///
/// Widgets call the visit method matching what they are (a container, a
/// scrollable, a focusable…) and then hand the operation to their children
/// through [`traverse`](Self::traverse). Every visit method does nothing by
/// default, so an operation only implements the visits it cares about.
pub trait Operation<T = ()>: Send {
    /// Visits the children of the current widget by calling `operate` on
    /// this operation. Not calling it prunes the subtree.
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<T>));

    /// Visits a widget that groups other widgets.
    fn container(&mut self, _id: Option<&Id>, _bounds: Rectangle) {}

    /// Visits a widget that scrolls its content by `translation`.
    fn scrollable(
        &mut self,
        _id: Option<&Id>,
        _bounds: Rectangle,
        _content_bounds: Rectangle,
        _translation: Vector,
        _state: &mut dyn Scrollable,
    ) {
    }

    /// Visits a widget that can take keyboard focus.
    fn focusable(&mut self, _id: Option<&Id>, _bounds: Rectangle, _state: &mut dyn Focusable) {}

    /// Visits a widget that edits text.
    fn text_input(&mut self, _id: Option<&Id>, _bounds: Rectangle, _state: &mut dyn TextInput) {}

    /// Visits a widget that displays text.
    fn text(&mut self, _id: Option<&Id>, _bounds: Rectangle, _text: &str) {}

    /// Visits a widget exposing state of its own type.
    fn custom(&mut self, _id: Option<&Id>, _bounds: Rectangle, _state: &mut dyn Any) {}

    /// What the walk produced. Called once the whole tree was visited.
    fn finish(&self) -> Outcome<T> {
        Outcome::None
    }
}

/// Implements every visit method of [`Operation`] (all but `traverse` and
/// `finish`) by forwarding it to `$inner`.
macro_rules! forward_visits {
    ($self:ident => $inner:expr) => {
        fn container(&mut $self, id: Option<&Id>, bounds: Rectangle) {
            $inner.container(id, bounds);
        }

        fn scrollable(
            &mut $self,
            id: Option<&Id>,
            bounds: Rectangle,
            content_bounds: Rectangle,
            translation: Vector,
            state: &mut dyn Scrollable,
        ) {
            $inner.scrollable(id, bounds, content_bounds, translation, state);
        }

        fn focusable(&mut $self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn Focusable) {
            $inner.focusable(id, bounds, state);
        }

        fn text_input(&mut $self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn TextInput) {
            $inner.text_input(id, bounds, state);
        }

        fn text(&mut $self, id: Option<&Id>, bounds: Rectangle, text: &str) {
            $inner.text(id, bounds, text);
        }

        fn custom(&mut $self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn Any) {
            $inner.custom(id, bounds, state);
        }
    };
}

use forward_visits;

impl<T, O> Operation<O> for Box<T>
where
    T: Operation<O> + ?Sized,
{
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<O>)) {
        (**self).traverse(operate);
    }

    forward_visits!(self => (**self));

    fn finish(&self) -> Outcome<O> {
        (**self).finish()
    }
}

/// What an [`Operation`] produced.
pub enum Outcome<T> {
    /// Nothing.
    None,
    /// A value.
    Some(T),
    /// Another operation that must walk the tree next.
    Chain(Box<dyn Operation<T>>),
}

impl<T: fmt::Debug> fmt::Debug for Outcome<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.write_str("Outcome::None"),
            Self::Some(value) => f.debug_tuple("Outcome::Some").field(value).finish(),
            Self::Chain(_) => f.write_str("Outcome::Chain(..)"),
        }
    }
}

//! Query or update internal widget state.
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

/// A piece of logic that can traverse the widget tree of an application in
/// order to query or update some widget state.
pub trait Operation<T = ()>: Send {
    /// Requests further traversal of the widget tree to keep operating.
    ///
    /// The provided `operate` closure may be called by an [`Operation`]
    /// to return control to the widget tree and keep traversing it. If
    /// the closure is not called, the children of the widget asking for
    /// traversal will be skipped.
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<T>));

    /// Operates on a widget that contains other widgets.
    fn container(&mut self, _id: Option<&Id>, _bounds: Rectangle) {}

    /// Operates on a widget that can be scrolled.
    fn scrollable(
        &mut self,
        _id: Option<&Id>,
        _bounds: Rectangle,
        _content_bounds: Rectangle,
        _translation: Vector,
        _state: &mut dyn Scrollable,
    ) {
    }

    /// Operates on a widget that can be focused.
    fn focusable(&mut self, _id: Option<&Id>, _bounds: Rectangle, _state: &mut dyn Focusable) {}

    /// Operates on a widget that has text input.
    fn text_input(&mut self, _id: Option<&Id>, _bounds: Rectangle, _state: &mut dyn TextInput) {}

    /// Operates on a widget that contains some text.
    fn text(&mut self, _id: Option<&Id>, _bounds: Rectangle, _text: &str) {}

    /// Operates on a custom widget with some state.
    fn custom(&mut self, _id: Option<&Id>, _bounds: Rectangle, _state: &mut dyn Any) {}

    /// Finishes the [`Operation`] and returns its [`Outcome`].
    fn finish(&self) -> Outcome<T> {
        Outcome::None
    }
}

impl<T, O> Operation<O> for Box<T>
where
    T: Operation<O> + ?Sized,
{
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<O>)) {
        self.as_mut().traverse(operate);
    }

    fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
        self.as_mut().container(id, bounds);
    }

    fn focusable(&mut self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn Focusable) {
        self.as_mut().focusable(id, bounds, state);
    }

    fn scrollable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        content_bounds: Rectangle,
        translation: Vector,
        state: &mut dyn Scrollable,
    ) {
        self.as_mut()
            .scrollable(id, bounds, content_bounds, translation, state);
    }

    fn text_input(&mut self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn TextInput) {
        self.as_mut().text_input(id, bounds, state);
    }

    fn text(&mut self, id: Option<&Id>, bounds: Rectangle, text: &str) {
        self.as_mut().text(id, bounds, text);
    }

    fn custom(&mut self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn Any) {
        self.as_mut().custom(id, bounds, state);
    }

    fn finish(&self) -> Outcome<O> {
        self.as_ref().finish()
    }
}

/// The result of an [`Operation`].
pub enum Outcome<T> {
    /// The [`Operation`] produced no result.
    None,

    /// The [`Operation`] produced some result.
    Some(T),

    /// The [`Operation`] needs to be followed by another [`Operation`].
    Chain(Box<dyn Operation<T>>),
}

impl<T> fmt::Debug for Outcome<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "Outcome::None"),
            Self::Some(output) => write!(f, "Outcome::Some({output:?})"),
            Self::Chain(_) => write!(f, "Outcome::Chain(...)"),
        }
    }
}

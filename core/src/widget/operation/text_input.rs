//! Operate on widgets that have text input.
use crate::Rectangle;
use crate::widget::Id;
use crate::widget::operation::Operation;

/// The internal state of a widget that has text input.
pub trait TextInput {
    /// Returns the current _visible_ text of the text input
    ///
    /// Normally, this is either its value or its placeholder.
    fn text(&self) -> &str;

    /// Moves the cursor of the text input to the front of the input text.
    fn move_cursor_to_front(&mut self);

    /// Moves the cursor of the text input to the end of the input text.
    fn move_cursor_to_end(&mut self);

    /// Moves the cursor of the text input to an arbitrary location.
    fn move_cursor_to(&mut self, position: usize);

    /// Selects all the content of the text input.
    fn select_all(&mut self);
    /// Selects the given content range of the text input.
    fn select_range(&mut self, start: usize, end: usize);
}

/// Produces an [`Operation`] that moves the cursor of the widget with the given [`Id`] to the
/// front.
pub fn move_cursor_to_front<T>(target: Id) -> impl Operation<T> {
    TargetEdit::new(target, Edit::MoveFront)
}

/// Produces an [`Operation`] that moves the cursor of the widget with the given [`Id`] to the
/// end.
pub fn move_cursor_to_end<T>(target: Id) -> impl Operation<T> {
    TargetEdit::new(target, Edit::MoveEnd)
}

/// Produces an [`Operation`] that moves the cursor of the widget with the given [`Id`] to the
/// provided position.
pub fn move_cursor_to<T>(target: Id, position: usize) -> impl Operation<T> {
    TargetEdit::new(target, Edit::MoveTo(position))
}

/// Produces an [`Operation`] that selects all the content of the widget with the given [`Id`].
pub fn select_all<T>(target: Id) -> impl Operation<T> {
    TargetEdit::new(target, Edit::SelectAll)
}

/// Produces an [`Operation`] that selects the given content range of the widget with the given [`Id`].
pub fn select_range<T>(target: Id, start: usize, end: usize) -> impl Operation<T> {
    TargetEdit::new(target, Edit::SelectRange { start, end })
}

#[derive(Clone, Copy)]
enum Edit {
    MoveFront,
    MoveEnd,
    MoveTo(usize),
    SelectAll,
    SelectRange { start: usize, end: usize },
}

/// Compact operation used by every programmatic text-field edit.
/// A single dispatch body keeps command traversal small and predictable.
struct TargetEdit {
    target: Id,
    edit: Edit,
}

impl TargetEdit {
    fn new(target: Id, edit: Edit) -> Self {
        Self { target, edit }
    }
}

impl<T> Operation<T> for TargetEdit {
    fn text_input(&mut self, id: Option<&Id>, _bounds: Rectangle, state: &mut dyn TextInput) {
        if id != Some(&self.target) {
            return;
        }

        match self.edit {
            Edit::MoveFront => state.move_cursor_to_front(),
            Edit::MoveEnd => state.move_cursor_to_end(),
            Edit::MoveTo(position) => state.move_cursor_to(position),
            Edit::SelectAll => state.select_all(),
            Edit::SelectRange { start, end } => state.select_range(start, end),
        }
    }

    fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation<T>)) {
        visit(self);
    }
}

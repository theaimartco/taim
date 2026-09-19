//! Track the cursor of a text input.
use crate::text_input::Value;

/// The cursor of a text input.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Cursor {
    anchor: usize,
    head: usize,
}

/// The state of a [`Cursor`].
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum State {
    /// Cursor without a selection
    Index(usize),

    /// Cursor selecting a range of text
    Selection {
        /// The start of the selection
        start: usize,
        /// The end of the selection
        end: usize,
    },
}

impl Default for Cursor {
    fn default() -> Self {
        Self { anchor: 0, head: 0 }
    }
}

impl Cursor {
    /// Returns the [`State`] of the [`Cursor`].
    pub fn state(&self, value: &Value) -> State {
        let length = value.len();
        let anchor = self.anchor.min(length);
        let head = self.head.min(length);

        if anchor == head {
            State::Index(head)
        } else {
            State::Selection {
                start: anchor,
                end: head,
            }
        }
    }

    /// Returns the current selection of the [`Cursor`] for the given [`Value`].
    ///
    /// `start` is guaranteed to be <= than `end`.
    pub fn selection(&self, value: &Value) -> Option<(usize, usize)> {
        match self.state(value) {
            State::Selection { start, end } => Some((start.min(end), start.max(end))),
            State::Index(_) => None,
        }
    }

    pub(crate) fn move_to(&mut self, position: usize) {
        self.anchor = position;
        self.head = position;
    }

    pub(crate) fn move_right(&mut self, value: &Value) {
        self.move_right_by_amount(value, 1);
    }

    pub(crate) fn move_right_by_words(&mut self, value: &Value) {
        self.move_to(value.next_end_of_word(self.right(value)));
    }

    pub(crate) fn move_right_by_amount(&mut self, value: &Value, amount: usize) {
        match self.state(value) {
            State::Index(index) => {
                self.move_to(index.saturating_add(amount).min(value.len()));
            }
            State::Selection { start, end } => self.move_to(end.max(start)),
        }
    }

    pub(crate) fn move_left(&mut self, value: &Value) {
        match self.state(value) {
            State::Index(index) if index > 0 => self.move_to(index - 1),
            State::Selection { start, end } => self.move_to(start.min(end)),
            State::Index(_) => self.move_to(0),
        }
    }

    pub(crate) fn move_left_by_words(&mut self, value: &Value) {
        self.move_to(value.previous_start_of_word(self.left(value)));
    }

    pub(crate) fn select_range(&mut self, start: usize, end: usize) {
        self.anchor = start;
        self.head = end;
    }

    pub(crate) fn select_left(&mut self, value: &Value) {
        match self.state(value) {
            State::Index(index) if index > 0 => {
                self.select_range(index, index - 1);
            }
            State::Selection { start, end } if end > 0 => {
                self.select_range(start, end - 1);
            }
            _ => {}
        }
    }

    pub(crate) fn select_right(&mut self, value: &Value) {
        match self.state(value) {
            State::Index(index) if index < value.len() => {
                self.select_range(index, index + 1);
            }
            State::Selection { start, end } if end < value.len() => {
                self.select_range(start, end + 1);
            }
            _ => {}
        }
    }

    pub(crate) fn select_left_by_words(&mut self, value: &Value) {
        match self.state(value) {
            State::Index(index) => {
                self.select_range(index, value.previous_start_of_word(index));
            }
            State::Selection { start, end } => {
                self.select_range(start, value.previous_start_of_word(end));
            }
        }
    }

    pub(crate) fn select_right_by_words(&mut self, value: &Value) {
        match self.state(value) {
            State::Index(index) => {
                self.select_range(index, value.next_end_of_word(index));
            }
            State::Selection { start, end } => {
                self.select_range(start, value.next_end_of_word(end));
            }
        }
    }

    pub(crate) fn select_all(&mut self, value: &Value) {
        self.select_range(0, value.len());
    }

    pub(crate) fn start(&self, value: &Value) -> usize {
        self.anchor.min(value.len())
    }

    pub(crate) fn end(&self, value: &Value) -> usize {
        self.head.min(value.len())
    }

    fn left(&self, value: &Value) -> usize {
        match self.state(value) {
            State::Index(index) => index,
            State::Selection { start, end } => start.min(end),
        }
    }

    fn right(&self, value: &Value) -> usize {
        match self.state(value) {
            State::Index(index) => index,
            State::Selection { start, end } => start.max(end),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_cursor_is_two_machine_words() {
        assert_eq!(
            std::mem::size_of::<Cursor>(),
            2 * std::mem::size_of::<usize>()
        );
    }

    #[test]
    fn selection_direction_and_clamping_are_preserved() {
        let value = Value::new("abc");
        let mut cursor = Cursor::default();
        cursor.select_range(9, 1);

        assert_eq!(cursor.state(&value), State::Selection { start: 3, end: 1 });
        assert_eq!(cursor.selection(&value), Some((1, 3)));
    }
}

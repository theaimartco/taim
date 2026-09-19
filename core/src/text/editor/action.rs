//! Editor commands and navigation intent.

use crate::Point;

use std::sync::Arc;

/// An interaction with a text editor.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Move the caret.
    Move(Motion),
    /// Extend a selection.
    Select(Motion),
    /// Select the word at the caret.
    SelectWord,
    /// Select the line at the caret.
    SelectLine,
    /// Select the whole buffer.
    SelectAll,
    /// Mutate the buffer.
    Edit(Edit),
    /// Place the caret at a point.
    Click(Point),
    /// Extend the selection to a point.
    Drag(Point),
    /// Scroll by logical lines.
    Scroll {
        /// Signed line delta.
        lines: i32,
    },
}

impl Action {
    /// Returns whether this action can mutate buffer contents.
    #[inline]
    pub const fn is_edit(&self) -> bool {
        matches!(self, Self::Edit(_))
    }
}

/// A buffer mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// Insert one character.
    Insert(char),
    /// Insert shared clipboard text without another payload copy.
    Paste(Arc<String>),
    /// Break the current line.
    Enter,
    /// Indent the current line.
    Indent,
    /// Unindent the current line.
    Unindent,
    /// Delete the previous character.
    Backspace,
    /// Delete the next character.
    Delete,
}

/// A caret movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Motion {
    /// Move left.
    Left,
    /// Move right.
    Right,
    /// Move up.
    Up,
    /// Move down.
    Down,
    /// Move to the previous word boundary.
    WordLeft,
    /// Move to the next word boundary.
    WordRight,
    /// Move to the line start.
    Home,
    /// Move to the line end.
    End,
    /// Move one viewport up.
    PageUp,
    /// Move one viewport down.
    PageDown,
    /// Move to the buffer start.
    DocumentStart,
    /// Move to the buffer end.
    DocumentEnd,
}

impl Motion {
    /// Widens a character/line-boundary movement to its semantic boundary.
    #[inline]
    pub const fn widen(self) -> Self {
        match self {
            Self::Left => Self::WordLeft,
            Self::Right => Self::WordRight,
            Self::Home => Self::DocumentStart,
            Self::End => Self::DocumentEnd,
            other => other,
        }
    }

    /// Returns the direction in which a collapsed selection should resolve.
    #[inline]
    pub const fn direction(self) -> Direction {
        match self {
            Self::Left
            | Self::Up
            | Self::WordLeft
            | Self::Home
            | Self::PageUp
            | Self::DocumentStart => Direction::Left,
            Self::Right
            | Self::Down
            | Self::WordRight
            | Self::End
            | Self::PageDown
            | Self::DocumentEnd => Direction::Right,
        }
    }
}

/// Horizontal direction of a text movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Toward the logical start.
    Left,
    /// Toward the logical end.
    Right,
}

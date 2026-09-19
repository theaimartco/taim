//! Caret and selection models optimized for retained rendering.

use crate::{Point, Rectangle};

use std::sync::Arc;

/// Visual selection geometry.
#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    /// A collapsed caret.
    Caret(Point),
    /// Immutable rectangles shared by the editor state and render frames.
    Range(Arc<[Rectangle]>),
}

impl Selection {
    /// Returns whether the selection is a collapsed caret.
    #[inline]
    pub const fn is_caret(&self) -> bool {
        matches!(self, Self::Caret(_))
    }

    /// Returns selection rectangles, or an empty slice for a caret.
    #[inline]
    pub fn ranges(&self) -> &[Rectangle] {
        match self {
            Self::Caret(_) => &[],
            Self::Range(ranges) => ranges,
        }
    }
}

/// Logical caret plus an optional selection anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cursor {
    /// Active caret position.
    pub position: Position,
    /// Selection anchor, when the range is expanded.
    pub selection: Option<Position>,
}

impl Cursor {
    /// Creates a collapsed cursor.
    #[inline]
    pub const fn new(position: Position) -> Self {
        Self {
            position,
            selection: None,
        }
    }
}

/// A byte-column position within a logical line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    /// Zero-based line index.
    pub line: usize,
    /// Zero-based byte column.
    pub column: usize,
}

impl Position {
    /// Creates a logical editor position.
    #[inline]
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

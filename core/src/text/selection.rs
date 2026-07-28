//! Logical selections and renderer-neutral selection geometry.

use crate::Rectangle;

use std::ops::Range;

/// A UTF-8 byte range whose endpoints are known character boundaries.
///
/// Text engines keep their native cursor representation internally, but this
/// value is the portable boundary between editing, IME, semantics, and
/// rendering. Construction validates offsets so malformed platform events
/// cannot split a code point.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Selection {
    range: Range<usize>,
}

impl Selection {
    /// Validates and creates a logical selection in `text`.
    pub fn new(text: &str, range: Range<usize>) -> Option<Self> {
        (range.start <= range.end
            && range.end <= text.len()
            && text.is_char_boundary(range.start)
            && text.is_char_boundary(range.end))
        .then_some(Self { range })
    }

    /// Returns the selected UTF-8 byte range.
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    /// Returns whether this is a caret rather than a non-empty range.
    pub fn is_caret(&self) -> bool {
        self.range.start == self.range.end
    }

    /// Clamps a selection to valid character boundaries in replacement text.
    pub fn clamp_to(&self, text: &str) -> Self {
        let boundary_before = |offset: usize| {
            let offset = offset.min(text.len());
            text.char_indices()
                .map(|(index, _)| index)
                .chain(std::iter::once(text.len()))
                .take_while(|index| *index <= offset)
                .last()
                .unwrap_or(0)
        };

        Self {
            range: boundary_before(self.range.start)..boundary_before(self.range.end),
        }
    }
}

/// Visual fragments of one logical selection.
///
/// Bidirectional and wrapped selections may occupy several disjoint visual
/// rectangles. The order is visual (paint order), while [`Self::logical`] is
/// retained for editing and accessibility operations.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionGeometry {
    /// Source logical range.
    pub logical: Selection,
    /// Visual rectangles in paint order.
    pub rectangles: Vec<Rectangle>,
}

impl SelectionGeometry {
    /// Creates geometry for a logical selection.
    pub fn new(logical: Selection, rectangles: Vec<Rectangle>) -> Self {
        Self {
            logical,
            rectangles,
        }
    }
}

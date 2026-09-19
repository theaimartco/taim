use std::ops::Range;

/// A platform text-composition event.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Event {
    /// Composition became active.
    Opened,
    /// Intermediate UTF-8 text and its optional byte selection.
    Preedit(String, Option<Range<usize>>),
    /// Final text to insert into the editor.
    Commit(String),
    /// Composition ended and pending preedit text should be cleared.
    Closed,
}

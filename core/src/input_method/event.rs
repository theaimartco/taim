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
    /// Complete native editor state. Composing text stays uncommitted until
    /// the platform clears its composing range.
    Edit(super::EditorState),
    /// Deletes text around the current selection without exporting context.
    /// UTF-16 counts round outward at surrogate boundaries; codepoint counts
    /// delete Unicode scalar values. The selected text itself is preserved.
    DeleteSurrounding {
        /// Units before the selection/caret.
        before: usize,
        /// Units after the selection/caret.
        after: usize,
        /// Counts Unicode scalars instead of UTF-16 units.
        codepoints: bool,
    },
    /// The platform editor action (for example, Done) was invoked.
    Submit,
    /// Composition ended and pending preedit text should be cleared.
    Closed,
}

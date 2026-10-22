//! Editable multi-line text.
mod action;
mod line;
mod selection;

pub use action::{Action, Direction, Edit, Motion};
pub use line::{Line, LineEnding};
pub use selection::{Cursor, Position, Selection};

use crate::text::highlighter::{self, Highlighter};
use crate::text::{LineHeight, Wrapping};
use crate::{Pixels, Size};

/// A multi-line text buffer with a cursor, as used by the text editor
/// widget. Renderers provide the implementation.
pub trait Editor: Sized + Default {
    /// The renderer's font handle.
    type Font: Copy + PartialEq + Default;

    /// A buffer holding `text`, cursor at the start.
    fn with_text(text: &str) -> Self;

    /// Whether the buffer holds no text.
    fn is_empty(&self) -> bool;

    /// Where the caret is, and the selection anchor if any.
    fn cursor(&self) -> Cursor;

    /// The selected range, as geometry for drawing.
    fn selection(&self) -> Selection;

    /// The selected text, if anything is selected.
    fn copy(&self) -> Option<String>;

    /// Line `index`, if the buffer has that many lines.
    fn line(&self, index: usize) -> Option<Line<'_>>;

    /// Number of lines.
    fn line_count(&self) -> usize;

    /// The first visible line and how far it is scrolled, in pixels.
    fn scroll_position(&self) -> (usize, f32) {
        (0, 0.0)
    }

    /// Applies an edit, a motion, a selection change or a scroll.
    fn perform(&mut self, action: Action);

    /// Places the caret (and selection) at `cursor`.
    fn move_to(&mut self, cursor: Cursor);

    /// The box the text is laid out in.
    fn bounds(&self) -> Size;

    /// The smallest box the text fits in.
    fn min_bounds(&self) -> Size;

    /// Metrics-hinting scale, if hinted.
    fn hint_factor(&self) -> Option<f32>;

    /// Applies new layout and style settings, re-laying out only what they
    /// affect.
    #[allow(clippy::too_many_arguments)]
    fn update(
        &mut self,
        bounds: Size,
        font: Self::Font,
        size: Pixels,
        line_height: LineHeight,
        wrapping: Wrapping,
        hint_factor: Option<f32>,
        highlighter: &mut impl Highlighter,
    );

    /// Colors the text with `highlighter`, turning each highlight into a
    /// font and color with `format`.
    fn highlight<H: Highlighter>(
        &mut self,
        font: Self::Font,
        highlighter: &mut H,
        format: impl Fn(&H::Highlight) -> highlighter::Format<Self::Font>,
    );
}

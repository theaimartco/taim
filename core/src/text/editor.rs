//! Edit text.
mod action;
mod line;
mod selection;

pub use action::{Action, Direction, Edit, Motion};
pub use line::{Line, LineEnding};
pub use selection::{Cursor, Position, Selection};

use crate::text::highlighter::{self, Highlighter};
use crate::text::{LineHeight, Wrapping};
use crate::{Pixels, Size};

/// A component that can be used by widgets to edit multi-line text.
pub trait Editor: Sized + Default {
    /// The font of the [`Editor`].
    type Font: Copy + PartialEq + Default;

    /// Creates a new [`Editor`] laid out with the given text.
    fn with_text(text: &str) -> Self;

    /// Returns true if the [`Editor`] has no contents.
    fn is_empty(&self) -> bool;

    /// Returns the current [`Cursor`] of the [`Editor`].
    fn cursor(&self) -> Cursor;

    /// Returns the current [`Selection`] of the [`Editor`].
    fn selection(&self) -> Selection;

    /// Returns the current selected text of the [`Editor`].
    fn copy(&self) -> Option<String>;

    /// Returns the text of the given line in the [`Editor`], if it exists.
    fn line(&self, index: usize) -> Option<Line<'_>>;

    /// Returns the amount of lines in the [`Editor`].
    fn line_count(&self) -> usize;

    /// Performs an [`Action`] on the [`Editor`].
    fn perform(&mut self, action: Action);

    /// Moves the cursor to the given position.
    fn move_to(&mut self, cursor: Cursor);

    /// Returns the current boundaries of the [`Editor`].
    fn bounds(&self) -> Size;

    /// Returns the minimum boundaries to fit the current contents of
    /// the [`Editor`].
    fn min_bounds(&self) -> Size;

    /// Returns the hint factor of the [`Editor`].
    fn hint_factor(&self) -> Option<f32>;

    /// Updates the [`Editor`] with some new attributes.
    fn update(
        &mut self,
        new_bounds: Size,
        new_font: Self::Font,
        new_size: Pixels,
        new_line_height: LineHeight,
        new_wrapping: Wrapping,
        new_hint_factor: Option<f32>,
        new_highlighter: &mut impl Highlighter,
    );

    /// Runs a text [`Highlighter`] in the [`Editor`].
    fn highlight<H: Highlighter>(
        &mut self,
        font: Self::Font,
        highlighter: &mut H,
        format_highlight: impl Fn(&H::Highlight) -> highlighter::Format<Self::Font>,
    );
}

//! Shaped, laid-out text.
use crate::alignment;
use crate::text::{
    Alignment, Difference, Ellipsis, Hit, LineHeight, Shaping, Span, Text, Wrapping,
};
use crate::{Pixels, Point, Rectangle, Size};

mod cached;
mod shaping_key;

pub use cached::Plain;
pub use shaping_key::{Direction, ShapingKey};

/// A shaped, laid-out paragraph a text [`Renderer`](super::Renderer) can
/// draw and query.
///
/// Paragraphs are expensive to build and cheap to keep: widgets store one
/// and use [`compare`](Self::compare) to decide whether it can be reused,
/// merely [`resize`](Self::resize)d, or must be rebuilt.
pub trait Paragraph: Sized + Default {
    /// The renderer's font handle.
    type Font: Copy + PartialEq;

    // Building

    /// Shapes and lays out plain text.
    fn with_text(text: Text<&str, Self::Font>) -> Self;

    /// Shapes and lays out rich text.
    fn with_spans<Link>(text: Text<&[Span<'_, Link, Self::Font>], Self::Font>) -> Self;

    /// Lays the already-shaped glyphs out again in `bounds`.
    fn resize(&mut self, bounds: Size);

    /// How much of this paragraph survives if the settings change to `text`.
    fn compare(&self, text: Text<(), Self::Font>) -> Difference;

    // The settings it was built with

    /// Font size.
    fn size(&self) -> Pixels;

    /// Metrics-hinting scale, if hinted.
    fn hint_factor(&self) -> Option<f32>;

    /// Default font.
    fn font(&self) -> Self::Font;

    /// Distance between baselines.
    fn line_height(&self) -> LineHeight;

    /// Horizontal line placement.
    fn align_x(&self) -> Alignment;

    /// Vertical block placement.
    fn align_y(&self) -> alignment::Vertical;

    /// Line-breaking policy.
    fn wrapping(&self) -> Wrapping;

    /// Elision policy.
    fn ellipsis(&self) -> Ellipsis;

    /// Shaping level.
    fn shaping(&self) -> Shaping;

    /// The box the paragraph was laid out in.
    fn bounds(&self) -> Size;

    // Measuring and querying

    /// The smallest box the laid-out text fits in.
    fn min_bounds(&self) -> Size;

    /// The character nearest to `point`, if `point` is inside the paragraph.
    fn hit_test(&self, point: Point) -> Option<Hit>;

    /// Which span `point` is over, by index.
    fn hit_span(&self, point: Point) -> Option<usize>;

    /// The boxes covering span `index`: one per line it spans.
    fn span_bounds(&self, index: usize) -> Vec<Rectangle>;

    /// Where grapheme `grapheme` of line `line` starts.
    fn grapheme_position(&self, line: usize, grapheme: usize) -> Option<Point>;

    /// Width of [`min_bounds`](Self::min_bounds).
    fn min_width(&self) -> f32 {
        self.min_bounds().width
    }

    /// Height of [`min_bounds`](Self::min_bounds).
    fn min_height(&self) -> f32 {
        self.min_bounds().height
    }
}

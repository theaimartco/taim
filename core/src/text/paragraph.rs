//! Draw paragraphs.
use crate::alignment;
use crate::text::{
    Alignment, Difference, Ellipsis, Hit, LineHeight, Shaping, Span, Text, Wrapping,
};
use crate::{Pixels, Point, Rectangle, Size};

mod cached;
mod shaping_key;

pub use cached::Plain;
pub use shaping_key::{Direction, ShapingKey};

/// A text paragraph.
pub trait Paragraph: Sized + Default {
    /// The font of this [`Paragraph`].
    type Font: Copy + PartialEq;

    /// Creates a new [`Paragraph`] laid out with the given [`Text`].
    fn with_text(text: Text<&str, Self::Font>) -> Self;

    /// Creates a new [`Paragraph`] laid out with the given [`Text`].
    fn with_spans<Link>(text: Text<&[Span<'_, Link, Self::Font>], Self::Font>) -> Self;

    /// Lays out the [`Paragraph`] with some new boundaries.
    fn resize(&mut self, new_bounds: Size);

    /// Compares the [`Paragraph`] with some desired [`Text`] and returns the
    /// [`Difference`].
    fn compare(&self, text: Text<(), Self::Font>) -> Difference;

    /// Returns the text size of the [`Paragraph`] in [`Pixels`].
    fn size(&self) -> Pixels;

    /// Returns the hint factor of the [`Paragraph`].
    fn hint_factor(&self) -> Option<f32>;

    /// Returns the font of the [`Paragraph`].
    fn font(&self) -> Self::Font;

    /// Returns the [`LineHeight`] of the [`Paragraph`].
    fn line_height(&self) -> LineHeight;

    /// Returns the horizontal alignment of the [`Paragraph`].
    fn align_x(&self) -> Alignment;

    /// Returns the vertical alignment of the [`Paragraph`].
    fn align_y(&self) -> alignment::Vertical;

    /// Returns the [`Wrapping`] strategy of the [`Paragraph`]>
    fn wrapping(&self) -> Wrapping;

    /// Returns the [`Ellipsis`] strategy of the [`Paragraph`]>
    fn ellipsis(&self) -> Ellipsis;

    /// Returns the [`Shaping`] strategy of the [`Paragraph`]>
    fn shaping(&self) -> Shaping;

    /// Returns the available bounds used to layout the [`Paragraph`].
    fn bounds(&self) -> Size;

    /// Returns the minimum boundaries that can fit the contents of the
    /// [`Paragraph`].
    fn min_bounds(&self) -> Size;

    /// Tests whether the provided point is within the boundaries of the
    /// [`Paragraph`], returning information about the nearest character.
    fn hit_test(&self, point: Point) -> Option<Hit>;

    /// Tests whether the provided point is within the boundaries of a
    /// [`Span`] in the [`Paragraph`], returning the index of the [`Span`]
    /// that was hit.
    fn hit_span(&self, point: Point) -> Option<usize>;

    /// Returns all bounds for the provided [`Span`] index of the [`Paragraph`].
    /// A [`Span`] can have multiple bounds for each line it's on.
    fn span_bounds(&self, index: usize) -> Vec<Rectangle>;

    /// Returns the distance to the given grapheme index in the [`Paragraph`].
    fn grapheme_position(&self, line: usize, index: usize) -> Option<Point>;

    /// Returns the minimum width that can fit the contents of the [`Paragraph`].
    fn min_width(&self) -> f32 {
        self.min_bounds().width
    }

    /// Returns the minimum height that can fit the contents of the [`Paragraph`].
    fn min_height(&self) -> f32 {
        self.min_bounds().height
    }
}

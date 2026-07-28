//! Draw paragraphs.
use crate::alignment;
use crate::text::{
    Alignment, Difference, Ellipsis, Hit, LineHeight, Shaping, Span, Text, Wrapping,
};
use crate::text::{FallbackChain, Script};
use crate::{Pixels, Point, Rectangle, Size};

use std::hash::{Hash, Hasher};

/// The base direction used to resolve a paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Direction {
    /// Infer from the paragraph's first strong character.
    #[default]
    Auto,
    /// Resolve the paragraph left-to-right.
    LeftToRight,
    /// Resolve the paragraph right-to-left.
    RightToLeft,
}

/// Shaping attributes that must participate in paragraph cache identity.
///
/// This deliberately keeps locale, direction, fallback and variable-font axis
/// identity together. A renderer can translate it to backend-specific font
/// attributes without accidentally reusing a run shaped for incompatible
/// inputs.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShapingKey {
    /// BCP-47 locale when the caller knows it.
    pub locale: Option<String>,
    /// Paragraph base direction.
    pub direction: Direction,
    /// Script chosen by application policy or detection.
    pub script: Script,
    /// Ordered fallback family chain.
    pub fallback: FallbackChain,
    /// Quantized variable-font axes, sorted by four-byte axis tag.
    pub axes: Vec<([u8; 4], u32)>,
}

impl Default for ShapingKey {
    fn default() -> Self {
        Self {
            locale: None,
            direction: Direction::Auto,
            script: Script::Latin,
            fallback: FallbackChain::for_script(Script::Latin),
            axes: Vec::new(),
        }
    }
}

impl ShapingKey {
    /// Creates a default key after deterministic script detection.
    pub fn for_text(text: &str) -> Self {
        let script = text
            .chars()
            .map(Script::detect)
            .find(|script| !matches!(script, Script::Latin | Script::Other))
            .unwrap_or(Script::Latin);

        Self {
            script,
            fallback: FallbackChain::for_script(script),
            ..Self::default()
        }
    }

    /// Sets a BCP-47 locale.
    pub fn locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = Some(locale.into());
        self
    }

    /// Sets the paragraph direction.
    pub fn direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    /// Adds or replaces a quantized variable-font axis.
    pub fn axis(mut self, tag: [u8; 4], value: f32) -> Self {
        let value = value.to_bits();
        if let Some(axis) = self.axes.iter_mut().find(|axis| axis.0 == tag) {
            axis.1 = value;
        } else {
            self.axes.push((tag, value));
            self.axes.sort_unstable_by_key(|axis| axis.0);
        }
        self
    }

    /// Hashes the full shaping identity into a caller-provided hasher.
    pub fn hash_into(&self, state: &mut impl Hasher) {
        self.hash(state);
    }
}

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

/// A [`Paragraph`] of plain text.
#[derive(Debug, Clone, Default)]
pub struct Plain<P: Paragraph> {
    raw: P,
    content: String,
}

impl<P: Paragraph> Plain<P> {
    /// Creates a new [`Plain`] paragraph.
    pub fn new(text: Text<String, P::Font>) -> Self {
        Self {
            raw: P::with_text(text.as_ref()),
            content: text.content,
        }
    }

    /// Updates the plain [`Paragraph`] to match the given [`Text`], if needed.
    ///
    /// Returns true if the [`Paragraph`] changed.
    pub fn update(&mut self, text: Text<&str, P::Font>) -> bool {
        if self.content != text.content {
            text.content.clone_into(&mut self.content);
            self.raw = P::with_text(text);
            return true;
        }

        match self.raw.compare(text.with_content(())) {
            Difference::None => false,
            Difference::Bounds => {
                self.raw.resize(text.bounds);
                true
            }
            Difference::Shape => {
                self.raw = P::with_text(text);
                true
            }
        }
    }

    /// Returns the horizontal alignment of the [`Paragraph`].
    pub fn align_x(&self) -> Alignment {
        self.raw.align_x()
    }

    /// Returns the vertical alignment of the [`Paragraph`].
    pub fn align_y(&self) -> alignment::Vertical {
        self.raw.align_y()
    }

    /// Returns the minimum boundaries that can fit the contents of the
    /// [`Paragraph`].
    pub fn min_bounds(&self) -> Size {
        self.raw.min_bounds()
    }

    /// Returns the minimum width that can fit the contents of the
    /// [`Paragraph`].
    pub fn min_width(&self) -> f32 {
        self.raw.min_width()
    }

    /// Returns the minimum height that can fit the contents of the
    /// [`Paragraph`].
    pub fn min_height(&self) -> f32 {
        self.raw.min_height()
    }

    /// Returns the cached [`Paragraph`].
    pub fn raw(&self) -> &P {
        &self.raw
    }

    /// Returns the current content of the plain [`Paragraph`].
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Returns the [`Paragraph`] as a [`Text`] definition.
    pub fn as_text(&self) -> Text<&str, P::Font> {
        Text {
            content: &self.content,
            bounds: self.raw.bounds(),
            size: self.raw.size(),
            line_height: self.raw.line_height(),
            font: self.raw.font(),
            align_x: self.raw.align_x(),
            align_y: self.raw.align_y(),
            shaping: self.raw.shaping(),
            wrapping: self.raw.wrapping(),
            ellipsis: self.raw.ellipsis(),
            hint_factor: self.raw.hint_factor(),
        }
    }
}

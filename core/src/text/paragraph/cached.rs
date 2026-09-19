use super::Paragraph;
use crate::Size;
use crate::alignment;
use crate::text::{Alignment, Difference, Text};

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

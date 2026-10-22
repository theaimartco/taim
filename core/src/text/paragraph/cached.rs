//! A plain-text paragraph that rebuilds itself only when it has to.
use super::Paragraph;
use crate::Size;
use crate::alignment;
use crate::text::{Alignment, Difference, Text};

/// A plain-text [`Paragraph`] that remembers its content, so callers can
/// hand it the desired [`Text`] every frame and pay only for real changes.
#[derive(Debug, Clone, Default)]
pub struct Plain<P: Paragraph> {
    shaped: P,
    content: String,
}

impl<P: Paragraph> Plain<P> {
    /// Shapes `text`.
    pub fn new(text: Text<String, P::Font>) -> Self {
        Self {
            shaped: P::with_text(text.as_ref()),
            content: text.content,
        }
    }

    /// Brings the paragraph in line with `text`, doing as little work as
    /// possible: nothing, a re-layout, or a full reshape.
    ///
    /// Returns whether anything changed.
    pub fn update(&mut self, text: Text<&str, P::Font>) -> bool {
        let difference = if self.content == text.content {
            self.shaped.compare(text.with_content(()))
        } else {
            text.content.clone_into(&mut self.content);
            Difference::Shape
        };

        match difference {
            Difference::None => return false,
            Difference::Bounds => self.shaped.resize(text.bounds),
            Difference::Shape => self.shaped = P::with_text(text),
        }

        true
    }

    /// Horizontal line placement.
    pub fn align_x(&self) -> Alignment {
        self.shaped.align_x()
    }

    /// Vertical block placement.
    pub fn align_y(&self) -> alignment::Vertical {
        self.shaped.align_y()
    }

    /// The smallest box the text fits in.
    pub fn min_bounds(&self) -> Size {
        self.shaped.min_bounds()
    }

    /// Width of [`min_bounds`](Self::min_bounds).
    pub fn min_width(&self) -> f32 {
        self.shaped.min_width()
    }

    /// Height of [`min_bounds`](Self::min_bounds).
    pub fn min_height(&self) -> f32 {
        self.shaped.min_height()
    }

    /// The underlying shaped paragraph.
    pub fn raw(&self) -> &P {
        &self.shaped
    }

    /// The text currently shaped.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// The settings and content this paragraph was built from.
    pub fn as_text(&self) -> Text<&str, P::Font> {
        let shaped = &self.shaped;

        Text {
            content: self.content.as_str(),
            bounds: shaped.bounds(),
            size: shaped.size(),
            line_height: shaped.line_height(),
            font: shaped.font(),
            align_x: shaped.align_x(),
            align_y: shaped.align_y(),
            shaping: shaped.shaping(),
            wrapping: shaped.wrapping(),
            ellipsis: shaped.ellipsis(),
            hint_factor: shaped.hint_factor(),
        }
    }
}

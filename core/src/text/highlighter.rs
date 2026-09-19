//! Highlight text.
use crate::Color;

use std::ops::Range;

/// A type capable of highlighting text.
///
/// A [`Highlighter`] highlights lines in sequence. When a line changes,
/// it must be notified and the lines after the changed one must be fed
/// again to the [`Highlighter`].
pub trait Highlighter: 'static {
    /// The settings to configure the [`Highlighter`].
    type Settings: PartialEq + Clone;

    /// The output of the [`Highlighter`].
    type Highlight;

    /// The highlight iterator type.
    type Iterator<'a>: Iterator<Item = (Range<usize>, Self::Highlight)>
    where
        Self: 'a;

    /// Creates a new [`Highlighter`] from its [`Self::Settings`].
    fn new(settings: &Self::Settings) -> Self;

    /// Updates the [`Highlighter`] with some new [`Self::Settings`].
    fn update(&mut self, new_settings: &Self::Settings);

    /// Notifies the [`Highlighter`] that the line at the given index has changed.
    fn change_line(&mut self, line: usize);

    /// Highlights the given line.
    ///
    /// If a line changed prior to this, the first line provided here will be the
    /// line that changed.
    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_>;

    /// Returns the current line of the [`Highlighter`].
    ///
    /// If `change_line` has been called, this will normally be the least index
    /// that changed.
    fn current_line(&self) -> usize;
}

/// A highlighter that highlights nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlainText;

impl Highlighter for PlainText {
    type Settings = ();
    type Highlight = ();

    type Iterator<'a> = std::iter::Empty<(Range<usize>, Self::Highlight)>;

    fn new(_settings: &Self::Settings) -> Self {
        Self
    }

    fn update(&mut self, _new_settings: &Self::Settings) {}

    fn change_line(&mut self, _line: usize) {}

    fn highlight_line(&mut self, _line: &str) -> Self::Iterator<'_> {
        std::iter::empty()
    }

    fn current_line(&self) -> usize {
        usize::MAX
    }
}

/// The format of some text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Format<Font> {
    /// The [`Color`] of the text.
    pub color: Option<Color>,
    /// The `Font` of the text.
    pub font: Option<Font>,
}

impl<Font> Format<Font> {
    /// Returns whether this highlight changes neither color nor font.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.color.is_none() && self.font.is_none()
    }

    /// Uses the non-empty fields of `top` over this format.
    pub fn overlay(self, top: Self) -> Self {
        Self {
            color: top.color.or(self.color),
            font: top.font.or(self.font),
        }
    }
}

/// Rejects a highlight range that cannot address complete UTF-8 characters.
///
/// Syntax highlighters generally report byte positions. Checking only the
/// line length is insufficient for non-ASCII text: passing a mid-codepoint
/// boundary to a text layout engine can crash an editor during rendering.
#[inline]
pub fn valid_range(line: &str, range: Range<usize>) -> Option<Range<usize>> {
    (range.start < range.end
        && range.end <= line.len()
        && line.is_char_boundary(range.start)
        && line.is_char_boundary(range.end))
    .then_some(range)
}

impl<Font> Default for Format<Font> {
    fn default() -> Self {
        Self {
            color: None,
            font: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_validation_respects_utf8_boundaries() {
        let line = "aéz";
        assert_eq!(valid_range(line, 1..3), Some(1..3));
        assert_eq!(valid_range(line, 1..2), None);
        assert_eq!(valid_range(line, 4..5), None);
        assert_eq!(valid_range(line, 2..2), None);
    }

    #[test]
    fn overlay_keeps_base_attributes_when_top_is_absent() {
        let base = Format {
            color: Some(Color::BLACK),
            font: Some(1_u8),
        };
        let top = Format {
            color: Some(Color::WHITE),
            font: None,
        };

        assert_eq!(base.overlay(top).color, Some(Color::WHITE));
        assert_eq!(base.overlay(top).font, Some(1));
    }
}

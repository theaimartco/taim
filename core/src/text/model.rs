//! The plain-data description of a paragraph and the policies it carries.
use super::*;

/// Everything needed to shape and lay out one paragraph.
///
/// `Content` is usually a string or a slice of [`Span`]s.
#[derive(Debug, Clone, Copy)]
pub struct Text<Content = String, Font = crate::Font> {
    /// What to draw.
    pub content: Content,
    /// The box the paragraph lays out in; wrapping happens at its width.
    pub bounds: Size,
    /// Font size in logical pixels.
    pub size: Pixels,
    /// Distance between baselines.
    pub line_height: LineHeight,
    /// Font used where a span does not choose one.
    pub font: Font,
    /// Horizontal placement of each line inside `bounds`.
    pub align_x: Alignment,
    /// Vertical placement of the block of lines inside `bounds`.
    pub align_y: alignment::Vertical,
    /// How much shaping work to do.
    pub shaping: Shaping,
    /// Where lines may break.
    pub wrapping: Wrapping,
    /// Where to elide text that does not fit.
    pub ellipsis: Ellipsis,
    /// Metrics-hinting scale.
    ///
    /// With `Some(factor)` the paragraph is laid out once at that scale and
    /// scaled linearly from there, which lets the renderer snap glyph metrics
    /// to the pixel grid. With `None` there is no hinting and glyphs are
    /// positioned at subpixel precision.
    pub hint_factor: Option<f32>,
}

impl<Content, Font: Copy> Text<Content, Font> {
    /// The same paragraph settings with different content.
    pub fn with_content<Other>(&self, content: Other) -> Text<Other, Font> {
        let Self {
            content: _,
            bounds,
            size,
            line_height,
            font,
            align_x,
            align_y,
            shaping,
            wrapping,
            ellipsis,
            hint_factor,
        } = *self;

        Text {
            content,
            bounds,
            size,
            line_height,
            font,
            align_x,
            align_y,
            shaping,
            wrapping,
            ellipsis,
            hint_factor,
        }
    }
}

impl<Content: AsRef<str>, Font: Copy> Text<Content, Font> {
    /// Borrows the content as a `&str`.
    pub fn as_ref(&self) -> Text<&str, Font> {
        self.with_content(self.content.as_ref())
    }
}

/// Horizontal placement of lines of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Alignment {
    /// Follow the writing direction: left for left-to-right scripts, right
    /// for right-to-left ones.
    #[default]
    Default,
    /// Flush left.
    Left,
    /// Centered.
    Center,
    /// Flush right.
    Right,
    /// Stretched to both edges.
    Justified,
}

impl From<alignment::Horizontal> for Alignment {
    fn from(horizontal: alignment::Horizontal) -> Self {
        use alignment::Horizontal as H;

        match horizontal {
            H::Left => Self::Left,
            H::Center => Self::Center,
            H::Right => Self::Right,
        }
    }
}

impl From<crate::Alignment> for Alignment {
    fn from(alignment: crate::Alignment) -> Self {
        use crate::Alignment as A;

        match alignment {
            A::Start => Self::Left,
            A::Center => Self::Center,
            A::End => Self::Right,
        }
    }
}

/// The closest box alignment; justified and direction-following text start
/// at the left edge.
impl From<Alignment> for alignment::Horizontal {
    fn from(alignment: Alignment) -> Self {
        match alignment {
            Alignment::Center => Self::Center,
            Alignment::Right => Self::Right,
            Alignment::Default | Alignment::Left | Alignment::Justified => Self::Left,
        }
    }
}

/// How much work goes into turning characters into positioned glyphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Shaping {
    /// [`Basic`](Self::Basic) for pure ASCII, [`Advanced`](Self::Advanced)
    /// for anything else.
    Auto,
    /// One glyph per character from the chosen font, with no ligatures,
    /// complex-script handling or fallback fonts. Cheap; right only when the
    /// text and font are fully under the application's control.
    Basic,
    /// Full shaping (ligatures, complex scripts, bidirectional text) with
    /// fallback to system fonts for missing glyphs. Correct for any text;
    /// noticeably more expensive.
    Advanced,
}

/// `Advanced` with the `advanced-shaping` feature, else `Basic` with
/// `basic-shaping`, else `Auto`.
impl Default for Shaping {
    fn default() -> Self {
        match (cfg!(feature = "advanced-shaping"), cfg!(feature = "basic-shaping")) {
            (true, _) => Self::Advanced,
            (false, true) => Self::Basic,
            (false, false) => Self::Auto,
        }
    }
}

/// Where a line of text may break when it reaches the paragraph width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Wrapping {
    /// Never; lines run past the width.
    None,
    /// Between words (the default).
    #[default]
    Word,
    /// Between any two glyphs.
    Glyph,
    /// Between words, and inside a word only if it is wider than a line.
    WordOrGlyph,
}

/// Where text that does not fit in the last visible line is elided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Ellipsis {
    /// Nowhere; the text is clipped (the default).
    #[default]
    None,
    /// At the start of the line: "…end of text".
    Start,
    /// In the middle of the line: "start…end".
    Middle,
    /// At the end of the line: "Start of text…".
    End,
}

/// Distance between consecutive baselines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineHeight {
    /// A multiple of the font size.
    Relative(f32),
    /// A fixed distance in logical pixels.
    Absolute(Pixels),
}

impl LineHeight {
    /// The line height for text of `text_size`, in logical pixels.
    pub fn to_absolute(self, text_size: Pixels) -> Pixels {
        match self {
            Self::Absolute(height) => height,
            Self::Relative(factor) => Pixels(text_size.0 * factor),
        }
    }
}

/// 1.3 × the font size.
impl Default for LineHeight {
    fn default() -> Self {
        Self::Relative(1.3)
    }
}

impl From<f32> for LineHeight {
    fn from(factor: f32) -> Self {
        Self::Relative(factor)
    }
}

impl From<Pixels> for LineHeight {
    fn from(height: Pixels) -> Self {
        Self::Absolute(height)
    }
}

/// Hashes the exact bit pattern, so the hash agrees with `==` for every
/// value except `NaN` (which is never a meaningful line height).
impl Hash for LineHeight {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let (tag, value) = match *self {
            Self::Relative(factor) => (0_u8, factor),
            Self::Absolute(Pixels(height)) => (1_u8, height),
        };

        state.write_u8(tag);
        value.to_bits().hash(state);
    }
}

/// What a point on a paragraph landed on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Hit {
    /// The character at this offset.
    CharOffset(usize),
}

impl Hit {
    /// The caret position the hit corresponds to.
    pub fn cursor(self) -> usize {
        let Self::CharOffset(offset) = self;
        offset
    }
}

/// How much of a shaped [`Paragraph`] survives a change to its [`Text`],
/// as reported by [`Paragraph::compare`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difference {
    /// Nothing changed; reuse the paragraph as is.
    None,
    /// Only the bounds changed; keep the shaped glyphs, redo line layout.
    Bounds,
    /// Content, fonts, sizes or other shaping inputs changed; shape and lay
    /// out again from scratch.
    Shape,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;

    fn hash(value: LineHeight) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn line_height_resolves_against_text_size() {
        assert_eq!(LineHeight::default().to_absolute(Pixels(10.0)), Pixels(13.0));
        assert_eq!(LineHeight::from(Pixels(7.0)).to_absolute(Pixels(99.0)), Pixels(7.0));
    }

    #[test]
    fn relative_and_absolute_hash_apart() {
        assert_ne!(hash(LineHeight::Relative(2.0)), hash(LineHeight::Absolute(Pixels(2.0))));
        assert_eq!(hash(LineHeight::Relative(2.0)), hash(LineHeight::Relative(2.0)));
    }

    #[test]
    fn justified_and_default_start_at_the_left() {
        for alignment in [Alignment::Default, Alignment::Justified, Alignment::Left] {
            assert_eq!(alignment::Horizontal::from(alignment), alignment::Horizontal::Left);
        }
        assert_eq!(Alignment::from(crate::Alignment::End), Alignment::Right);
    }

    #[test]
    fn with_content_keeps_every_setting() {
        let text = Text {
            content: String::from("a"),
            bounds: Size::new(10.0, 20.0),
            size: Pixels(12.0),
            line_height: LineHeight::Relative(1.5),
            font: crate::Font::MONOSPACE,
            align_x: Alignment::Center,
            align_y: alignment::Vertical::Bottom,
            shaping: Shaping::Basic,
            wrapping: Wrapping::Glyph,
            ellipsis: Ellipsis::Middle,
            hint_factor: Some(2.0),
        };
        let borrowed = text.as_ref();

        assert_eq!(borrowed.content, "a");
        assert_eq!(borrowed.bounds, text.bounds);
        assert_eq!(borrowed.align_x, Alignment::Center);
        assert_eq!(borrowed.ellipsis, Ellipsis::Middle);
        assert_eq!(borrowed.hint_factor, Some(2.0));
    }
}

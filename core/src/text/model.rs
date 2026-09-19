use super::*;

/// A paragraph.
#[derive(Debug, Clone, Copy)]
pub struct Text<Content = String, Font = crate::Font> {
    /// The content of the paragraph.
    pub content: Content,

    /// The bounds of the paragraph.
    pub bounds: Size,

    /// The size of the [`Text`] in logical pixels.
    pub size: Pixels,

    /// The line height of the [`Text`].
    pub line_height: LineHeight,

    /// The font of the [`Text`].
    pub font: Font,

    /// The horizontal alignment of the [`Text`].
    pub align_x: Alignment,

    /// The vertical alignment of the [`Text`].
    pub align_y: alignment::Vertical,

    /// The [`Shaping`] strategy of the [`Text`].
    pub shaping: Shaping,

    /// The [`Wrapping`] strategy of the [`Text`].
    pub wrapping: Wrapping,

    /// The [`Ellipsis`] strategy of the [`Text`].
    pub ellipsis: Ellipsis,

    /// The scale factor that may be used to internally scale the layout
    /// calculation of the [`Paragraph`] and leverage metrics hinting.
    ///
    /// Effectively, this defines the "base" layout that will be used for
    /// linear scaling.
    ///
    /// If `None`, hinting will be disabled and subpixel positioning will be
    /// performed.
    pub hint_factor: Option<f32>,
}

impl<Content, Font> Text<Content, Font>
where
    Font: Copy,
{
    /// Returns a new [`Text`] replacing only the content with the
    /// given value.
    pub fn with_content<T>(&self, content: T) -> Text<T, Font> {
        Text {
            content,
            bounds: self.bounds,
            size: self.size,
            line_height: self.line_height,
            font: self.font,
            align_x: self.align_x,
            align_y: self.align_y,
            shaping: self.shaping,
            wrapping: self.wrapping,
            ellipsis: self.ellipsis,
            hint_factor: self.hint_factor,
        }
    }
}

impl<Content, Font> Text<Content, Font>
where
    Content: AsRef<str>,
    Font: Copy,
{
    /// Returns a borrowed version of [`Text`].
    pub fn as_ref(&self) -> Text<&str, Font> {
        self.with_content(self.content.as_ref())
    }
}

/// The alignment of some text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Alignment {
    /// No specific alignment.
    ///
    /// Left-to-right text will be aligned to the left, while
    /// right-to-left text will be aligned to the right.
    #[default]
    Default,
    /// Align text to the left.
    Left,
    /// Center text.
    Center,
    /// Align text to the right.
    Right,
    /// Justify text.
    Justified,
}

impl From<alignment::Horizontal> for Alignment {
    fn from(alignment: alignment::Horizontal) -> Self {
        match alignment {
            alignment::Horizontal::Left => Self::Left,
            alignment::Horizontal::Center => Self::Center,
            alignment::Horizontal::Right => Self::Right,
        }
    }
}

impl From<crate::Alignment> for Alignment {
    fn from(alignment: crate::Alignment) -> Self {
        match alignment {
            crate::Alignment::Start => Self::Left,
            crate::Alignment::Center => Self::Center,
            crate::Alignment::End => Self::Right,
        }
    }
}

impl From<Alignment> for alignment::Horizontal {
    fn from(alignment: Alignment) -> Self {
        match alignment {
            Alignment::Default | Alignment::Left | Alignment::Justified => {
                alignment::Horizontal::Left
            }
            Alignment::Center => alignment::Horizontal::Center,
            Alignment::Right => alignment::Horizontal::Right,
        }
    }
}

/// The shaping strategy of some text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Shaping {
    /// Auto-detect the best shaping strategy from the text.
    ///
    /// This strategy will use [`Basic`](Self::Basic) shaping if the
    /// text consists of only ASCII characters; otherwise, it will
    /// use [`Advanced`](Self::Advanced) shaping.
    ///
    /// This is the default, if neither the `basic-shaping` nor `advanced-shaping`
    /// features are enabled.
    Auto,
    /// No shaping and no font fallback.
    ///
    /// This shaping strategy is very cheap, but it will not display complex
    /// scripts properly nor try to find missing glyphs in your system fonts.
    ///
    /// You should use this strategy when you have complete control of the text
    /// and the font you are displaying in your application.
    ///
    /// This will be the default if the `basic-shaping` feature is enabled and
    /// the `advanced-shaping` feature is disabled.
    Basic,
    /// Advanced text shaping and font fallback.
    ///
    /// You will need to enable this flag if the text contains a complex
    /// script, the font used needs it, and/or multiple fonts in your system
    /// may be needed to display all of the glyphs.
    ///
    /// Advanced shaping is expensive! You should only enable it when necessary.
    ///
    /// This will be the default if the `advanced-shaping` feature is enabled.
    Advanced,
}

impl Default for Shaping {
    fn default() -> Self {
        if cfg!(feature = "advanced-shaping") {
            Self::Advanced
        } else if cfg!(feature = "basic-shaping") {
            Self::Basic
        } else {
            Self::Auto
        }
    }
}

/// The wrapping strategy of some text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Wrapping {
    /// No wrapping.
    None,
    /// Wraps at the word level.
    ///
    /// This is the default.
    #[default]
    Word,
    /// Wraps at the glyph level.
    Glyph,
    /// Wraps at the word level, or fallback to glyph level if a word can't fit on a line by itself.
    WordOrGlyph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
/// The ellipsis strategy of some text.
pub enum Ellipsis {
    /// No ellipsis.
    ///
    /// This is the default.
    #[default]
    None,
    /// Ellipsize the start of the last visual line in the text.
    Start,
    /// Ellipsize the middle of the last visual line in the text.
    Middle,
    /// Ellipsize the end of the last visual line in the text.
    End,
}

/// The height of a line of text in a paragraph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineHeight {
    /// A factor of the size of the text.
    Relative(f32),

    /// An absolute height in logical pixels.
    Absolute(Pixels),
}

impl LineHeight {
    /// Returns the [`LineHeight`] in absolute logical pixels.
    pub fn to_absolute(self, text_size: Pixels) -> Pixels {
        match self {
            Self::Relative(factor) => Pixels(factor * text_size.0),
            Self::Absolute(pixels) => pixels,
        }
    }
}

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
    fn from(pixels: Pixels) -> Self {
        Self::Absolute(pixels)
    }
}

impl Hash for LineHeight {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Relative(factor) => {
                state.write_u8(0);
                factor.to_bits().hash(state);
            }
            Self::Absolute(pixels) => {
                state.write_u8(1);
                f32::from(*pixels).to_bits().hash(state);
            }
        }
    }
}

/// The result of hit testing on text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Hit {
    /// The point was within the bounds of the returned character index.
    CharOffset(usize),
}

impl Hit {
    /// Computes the cursor position of the [`Hit`] .
    pub fn cursor(self) -> usize {
        match self {
            Self::CharOffset(i) => i,
        }
    }
}

/// The difference detected in some text.
///
/// You will obtain a [`Difference`] when you [`compare`] a [`Paragraph`] with some
/// [`Text`].
///
/// [`compare`]: Paragraph::compare
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difference {
    /// No difference.
    ///
    /// The text can be reused as it is!
    None,

    /// A bounds difference.
    ///
    /// This normally means a relayout is necessary, but the shape of the text can
    /// be reused.
    Bounds,

    /// A shape difference.
    ///
    /// The contents, alignment, sizes, fonts, or any other essential attributes
    /// of the shape of the text have changed. A complete reshape and relayout of
    /// the text is necessary.
    Shape,
}

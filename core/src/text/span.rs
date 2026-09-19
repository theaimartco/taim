use super::*;

/// A span of text.
#[derive(Debug, Clone)]
pub struct Span<'a, Link = (), Font = crate::Font> {
    /// The [`Fragment`] of text.
    pub text: Fragment<'a>,
    /// The size of the [`Span`] in [`Pixels`].
    pub size: Option<Pixels>,
    /// The [`LineHeight`] of the [`Span`].
    pub line_height: Option<LineHeight>,
    /// The font of the [`Span`].
    pub font: Option<Font>,
    /// The [`Color`] of the [`Span`].
    pub color: Option<Color>,
    /// The link of the [`Span`].
    pub link: Option<Link>,
    /// The [`Highlight`] of the [`Span`].
    pub highlight: Option<Highlight>,
    /// The [`Padding`] of the [`Span`].
    ///
    /// Currently, it only affects the bounds of the [`Highlight`].
    pub padding: Padding,
    /// Whether the [`Span`] should be underlined or not.
    pub underline: bool,
    /// Whether the [`Span`] should be struck through or not.
    pub strikethrough: bool,
}

/// A text highlight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Highlight {
    /// The [`Background`] of the highlight.
    pub background: Background,
    /// The [`Border`] of the highlight.
    pub border: Border,
}

impl<'a, Link, Font> Span<'a, Link, Font> {
    /// Creates a new [`Span`] of text with the given text fragment.
    pub fn new(fragment: impl IntoFragment<'a>) -> Self {
        Self {
            text: fragment.into_fragment(),
            ..Self::default()
        }
    }

    /// Sets the size of the [`Span`].
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = Some(size.into());
        self
    }

    /// Sets the [`LineHeight`] of the [`Span`].
    pub fn line_height(mut self, line_height: impl Into<LineHeight>) -> Self {
        self.line_height = Some(line_height.into());
        self
    }

    /// Sets the font of the [`Span`].
    pub fn font(mut self, font: impl Into<Font>) -> Self {
        self.font = Some(font.into());
        self
    }

    /// Sets the font of the [`Span`], if any.
    pub fn font_maybe(mut self, font: Option<impl Into<Font>>) -> Self {
        self.font = font.map(Into::into);
        self
    }

    /// Sets the [`Color`] of the [`Span`].
    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// Sets the [`Color`] of the [`Span`], if any.
    pub fn color_maybe(mut self, color: Option<impl Into<Color>>) -> Self {
        self.color = color.map(Into::into);
        self
    }

    /// Sets the link of the [`Span`].
    pub fn link(mut self, link: impl Into<Link>) -> Self {
        self.link = Some(link.into());
        self
    }

    /// Sets the link of the [`Span`], if any.
    pub fn link_maybe(mut self, link: Option<impl Into<Link>>) -> Self {
        self.link = link.map(Into::into);
        self
    }

    /// Sets the [`Background`] of the [`Span`].
    pub fn background(self, background: impl Into<Background>) -> Self {
        self.background_maybe(Some(background))
    }

    /// Sets the [`Background`] of the [`Span`], if any.
    pub fn background_maybe(mut self, background: Option<impl Into<Background>>) -> Self {
        let Some(background) = background else {
            return self;
        };

        match &mut self.highlight {
            Some(highlight) => {
                highlight.background = background.into();
            }
            None => {
                self.highlight = Some(Highlight {
                    background: background.into(),
                    border: Border::default(),
                });
            }
        }

        self
    }

    /// Sets the [`Border`] of the [`Span`].
    pub fn border(self, border: impl Into<Border>) -> Self {
        self.border_maybe(Some(border))
    }

    /// Sets the [`Border`] of the [`Span`], if any.
    pub fn border_maybe(mut self, border: Option<impl Into<Border>>) -> Self {
        let Some(border) = border else {
            return self;
        };

        match &mut self.highlight {
            Some(highlight) => {
                highlight.border = border.into();
            }
            None => {
                self.highlight = Some(Highlight {
                    border: border.into(),
                    background: Background::Color(Color::TRANSPARENT),
                });
            }
        }

        self
    }

    /// Sets the [`Padding`] of the [`Span`].
    ///
    /// It only affects the [`background`] and [`border`] of the
    /// [`Span`], currently.
    ///
    /// [`background`]: Self::background
    /// [`border`]: Self::border
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets whether the [`Span`] should be underlined or not.
    pub fn underline(mut self, underline: bool) -> Self {
        self.underline = underline;
        self
    }

    /// Sets whether the [`Span`] should be struck through or not.
    pub fn strikethrough(mut self, strikethrough: bool) -> Self {
        self.strikethrough = strikethrough;
        self
    }

    /// Turns the [`Span`] into a static one.
    pub fn to_static(self) -> Span<'static, Link, Font> {
        Span {
            text: Cow::Owned(self.text.into_owned()),
            size: self.size,
            line_height: self.line_height,
            font: self.font,
            color: self.color,
            link: self.link,
            highlight: self.highlight,
            padding: self.padding,
            underline: self.underline,
            strikethrough: self.strikethrough,
        }
    }
}

impl<Link, Font> Default for Span<'_, Link, Font> {
    fn default() -> Self {
        Self {
            text: Cow::default(),
            size: None,
            line_height: None,
            font: None,
            color: None,
            link: None,
            highlight: None,
            padding: Padding::default(),
            underline: false,
            strikethrough: false,
        }
    }
}

impl<'a, Link, Font> From<&'a str> for Span<'a, Link, Font> {
    fn from(value: &'a str) -> Self {
        Span::new(value)
    }
}

impl<Link, Font: PartialEq> PartialEq for Span<'_, Link, Font> {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
            && self.size == other.size
            && self.line_height == other.line_height
            && self.font == other.font
            && self.color == other.color
            && self.highlight == other.highlight
            && self.padding == other.padding
            && self.underline == other.underline
            && self.strikethrough == other.strikethrough
    }
}

/// A fragment of [`Text`].
///
/// This is just an alias to a string that may be either
/// borrowed or owned.
pub type Fragment<'a> = Cow<'a, str>;

/// A trait for converting a value to some text [`Fragment`].
pub trait IntoFragment<'a> {
    /// Converts the value to some text [`Fragment`].
    fn into_fragment(self) -> Fragment<'a>;
}

impl<'a> IntoFragment<'a> for Fragment<'a> {
    fn into_fragment(self) -> Fragment<'a> {
        self
    }
}

impl<'a> IntoFragment<'a> for &'a Fragment<'_> {
    fn into_fragment(self) -> Fragment<'a> {
        Fragment::Borrowed(self)
    }
}

impl<'a> IntoFragment<'a> for &'a str {
    fn into_fragment(self) -> Fragment<'a> {
        Fragment::Borrowed(self)
    }
}

impl<'a> IntoFragment<'a> for &'a String {
    fn into_fragment(self) -> Fragment<'a> {
        Fragment::Borrowed(self.as_str())
    }
}

impl<'a> IntoFragment<'a> for String {
    fn into_fragment(self) -> Fragment<'a> {
        Fragment::Owned(self)
    }
}

macro_rules! into_fragment {
    ($type:ty) => {
        impl<'a> IntoFragment<'a> for $type {
            fn into_fragment(self) -> Fragment<'a> {
                Fragment::Owned(self.to_string())
            }
        }

        impl<'a> IntoFragment<'a> for &$type {
            fn into_fragment(self) -> Fragment<'a> {
                Fragment::Owned(self.to_string())
            }
        }
    };
}

into_fragment!(char);
into_fragment!(bool);

into_fragment!(u8);
into_fragment!(u16);
into_fragment!(u32);
into_fragment!(u64);
into_fragment!(u128);
into_fragment!(usize);

into_fragment!(i8);
into_fragment!(i16);
into_fragment!(i32);
into_fragment!(i64);
into_fragment!(i128);
into_fragment!(isize);

into_fragment!(f32);
into_fragment!(f64);

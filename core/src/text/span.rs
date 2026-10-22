//! Rich-text runs.
//!
//! A [`Span`] is a run of text that shares one style. Every style field is
//! optional: `None` means "inherit from the paragraph". Spans are built with
//! chained setters; each setter also has a `_maybe` form taking an `Option`,
//! so conditional styling does not need branching at the call site.
use super::*;

/// A run of text sharing one style.
#[derive(Debug, Clone)]
pub struct Span<'a, Link = (), Font = crate::Font> {
    /// The characters of the run.
    pub text: Fragment<'a>,
    /// Font size; inherits the paragraph's when `None`.
    pub size: Option<Pixels>,
    /// Line height; inherits the paragraph's when `None`.
    pub line_height: Option<LineHeight>,
    /// Font; inherits the paragraph's when `None`.
    pub font: Option<Font>,
    /// Text color; inherits the paragraph's when `None`.
    pub color: Option<Color>,
    /// Value reported when the run is clicked, making it a link.
    pub link: Option<Link>,
    /// Box drawn behind the run.
    pub highlight: Option<Highlight>,
    /// Space between the run's glyphs and its [`Highlight`] box. It does not
    /// move the glyphs.
    pub padding: Padding,
    /// Draw a line under the run.
    pub underline: bool,
    /// Draw a line through the run.
    pub strikethrough: bool,
}

/// The box drawn behind a highlighted [`Span`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Highlight {
    /// Fill of the box.
    pub background: Background,
    /// Outline of the box.
    pub border: Border,
}

/// Generates `name(value)` and `name_maybe(option)` setters for an optional
/// style field.
macro_rules! optional_style {
    ($( $(#[$doc:meta])* $field:ident / $maybe:ident: $ty:ty; )+) => {
        $(
            $(#[$doc])*
            pub fn $field(mut self, value: impl Into<$ty>) -> Self {
                self.$field = Some(value.into());
                self
            }

            #[doc = concat!("Like [`", stringify!($field), "`](Self::", stringify!($field), "), but `None` clears it.")]
            pub fn $maybe(mut self, value: Option<impl Into<$ty>>) -> Self {
                self.$field = value.map(Into::into);
                self
            }
        )+
    };
}

impl<'a, Link, Font> Span<'a, Link, Font> {
    /// A run of `text` that inherits every style from its paragraph.
    pub fn new(text: impl IntoFragment<'a>) -> Self {
        Self {
            text: text.into_fragment(),
            ..Self::default()
        }
    }

    /// Sets the font size.
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = Some(size.into());
        self
    }

    /// Sets the line height.
    pub fn line_height(mut self, line_height: impl Into<LineHeight>) -> Self {
        self.line_height = Some(line_height.into());
        self
    }

    optional_style! {
        /// Sets the font.
        font / font_maybe: Font;
        /// Sets the text color.
        color / color_maybe: Color;
        /// Makes the run a link reporting `value` when clicked.
        link / link_maybe: Link;
    }

    /// Fills the highlight box behind the run.
    pub fn background(self, background: impl Into<Background>) -> Self {
        self.background_maybe(Some(background))
    }

    /// Like [`background`](Self::background); `None` leaves the span as is.
    pub fn background_maybe(mut self, background: Option<impl Into<Background>>) -> Self {
        if let Some(background) = background {
            self.highlight_mut().background = background.into();
        }
        self
    }

    /// Outlines the highlight box behind the run.
    pub fn border(self, border: impl Into<Border>) -> Self {
        self.border_maybe(Some(border))
    }

    /// Like [`border`](Self::border); `None` leaves the span as is.
    pub fn border_maybe(mut self, border: Option<impl Into<Border>>) -> Self {
        if let Some(border) = border {
            self.highlight_mut().border = border.into();
        }
        self
    }

    /// Sets the space between the glyphs and the highlight box drawn by
    /// [`background`](Self::background) and [`border`](Self::border).
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Underlines the run.
    pub fn underline(mut self, underline: bool) -> Self {
        self.underline = underline;
        self
    }

    /// Strikes the run through.
    pub fn strikethrough(mut self, strikethrough: bool) -> Self {
        self.strikethrough = strikethrough;
        self
    }

    /// Owns the text, detaching the span from what it borrowed.
    pub fn to_static(self) -> Span<'static, Link, Font> {
        let Self {
            text,
            size,
            line_height,
            font,
            color,
            link,
            highlight,
            padding,
            underline,
            strikethrough,
        } = self;

        Span {
            text: Cow::Owned(text.into_owned()),
            size,
            line_height,
            font,
            color,
            link,
            highlight,
            padding,
            underline,
            strikethrough,
        }
    }

    /// The highlight box, created transparent and borderless if missing.
    fn highlight_mut(&mut self) -> &mut Highlight {
        self.highlight.get_or_insert(Highlight {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
        })
    }
}

impl<Link, Font> Default for Span<'_, Link, Font> {
    fn default() -> Self {
        Self {
            text: Fragment::Borrowed(""),
            size: None,
            line_height: None,
            font: None,
            color: None,
            link: None,
            highlight: None,
            padding: Padding::ZERO,
            underline: false,
            strikethrough: false,
        }
    }
}

impl<'a, Link, Font> From<&'a str> for Span<'a, Link, Font> {
    fn from(text: &'a str) -> Self {
        Self::new(text)
    }
}

/// Two spans are equal when they look the same; the link payload is not
/// compared because it does not affect rendering.
impl<Link, Font: PartialEq> PartialEq for Span<'_, Link, Font> {
    fn eq(&self, other: &Self) -> bool {
        let looks = |span: &Self| {
            (
                span.size,
                span.line_height,
                span.color,
                span.highlight,
                span.padding,
                span.underline,
                span.strikethrough,
            )
        };

        self.text == other.text && self.font == other.font && looks(self) == looks(other)
    }
}

/// Text that is either borrowed or owned.
pub type Fragment<'a> = Cow<'a, str>;

/// Anything that can become the text of a [`Span`] or [`Text`].
pub trait IntoFragment<'a> {
    /// Converts `self` into text, borrowing when possible.
    fn into_fragment(self) -> Fragment<'a>;
}

impl<'a> IntoFragment<'a> for Fragment<'a> {
    fn into_fragment(self) -> Fragment<'a> {
        self
    }
}

impl<'a> IntoFragment<'a> for &'a Fragment<'_> {
    fn into_fragment(self) -> Fragment<'a> {
        Cow::Borrowed(self.as_ref())
    }
}

impl<'a> IntoFragment<'a> for &'a str {
    fn into_fragment(self) -> Fragment<'a> {
        Cow::Borrowed(self)
    }
}

impl<'a> IntoFragment<'a> for &'a String {
    fn into_fragment(self) -> Fragment<'a> {
        Cow::Borrowed(self)
    }
}

impl<'a> IntoFragment<'a> for String {
    fn into_fragment(self) -> Fragment<'a> {
        Cow::Owned(self)
    }
}

/// Scalars are formatted with `Display`, by value or by reference.
macro_rules! display_fragment {
    ($($scalar:ty),+ $(,)?) => {
        $(
            impl<'a> IntoFragment<'a> for $scalar {
                fn into_fragment(self) -> Fragment<'a> {
                    Cow::Owned(self.to_string())
                }
            }

            impl<'a> IntoFragment<'a> for &$scalar {
                fn into_fragment(self) -> Fragment<'a> {
                    Cow::Owned(self.to_string())
                }
            }
        )+
    };
}

display_fragment!(
    char, bool, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64,
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn border_alone_creates_a_transparent_highlight() {
        let span: Span<'_> = Span::new("x").border(Border::default().width(1.0));
        let highlight = span.highlight.expect("highlight created");

        assert_eq!(highlight.background, Background::Color(Color::TRANSPARENT));
        assert_eq!(highlight.border.width, 1.0);
    }

    #[test]
    fn background_then_border_keep_both() {
        let span: Span<'_> = Span::new("x")
            .background(Color::BLACK)
            .border(Border::default().width(2.0));
        let highlight = span.highlight.expect("highlight created");

        assert_eq!(highlight.background, Background::Color(Color::BLACK));
        assert_eq!(highlight.border.width, 2.0);
    }

    #[test]
    fn maybe_setters_clear_or_keep() {
        let span: Span<'_> = Span::new("x").color(Color::WHITE).color_maybe(None::<Color>);
        assert_eq!(span.color, None);

        let span: Span<'_> = Span::new("x").background_maybe(None::<Color>);
        assert!(span.highlight.is_none());
    }

    #[test]
    fn equality_ignores_links() {
        let a: Span<'_, u8> = Span::new("go").link(1);
        let b: Span<'_, u8> = Span::new("go").link(2);

        assert_eq!(a, b);
        assert_ne!(a, Span::new("go").underline(true));
    }

    #[test]
    fn scalars_become_text() {
        assert_eq!(42_u8.into_fragment(), "42");
        assert_eq!((&1.5_f32).into_fragment(), "1.5");
        assert!(matches!("hi".into_fragment(), Cow::Borrowed("hi")));
    }
}

//! Plain text widget model, formatting, measurement, and drawing.
mod implementation;
mod style;

pub use implementation::{draw, layout};
pub use style::{
    Catalog, Style, StyleFn, base, danger, default, primary, secondary, success, warning,
};

use crate::alignment;
use crate::text;
use crate::text::paragraph;
use crate::{Color, Length, Pixels};

pub use text::{Alignment, Ellipsis, LineHeight, Shaping, Wrapping};

/// A plain text widget backed by a borrowed or owned fragment.
#[must_use]
pub struct Text<'a, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    pub(super) fragment: text::Fragment<'a>,
    pub(super) format: Format<Renderer::Font>,
    pub(super) class: Theme::Class<'a>,
}

impl<'a, Theme, Renderer> Text<'a, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    /// Creates a text widget without allocating borrowed fragments.
    pub fn new(fragment: impl text::IntoFragment<'a>) -> Self {
        Self {
            fragment: fragment.into_fragment(),
            format: Format::default(),
            class: Theme::default(),
        }
    }

    /// Returns the text content without cloning an owned fragment.
    pub fn content(&self) -> &str {
        self.fragment.as_ref()
    }

    /// Returns whether the text fragment contains no bytes.
    pub fn is_empty(&self) -> bool {
        self.fragment.is_empty()
    }

    /// Returns the complete shaping and layout format.
    pub const fn format(&self) -> &Format<Renderer::Font> {
        &self.format
    }

    /// Replaces the complete shaping and layout format.
    pub fn with_format(mut self, format: Format<Renderer::Font>) -> Self {
        self.format = format;
        self
    }

    /// Sets a nonnegative text size.
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        let size = size.into().0;
        self.format.size = Some(Pixels(if size.is_finite() { size.max(0.0) } else { 0.0 }));
        self
    }

    /// Sets line height.
    pub fn line_height(mut self, line_height: impl Into<LineHeight>) -> Self {
        self.format.line_height = line_height.into();
        self
    }

    /// Sets font.
    pub fn font(mut self, font: impl Into<Renderer::Font>) -> Self {
        self.format.font = Some(font.into());
        self
    }

    /// Sets optional font.
    pub fn font_maybe(mut self, font: Option<impl Into<Renderer::Font>>) -> Self {
        self.format.font = font.map(Into::into);
        self
    }

    /// Sets width behavior.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.format.width = width.into();
        self
    }

    /// Sets height behavior.
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.format.height = height.into();
        self
    }

    /// Centers text on both axes.
    pub fn center(self) -> Self {
        self.align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
    }

    /// Sets horizontal alignment.
    pub fn align_x(mut self, alignment: impl Into<text::Alignment>) -> Self {
        self.format.align_x = alignment.into();
        self
    }

    /// Sets vertical alignment.
    pub fn align_y(mut self, alignment: impl Into<alignment::Vertical>) -> Self {
        self.format.align_y = alignment.into();
        self
    }

    /// Sets shaping strategy.
    pub fn shaping(mut self, shaping: Shaping) -> Self {
        self.format.shaping = shaping;
        self
    }

    /// Sets wrapping strategy.
    pub fn wrapping(mut self, wrapping: Wrapping) -> Self {
        self.format.wrapping = wrapping;
        self
    }

    /// Sets ellipsis strategy.
    pub fn ellipsis(mut self, ellipsis: Ellipsis) -> Self {
        self.format.ellipsis = ellipsis;
        self
    }

    /// Sets a dynamic style.
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets an explicit color.
    pub fn color(self, color: impl Into<Color>) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.color_maybe(Some(color))
    }

    /// Sets an optional explicit color.
    pub fn color_maybe(self, color: Option<impl Into<Color>>) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        let color = color.map(Into::into);
        self.style(move |_| Style { color })
    }

    /// Sets a theme class.
    #[cfg(feature = "advanced")]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }
}

/// Retained paragraph state.
pub type State<P> = paragraph::Plain<P>;

/// Text layout and shaping configuration.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub struct Format<Font> {
    pub width: Length,
    pub height: Length,
    pub size: Option<Pixels>,
    pub font: Option<Font>,
    pub line_height: LineHeight,
    pub align_x: text::Alignment,
    pub align_y: alignment::Vertical,
    pub shaping: Shaping,
    pub wrapping: Wrapping,
    pub ellipsis: Ellipsis,
}

impl<Font> Default for Format<Font> {
    fn default() -> Self {
        Self {
            width: Length::Shrink,
            height: Length::Shrink,
            size: None,
            font: None,
            line_height: LineHeight::default(),
            align_x: text::Alignment::Default,
            align_y: alignment::Vertical::Top,
            shaping: Shaping::default(),
            wrapping: Wrapping::default(),
            ellipsis: Ellipsis::default(),
        }
    }
}

impl<Font> Format<Font> {
    /// Sets both layout dimensions at once.
    pub fn dimensions(mut self, width: impl Into<Length>, height: impl Into<Length>) -> Self {
        self.width = width.into();
        self.height = height.into();
        self
    }

    /// Returns whether the format requests neither wrapping nor ellipsis.
    pub fn preserves_full_line(&self) -> bool {
        self.wrapping == Wrapping::None && self.ellipsis == Ellipsis::None
    }
}

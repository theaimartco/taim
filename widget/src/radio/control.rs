use super::*;

/// A circular control representing one choice in a group.
pub struct Radio<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    pub(super) is_selected: bool,
    pub(super) on_click: Message,
    pub(super) label: text::Fragment<'a>,
    pub(super) width: Length,
    pub(super) size: f32,
    pub(super) spacing: f32,
    pub(super) text_size: Option<Pixels>,
    pub(super) line_height: text::LineHeight,
    pub(super) shaping: text::Shaping,
    pub(super) wrapping: text::Wrapping,
    pub(super) font: Option<Renderer::Font>,
    pub(super) class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> Radio<'a, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: Catalog,
    Renderer: text::Renderer,
{
    /// The default diameter of a radio indicator.
    pub const DEFAULT_SIZE: f32 = 16.0;

    /// The default gap between the indicator and label.
    pub const DEFAULT_SPACING: f32 = 8.0;

    /// Creates a radio for `value` and the current `selected` value.
    pub fn new<F, V>(
        label: impl text::IntoFragment<'a>,
        value: V,
        selected: Option<V>,
        f: F,
    ) -> Self
    where
        V: Eq + Copy,
        F: FnOnce(V) -> Message,
    {
        Self {
            is_selected: Some(value) == selected,
            on_click: f(value),
            label: label.into_fragment(),
            width: Length::Shrink,
            size: Self::DEFAULT_SIZE,
            spacing: Self::DEFAULT_SPACING,
            text_size: None,
            line_height: text::LineHeight::default(),
            shaping: text::Shaping::default(),
            wrapping: text::Wrapping::default(),
            font: None,
            class: Theme::default(),
        }
    }

    /// Sets the indicator diameter.
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = size.into().0;
        self
    }

    /// Sets the total width.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the gap between indicator and label.
    pub fn spacing(mut self, spacing: impl Into<Pixels>) -> Self {
        self.spacing = spacing.into().0;
        self
    }

    /// Sets the label text size.
    pub fn text_size(mut self, text_size: impl Into<Pixels>) -> Self {
        self.text_size = Some(text_size.into());
        self
    }

    /// Sets the label line height.
    pub fn line_height(mut self, line_height: impl Into<text::LineHeight>) -> Self {
        self.line_height = line_height.into();
        self
    }

    /// Sets the label shaping strategy.
    pub fn shaping(mut self, shaping: text::Shaping) -> Self {
        self.shaping = shaping;
        self
    }

    /// Sets the label wrapping strategy.
    pub fn wrapping(mut self, wrapping: text::Wrapping) -> Self {
        self.wrapping = wrapping;
        self
    }

    /// Sets the label font.
    pub fn font(mut self, font: impl Into<Renderer::Font>) -> Self {
        self.font = Some(font.into());
        self
    }

    /// Sets a style function.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets a theme style class.
    #[cfg(feature = "advanced")]
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }
}

//! Accessible binary switch controls.
mod implementation;
mod style;

pub use style::{Catalog, Style, StyleFn, default};

use crate::core::text;
use crate::core::{Length, Pixels};

/// A binary switch with optional text.
pub struct Toggler<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    pub(super) is_toggled: bool,
    pub(super) on_toggle: Option<Box<dyn Fn(bool) -> Message + 'a>>,
    pub(super) label: Option<text::Fragment<'a>>,
    pub(super) width: Length,
    pub(super) size: f32,
    pub(super) text_size: Option<Pixels>,
    pub(super) line_height: text::LineHeight,
    pub(super) alignment: text::Alignment,
    pub(super) text_shaping: text::Shaping,
    pub(super) wrapping: text::Wrapping,
    pub(super) spacing: f32,
    pub(super) font: Option<Renderer::Font>,
    pub(super) class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> Toggler<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    /// Default switch height.
    pub const DEFAULT_SIZE: f32 = 16.0;

    /// Creates a toggler with the supplied state.
    pub fn new(is_toggled: bool) -> Self {
        Self {
            is_toggled,
            on_toggle: None,
            label: None,
            width: Length::Shrink,
            size: Self::DEFAULT_SIZE,
            text_size: None,
            line_height: text::LineHeight::default(),
            alignment: text::Alignment::Default,
            text_shaping: text::Shaping::default(),
            wrapping: text::Wrapping::default(),
            spacing: Self::DEFAULT_SIZE / 2.0,
            font: None,
            class: Theme::default(),
        }
    }

    /// Sets a borrowed or owned label fragment.
    pub fn label(mut self, label: impl text::IntoFragment<'a>) -> Self {
        self.label = Some(label.into_fragment());
        self
    }

    /// Enables the toggler with a message factory.
    pub fn on_toggle(mut self, on_toggle: impl Fn(bool) -> Message + 'a) -> Self {
        self.on_toggle = Some(Box::new(on_toggle));
        self
    }

    /// Sets an optional message factory.
    pub fn on_toggle_maybe(mut self, on_toggle: Option<impl Fn(bool) -> Message + 'a>) -> Self {
        self.on_toggle = on_toggle.map(|on_toggle| Box::new(on_toggle) as _);
        self
    }

    /// Sets switch height.
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = nonnegative(size.into().0, Self::DEFAULT_SIZE);
        self
    }

    /// Sets total width.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets label text size.
    pub fn text_size(mut self, text_size: impl Into<Pixels>) -> Self {
        self.text_size = Some(Pixels(nonnegative(text_size.into().0, 0.0)));
        self
    }

    /// Sets label line height.
    pub fn line_height(mut self, line_height: impl Into<text::LineHeight>) -> Self {
        self.line_height = line_height.into();
        self
    }

    /// Sets horizontal text alignment.
    pub fn alignment(mut self, alignment: impl Into<text::Alignment>) -> Self {
        self.alignment = alignment.into();
        self
    }

    /// Sets text shaping strategy.
    pub fn shaping(mut self, shaping: text::Shaping) -> Self {
        self.text_shaping = shaping;
        self
    }

    /// Sets text wrapping strategy.
    pub fn wrapping(mut self, wrapping: text::Wrapping) -> Self {
        self.wrapping = wrapping;
        self
    }

    /// Sets nonnegative track-to-label spacing.
    pub fn spacing(mut self, spacing: impl Into<Pixels>) -> Self {
        self.spacing = nonnegative(spacing.into().0, 0.0);
        self
    }

    /// Sets label font.
    pub fn font(mut self, font: impl Into<Renderer::Font>) -> Self {
        self.font = Some(font.into());
        self
    }

    /// Sets a status-aware dynamic style.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets a theme class.
    #[cfg(feature = "advanced")]
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    pub(super) fn status(&self, hovered: bool) -> Status {
        if self.on_toggle.is_none() {
            Status::Disabled {
                is_toggled: self.is_toggled,
            }
        } else if hovered {
            Status::Hovered {
                is_toggled: self.is_toggled,
            }
        } else {
            Status::Active {
                is_toggled: self.is_toggled,
            }
        }
    }
}

/// Interaction state used for theming.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Enabled and idle.
    Active {
        /// Current switch value.
        is_toggled: bool,
    },
    /// Enabled and hovered.
    Hovered {
        /// Current switch value.
        is_toggled: bool,
    },
    /// Disabled.
    Disabled {
        /// Current switch value.
        is_toggled: bool,
    },
}

#[inline]
fn nonnegative(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        fallback
    }
}

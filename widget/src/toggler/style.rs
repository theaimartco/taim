use crate::core::border;
use crate::core::{Background, Color, Theme};

use super::Status;

/// The visual style of a toggler.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// Track background.
    pub background: Background,
    /// Track border width.
    pub background_border_width: f32,
    /// Track border color.
    pub background_border_color: Color,
    /// Thumb background.
    pub foreground: Background,
    /// Thumb border width.
    pub foreground_border_width: f32,
    /// Thumb border color.
    pub foreground_border_color: Color,
    /// Optional label color.
    pub text_color: Option<Color>,
    /// Optional shared corner radius.
    pub border_radius: Option<border::Radius>,
    /// Thumb padding as a fraction of track height.
    pub padding_ratio: f32,
}

/// Resolves toggler style classes.
pub trait Catalog: Sized {
    /// Theme-specific class.
    type Class<'a>;

    /// Produces the default class.
    fn default<'a>() -> Self::Class<'a>;

    /// Resolves a class for the current status.
    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style;
}

/// A dynamic toggler styling function.
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, Status) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style {
        class(self, status)
    }
}

/// Default toggler style.
pub fn default(theme: &Theme, status: Status) -> Style {
    let palette = theme.palette();
    let background = match status {
        Status::Active { is_toggled } | Status::Hovered { is_toggled } => {
            if is_toggled {
                palette.primary.base.color
            } else {
                palette.background.strong.color
            }
        }
        Status::Disabled { is_toggled } => {
            if is_toggled {
                palette.background.strong.color
            } else {
                palette.background.weak.color
            }
        }
    };
    let foreground = match status {
        Status::Active { is_toggled } => {
            if is_toggled {
                palette.primary.base.text
            } else {
                palette.background.base.color
            }
        }
        Status::Hovered { is_toggled } => {
            if is_toggled {
                Color {
                    a: 0.5,
                    ..palette.primary.base.text
                }
            } else {
                palette.background.weak.color
            }
        }
        Status::Disabled { .. } => palette.background.weakest.color,
    };

    Style {
        background: background.into(),
        foreground: foreground.into(),
        foreground_border_width: 0.0,
        foreground_border_color: Color::TRANSPARENT,
        background_border_width: 0.0,
        background_border_color: Color::TRANSPARENT,
        text_color: None,
        border_radius: None,
        padding_ratio: 0.1,
    }
}

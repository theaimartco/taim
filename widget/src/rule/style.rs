use crate::core::border;
use crate::core::{Color, Theme};

use super::FillMode;

/// The visual style of a rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// Line color.
    pub color: Color,
    /// Corner radius.
    pub radius: border::Radius,
    /// Main-axis fill behavior.
    pub fill_mode: FillMode,
    /// Whether physical-pixel snapping is enabled.
    pub snap: bool,
}

/// Resolves rule style classes.
pub trait Catalog: Sized {
    /// Theme-specific class.
    type Class<'a>;

    /// Produces the default class.
    fn default<'a>() -> Self::Class<'a>;

    /// Resolves a class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A dynamic rule styling function.
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}

/// Default rule style.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();
    Style {
        color: palette.background.strong.color,
        radius: 0.0.into(),
        fill_mode: FillMode::Full,
        snap: true,
    }
}

/// Low-emphasis rule style.
pub fn weak(theme: &Theme) -> Style {
    let palette = theme.palette();
    Style {
        color: palette.background.weak.color,
        radius: 0.0.into(),
        fill_mode: FillMode::Full,
        snap: true,
    }
}

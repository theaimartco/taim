use super::*;

/// Table separator appearance.
#[derive(Debug, Clone, Copy)]
pub struct Style {
    /// Vertical separator background.
    pub separator_x: Background,
    /// Horizontal separator background.
    pub separator_y: Background,
}

/// Theme catalog for tables.
pub trait Catalog {
    /// Theme-specific class type.
    type Class<'a>;

    /// Returns the default class.
    fn default<'a>() -> Self::Class<'a>;

    /// Resolves a class into a table style.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A table styling function.
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

impl<Theme> From<Style> for StyleFn<'_, Theme> {
    fn from(style: Style) -> Self {
        Box::new(move |_theme| style)
    }
}

impl Catalog for crate::Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}

/// Returns the built-in table style.
pub fn default(theme: &crate::Theme) -> Style {
    let separator = theme.palette().background.strong.color.into();

    Style {
        separator_x: separator,
        separator_y: separator,
    }
}

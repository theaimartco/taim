use super::*;

/// QR foreground and background colors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// Data-module color.
    pub cell: Color,
    /// Background and quiet-zone color.
    pub background: Color,
}

/// Theme catalog for QR codes.
pub trait Catalog {
    /// Theme-specific class type.
    type Class<'a>;

    /// Returns the default class.
    fn default<'a>() -> Self::Class<'a>;

    /// Resolves a class into a style.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A QR-code styling function.
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

/// Returns the built-in QR-code style.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.seed();

    Style {
        cell: palette.text,
        background: palette.background,
    }
}

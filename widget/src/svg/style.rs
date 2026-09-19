use crate::core::{Color, Theme};

use super::Status;

/// The visual style of an SVG.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Style {
    /// Optional whole-image color filter.
    pub color: Option<Color>,
}

/// Resolves SVG style classes.
pub trait Catalog {
    /// Theme-specific class.
    type Class<'a>;

    /// Produces the default class.
    fn default<'a>() -> Self::Class<'a>;

    /// Resolves a class for the current interaction status.
    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style;
}

/// A dynamic SVG styling function.
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, Status) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|_, _| Style::default())
    }

    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style {
        class(self, status)
    }
}

impl<Theme> From<Style> for StyleFn<'_, Theme> {
    fn from(style: Style) -> Self {
        Box::new(move |_, _| style)
    }
}

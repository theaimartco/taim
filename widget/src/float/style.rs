use crate::core::{Shadow, border};

/// The theme catalog of a [`super::Float`].
pub trait Catalog {
    /// The item class of the catalog.
    type Class<'a>;

    /// Produces the default class.
    fn default<'a>() -> Self::Class<'a>;

    /// Resolves a class into a [`Style`].
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for a [`super::Float`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

impl Catalog for crate::Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|_| Style::default())
    }

    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}

/// The visual style of a [`super::Float`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Style {
    /// The shadow rendered behind the content.
    pub shadow: Shadow,
    /// The corner radius of the shadow.
    pub shadow_border_radius: border::Radius,
}

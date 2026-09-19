use crate::{Color, Theme};

/// Text appearance resolved from a theme class.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Style {
    /// Optional color overriding the inherited renderer color.
    pub color: Option<Color>,
}

/// Resolves text style classes.
pub trait Catalog: Sized {
    /// Theme-specific class.
    type Class<'a>;

    /// Produces the default class.
    fn default<'a>() -> Self::Class<'a>;

    /// Resolves a class.
    fn style(&self, item: &Self::Class<'_>) -> Style;
}

/// A dynamic text styling function.
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

/// Inherits the surrounding text color.
pub fn default(_theme: &Theme) -> Style {
    Style::default()
}

/// Uses the base text color.
pub fn base(theme: &Theme) -> Style {
    Style {
        color: Some(theme.seed().text),
    }
}

/// Uses the primary semantic color.
pub fn primary(theme: &Theme) -> Style {
    Style {
        color: Some(theme.seed().primary),
    }
}

/// Uses the secondary semantic color.
pub fn secondary(theme: &Theme) -> Style {
    Style {
        color: Some(theme.palette().secondary.base.color),
    }
}

/// Uses the success semantic color.
pub fn success(theme: &Theme) -> Style {
    Style {
        color: Some(theme.seed().success),
    }
}

/// Uses the warning semantic color.
pub fn warning(theme: &Theme) -> Style {
    Style {
        color: Some(theme.seed().warning),
    }
}

/// Uses the danger semantic color.
pub fn danger(theme: &Theme) -> Style {
    Style {
        color: Some(theme.seed().danger),
    }
}

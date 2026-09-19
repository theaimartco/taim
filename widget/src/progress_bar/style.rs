use crate::core::border::{self, Border};
use crate::core::{Background, Theme};

/// The visual style of a progress bar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// Inactive track background.
    pub background: Background,
    /// Active progress background.
    pub bar: Background,
    /// Track border.
    pub border: Border,
}

/// Resolves progress-bar style classes.
pub trait Catalog: Sized {
    /// Theme-specific class.
    type Class<'a>;

    /// Produces the default class.
    fn default<'a>() -> Self::Class<'a>;

    /// Resolves a class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A dynamic progress-bar styling function.
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(primary)
    }

    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}

/// Primary progress style.
pub fn primary(theme: &Theme) -> Style {
    let palette = theme.palette();
    styled(palette.background.strong.color, palette.primary.base.color)
}

/// Secondary progress style.
pub fn secondary(theme: &Theme) -> Style {
    let palette = theme.palette();
    styled(
        palette.background.strong.color,
        palette.secondary.base.color,
    )
}

/// Success progress style.
pub fn success(theme: &Theme) -> Style {
    let palette = theme.palette();
    styled(palette.background.strong.color, palette.success.base.color)
}

/// Warning progress style.
pub fn warning(theme: &Theme) -> Style {
    let palette = theme.palette();
    styled(palette.background.strong.color, palette.warning.base.color)
}

/// Danger progress style.
pub fn danger(theme: &Theme) -> Style {
    let palette = theme.palette();
    styled(palette.background.strong.color, palette.danger.base.color)
}

fn styled(background: impl Into<Background>, bar: impl Into<Background>) -> Style {
    Style {
        background: background.into(),
        bar: bar.into(),
        border: border::rounded(2),
    }
}

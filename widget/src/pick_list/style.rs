//! Pick-list visual states and theme catalog.

use crate::core::{Background, Border, Color, Theme};
use crate::overlay::menu;

/// Current visual state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Enabled and idle.
    Active,
    /// Pointer hovered.
    Hovered,
    /// Menu open, with independent field hover state.
    Opened {
        /// Whether the closed field region is hovered.
        is_hovered: bool,
    },
    /// Selection callback absent.
    Disabled,
}

/// Pick-list field appearance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// Selected label color.
    pub text_color: Color,
    /// Placeholder color.
    pub placeholder_color: Color,
    /// Indicator color.
    pub handle_color: Color,
    /// Field background.
    pub background: Background,
    /// Field border.
    pub border: Border,
}

/// Theme catalog for pick-list fields and menus.
pub trait Catalog: menu::Catalog {
    /// Field class type.
    type Class<'a>;

    /// Default field class.
    fn default<'a>() -> <Self as Catalog>::Class<'a>;

    /// Default menu class.
    fn default_menu<'a>() -> <Self as menu::Catalog>::Class<'a> {
        <Self as menu::Catalog>::default()
    }

    /// Resolves a field style.
    fn style(&self, class: &<Self as Catalog>::Class<'_>, status: Status) -> Style;
}

/// A boxed pick-list style resolver.
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, Status) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> <Self as Catalog>::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &<Self as Catalog>::Class<'_>, status: Status) -> Style {
        class(self, status)
    }
}

/// Default TAIM pick-list appearance.
pub fn default(theme: &Theme, status: Status) -> Style {
    let palette = theme.palette();
    let active = Style {
        text_color: palette.background.weak.text,
        background: palette.background.weak.color.into(),
        placeholder_color: palette.secondary.base.color,
        handle_color: palette.background.weak.text,
        border: Border {
            radius: 2.0.into(),
            width: 1.0,
            color: palette.background.strong.color,
        },
    };

    match status {
        Status::Active => active,
        Status::Hovered | Status::Opened { .. } => Style {
            border: Border {
                color: palette.primary.strong.color,
                ..active.border
            },
            ..active
        },
        Status::Disabled => Style {
            text_color: palette.background.strongest.color,
            background: palette.background.weaker.color.into(),
            placeholder_color: palette.background.strongest.color,
            handle_color: palette.background.strongest.color,
            border: Border {
                color: palette.background.weak.color,
                ..active.border
            },
        },
    }
}

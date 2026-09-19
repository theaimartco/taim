use super::*;

/// The appearance of a [`PaneGrid`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The appearance of a hovered region highlight.
    pub hovered_region: Highlight,
    /// The appearance of a picked split.
    pub picked_split: Line,
    /// The appearance of a hovered split.
    pub hovered_split: Line,
}

/// The appearance of a highlight of the [`PaneGrid`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Highlight {
    /// The [`Background`] of the pane region.
    pub background: Background,
    /// The [`Border`] of the pane region.
    pub border: Border,
}

/// A line.
///
/// It is normally used to define the highlight of something, like a split.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
    /// The [`Color`] of the [`Line`].
    pub color: Color,
    /// The width of the [`Line`].
    pub width: f32,
}

/// The theme catalog of a [`PaneGrid`].
pub trait Catalog: container::Catalog {
    /// The item class of this [`Catalog`].
    type Class<'a>;

    /// The default class produced by this [`Catalog`].
    fn default<'a>() -> <Self as Catalog>::Class<'a>;

    /// The [`Style`] of a class with the given status.
    fn style(&self, class: &<Self as Catalog>::Class<'_>) -> Style;
}

/// A styling function for a [`PaneGrid`].
///
/// This is just a boxed closure: `Fn(&Theme, Status) -> Style`.
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> StyleFn<'a, Self> {
        Box::new(default)
    }

    fn style(&self, class: &StyleFn<'_, Self>) -> Style {
        class(self)
    }
}

/// The default style of a [`PaneGrid`].
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        hovered_region: Highlight {
            background: Background::Color(Color {
                a: 0.5,
                ..palette.primary.base.color
            }),
            border: Border {
                width: 2.0,
                color: palette.primary.strong.color,
                radius: 0.0.into(),
            },
        },
        hovered_split: Line {
            color: palette.primary.base.color,
            width: 2.0,
        },
        picked_split: Line {
            color: palette.primary.strong.color,
            width: 2.0,
        },
    }
}

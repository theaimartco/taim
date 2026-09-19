use super::*;

/// The current interaction status of a radio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The radio is not hovered.
    Active {
        /// Whether this choice is selected.
        is_selected: bool,
    },
    /// The pointer is over the radio.
    Hovered {
        /// Whether this choice is selected.
        is_selected: bool,
    },
}

/// The visual appearance of a radio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// Indicator background.
    pub background: Background,
    /// Selected-dot color.
    pub dot_color: Color,
    /// Indicator border width.
    pub border_width: f32,
    /// Indicator border color.
    pub border_color: Color,
    /// Optional label color.
    pub text_color: Option<Color>,
}

/// Theme catalog for radios.
pub trait Catalog {
    /// Theme-specific class type.
    type Class<'a>;

    /// Returns the default class.
    fn default<'a>() -> Self::Class<'a>;

    /// Resolves a class and status into a style.
    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style;
}

/// A radio styling function.
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

/// Returns the built-in radio style.
pub fn default(theme: &Theme, status: Status) -> Style {
    let palette = theme.palette();
    let active = Style {
        background: Color::TRANSPARENT.into(),
        dot_color: palette.primary.strong.color,
        border_width: 1.0,
        border_color: palette.primary.strong.color,
        text_color: None,
    };

    match status {
        Status::Active { .. } => active,
        Status::Hovered { .. } => Style {
            dot_color: palette.primary.strong.color,
            background: palette.primary.weak.color.into(),
            ..active
        },
    }
}

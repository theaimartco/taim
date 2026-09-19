use super::*;

/// The appearance of a container.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The text [`Color`] of the container.
    pub text_color: Option<Color>,
    /// The [`Background`] of the container.
    pub background: Option<Background>,
    /// The [`Border`] of the container.
    pub border: Border,
    /// The [`Shadow`] of the container.
    pub shadow: Shadow,
    /// Whether the container should be snapped to the pixel grid.
    pub snap: bool,
    /// What the container is made of.
    ///
    /// [`Material::OPAQUE`] by default, which is what a container already
    /// draws without this field. A material makes the container's fill
    /// respond to the scene light, and -- once it transmits -- to what is
    /// behind it. See [`taim_core::material`] for the physical properties.
    pub material: Material,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            text_color: None,
            background: None,
            border: Border::default(),
            shadow: Shadow::default(),
            snap: renderer::CRISP,
            material: Material::OPAQUE,
        }
    }
}

impl Style {
    /// Updates the text color of the [`Style`].
    pub fn color(self, color: impl Into<Color>) -> Self {
        Self {
            text_color: Some(color.into()),
            ..self
        }
    }

    /// Updates the border of the [`Style`].
    pub fn border(self, border: impl Into<Border>) -> Self {
        Self {
            border: border.into(),
            ..self
        }
    }

    /// Updates the material of the [`Style`].
    pub fn material(self, material: Material) -> Self {
        Self { material, ..self }
    }

    /// Updates the background of the [`Style`].
    pub fn background(self, background: impl Into<Background>) -> Self {
        Self {
            background: Some(background.into()),
            ..self
        }
    }

    /// Updates the shadow of the [`Style`].
    pub fn shadow(self, shadow: impl Into<Shadow>) -> Self {
        Self {
            shadow: shadow.into(),
            ..self
        }
    }
}

impl From<Color> for Style {
    fn from(color: Color) -> Self {
        Self::default().background(color)
    }
}

impl From<Gradient> for Style {
    fn from(gradient: Gradient) -> Self {
        Self::default().background(gradient)
    }
}

impl From<gradient::Linear> for Style {
    fn from(gradient: gradient::Linear) -> Self {
        Self::default().background(gradient)
    }
}

/// The theme catalog of a [`Container`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class with the given status.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for a [`Container`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

impl<Theme> From<Style> for StyleFn<'_, Theme> {
    fn from(style: Style) -> Self {
        Box::new(move |_theme| style)
    }
}

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(transparent)
    }

    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}

/// A transparent [`Container`].
pub fn transparent<Theme>(_theme: &Theme) -> Style {
    Style::default()
}

/// A [`Container`] with the given [`Background`].
pub fn background(background: impl Into<Background>) -> Style {
    Style::default().background(background)
}

/// A rounded [`Container`] with a background.
pub fn rounded_box(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        background: Some(palette.background.weak.color.into()),
        text_color: Some(palette.background.weak.text),
        border: border::rounded(2),
        ..Style::default()
    }
}

/// A bordered [`Container`] with a background.
pub fn bordered_box(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        background: Some(palette.background.weakest.color.into()),
        text_color: Some(palette.background.weakest.text),
        border: Border {
            width: 1.0,
            radius: 5.0.into(),
            color: palette.background.weak.color,
        },
        ..Style::default()
    }
}

/// A [`Container`] with a dark background and white text.
pub fn dark(_theme: &Theme) -> Style {
    style(theme::palette::Pair {
        color: color!(0x111111),
        text: Color::WHITE,
    })
}

/// A [`Container`] with a primary background color.
pub fn primary(theme: &Theme) -> Style {
    let palette = theme.palette();

    style(palette.primary.base)
}

/// A [`Container`] with a secondary background color.
pub fn secondary(theme: &Theme) -> Style {
    let palette = theme.palette();

    style(palette.secondary.base)
}

/// A [`Container`] with a success background color.
pub fn success(theme: &Theme) -> Style {
    let palette = theme.palette();

    style(palette.success.base)
}

/// A [`Container`] with a warning background color.
pub fn warning(theme: &Theme) -> Style {
    let palette = theme.palette();

    style(palette.warning.base)
}

/// A [`Container`] with a danger background color.
pub fn danger(theme: &Theme) -> Style {
    let palette = theme.palette();

    style(palette.danger.base)
}

fn style(pair: theme::palette::Pair) -> Style {
    Style {
        background: Some(pair.color.into()),
        text_color: Some(pair.text),
        border: border::rounded(2),
        ..Style::default()
    }
}

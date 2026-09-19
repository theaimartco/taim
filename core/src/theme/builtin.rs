use super::{Palette, palette};
use crate::Color;

use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

/// A built-in theme.
#[derive(Debug, Clone, PartialEq)]
pub enum Theme {
    /// The built-in light variant.
    Light,
    /// The built-in dark variant.
    Dark,
    /// The built-in high-contrast variant.
    HighContrast,
    /// A [`Theme`] that uses a [`Custom`] palette.
    Custom(Arc<Custom>),
}

impl Theme {
    /// A list with all the defined themes.
    pub const ALL: &'static [Self] = &[Self::Light, Self::Dark, Self::HighContrast];

    /// Creates a new custom [`Theme`] from the given [`Seed`](palette::Seed).
    pub fn custom(name: impl Into<Cow<'static, str>>, seed: palette::Seed) -> Self {
        Self::custom_with_fn(name, seed, Palette::generate)
    }

    /// Creates a new custom [`Theme`] from the given [`Seed`](palette::Seed), with
    /// a custom generator of a [`Palette`].
    pub fn custom_with_fn(
        name: impl Into<Cow<'static, str>>,
        palette: palette::Seed,
        generate: impl FnOnce(palette::Seed) -> Palette,
    ) -> Self {
        Self::Custom(Arc::new(Custom::with_fn(name, palette, generate)))
    }

    /// Returns the [`Palette`] of the [`Theme`].
    pub fn palette(&self) -> &palette::Palette {
        match self {
            Self::Light => &palette::LIGHT,
            Self::Dark => &palette::DARK,
            Self::HighContrast => &palette::HIGH_CONTRAST,
            Self::Custom(custom) => &custom.palette,
        }
    }

    /// Returns the [`Seed`](palette::Seed) of the [`Theme`].
    pub fn seed(&self) -> palette::Seed {
        match self {
            Self::Light => palette::Seed::LIGHT,
            Self::Dark => palette::Seed::DARK,
            Self::HighContrast => palette::Seed::HIGH_CONTRAST,
            Self::Custom(custom) => custom.seed,
        }
    }
}

impl fmt::Display for Theme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A [`Theme`] with a customized [`Palette`].
#[derive(Debug, Clone, PartialEq)]
pub struct Custom {
    name: Cow<'static, str>,
    seed: palette::Seed,
    palette: Palette,
}

impl Custom {
    /// Creates a [`Custom`] theme from the given [`Seed`](palette::Seed).
    pub fn new(name: impl Into<Cow<'static, str>>, seed: palette::Seed) -> Self {
        Self::with_fn(name, seed, Palette::generate)
    }

    /// Creates a [`Custom`] theme from the given [`Seed`](palette::Seed) with
    /// a custom generator of a [`Palette`].
    pub fn with_fn(
        name: impl Into<Cow<'static, str>>,
        seed: palette::Seed,
        generate: impl FnOnce(palette::Seed) -> Palette,
    ) -> Self {
        Self {
            name: name.into(),
            seed,
            palette: generate(seed),
        }
    }
}

impl fmt::Display for Custom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// A theme mode, denoting the tone or brightness of a theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// No specific tone.
    #[default]
    None,
    /// A mode referring to themes with light tones.
    Light,
    /// A mode referring to themes with dark tones.
    Dark,
}

/// The base style of a theme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The background [`Color`] of the application.
    pub background_color: Color,

    /// The default text [`Color`] of the application.
    pub text_color: Color,
}

/// The default blank style of a theme.
pub trait Base {
    /// Returns the default theme for the preferred [`Mode`].
    fn default(preference: Mode) -> Self;

    /// Returns the [`Mode`] of the theme.
    fn mode(&self) -> Mode;

    /// Returns the default base [`Style`] of the theme.
    fn base(&self) -> Style;

    /// Returns the [`Seed`](palette::Seed) of the theme.
    ///
    /// This may be used by the runtime to recreate a [`Theme`] for
    /// debugging purposes; like displaying performance metrics or devtools.
    fn seed(&self) -> Option<palette::Seed>;

    /// Returns the unique name of the theme.
    ///
    /// This name may be used to efficiently detect theme
    /// changes in some widgets.
    fn name(&self) -> &str;

    /// Returns the [`SceneLight`](crate::material::SceneLight) every
    /// material-shaded surface in the theme responds to.
    ///
    /// [`SceneLight::KEY`](crate::material::SceneLight::KEY) by default,
    /// which is what the renderer already assumes when nothing overrides it
    /// -- so a theme that never opts into materials changes nothing.
    fn light(&self) -> crate::material::SceneLight {
        crate::material::SceneLight::KEY
    }
}

impl Base for Theme {
    fn default(preference: Mode) -> Self {
        use std::env;
        use std::sync::OnceLock;

        static SYSTEM: OnceLock<Option<Theme>> = OnceLock::new();

        let system = SYSTEM.get_or_init(|| {
            let name = env::var("TAIM_THEME").ok()?;

            Theme::ALL
                .iter()
                .find(|theme| theme.to_string() == name)
                .cloned()
        });

        if let Some(system) = system {
            return system.clone();
        }

        match preference {
            Mode::None | Mode::Light => Self::Light,
            Mode::Dark => Self::Dark,
        }
    }

    fn mode(&self) -> Mode {
        if self.palette().is_dark {
            Mode::Dark
        } else {
            Mode::Light
        }
    }

    fn base(&self) -> Style {
        default(self)
    }

    fn seed(&self) -> Option<palette::Seed> {
        Some(self.seed())
    }

    fn name(&self) -> &str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
            Self::HighContrast => "High Contrast",
            Self::Custom(custom) => &custom.name,
        }
    }
}

/// The default [`Style`] of a built-in [`Theme`].
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        background_color: palette.background.base.color,
        text_color: palette.background.base.text,
    }
}

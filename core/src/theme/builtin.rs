//! The themes TAIM ships, custom themes, and the [`Base`] contract every
//! theme fulfils.
use super::{Palette, palette};
use crate::Color;

use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

/// A theme: one of the built-in palettes or a [`Custom`] one.
#[derive(Debug, Clone, PartialEq)]
pub enum Theme {
    /// Light backgrounds, dark text.
    Light,
    /// Dark backgrounds, light text.
    Dark,
    /// Maximum-contrast palette for low-vision users.
    HighContrast,
    /// A palette generated from a seed.
    Custom(Arc<Custom>),
}

/// What distinguishes one built-in theme from another.
struct Builtin {
    name: &'static str,
    seed: palette::Seed,
    palette: &'static Palette,
}

impl Theme {
    /// Every built-in theme.
    pub const ALL: &'static [Self] = &[Self::Light, Self::Dark, Self::HighContrast];

    /// A custom theme named `name`, its palette generated from `seed`.
    pub fn custom(name: impl Into<Cow<'static, str>>, seed: palette::Seed) -> Self {
        Self::custom_with_fn(name, seed, Palette::generate)
    }

    /// A custom theme whose palette `generate` derives from `seed`.
    pub fn custom_with_fn(
        name: impl Into<Cow<'static, str>>,
        seed: palette::Seed,
        generate: impl FnOnce(palette::Seed) -> Palette,
    ) -> Self {
        Self::Custom(Arc::new(Custom::with_fn(name, seed, generate)))
    }

    /// The full palette widgets style themselves from.
    pub fn palette(&self) -> &Palette {
        match self.builtin() {
            Some(builtin) => builtin.palette,
            None => &self.as_custom().palette,
        }
    }

    /// The seed the palette was generated from.
    pub fn seed(&self) -> palette::Seed {
        match self.builtin() {
            Some(builtin) => builtin.seed,
            None => self.as_custom().seed,
        }
    }

    fn builtin(&self) -> Option<Builtin> {
        Some(match self {
            Self::Light => Builtin {
                name: "Light",
                seed: palette::Seed::LIGHT,
                palette: &palette::LIGHT,
            },
            Self::Dark => Builtin {
                name: "Dark",
                seed: palette::Seed::DARK,
                palette: &palette::DARK,
            },
            Self::HighContrast => Builtin {
                name: "High Contrast",
                seed: palette::Seed::HIGH_CONTRAST,
                palette: &palette::HIGH_CONTRAST,
            },
            Self::Custom(_) => return None,
        })
    }

    fn as_custom(&self) -> &Custom {
        match self {
            Self::Custom(custom) => custom,
            _ => unreachable!("built-in themes are resolved by `builtin`"),
        }
    }

    /// The built-in theme named by `TAIM_THEME`, read once per process.
    fn from_environment() -> Option<Self> {
        use std::sync::OnceLock;

        static CHOSEN: OnceLock<Option<Theme>> = OnceLock::new();

        CHOSEN
            .get_or_init(|| {
                let name = std::env::var("TAIM_THEME").ok()?;
                Self::ALL.iter().find(|theme| theme.name() == name).cloned()
            })
            .clone()
    }
}

impl fmt::Display for Theme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(Base::name(self))
    }
}

/// A named palette generated from a seed.
#[derive(Debug, Clone, PartialEq)]
pub struct Custom {
    name: Cow<'static, str>,
    seed: palette::Seed,
    palette: Palette,
}

impl Custom {
    /// A theme named `name`, its palette generated from `seed`.
    pub fn new(name: impl Into<Cow<'static, str>>, seed: palette::Seed) -> Self {
        Self::with_fn(name, seed, Palette::generate)
    }

    /// A theme whose palette `generate` derives from `seed`.
    pub fn with_fn(
        name: impl Into<Cow<'static, str>>,
        seed: palette::Seed,
        generate: impl FnOnce(palette::Seed) -> Palette,
    ) -> Self {
        Self {
            name: name.into(),
            palette: generate(seed),
            seed,
        }
    }
}

impl fmt::Display for Custom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

/// Whether a theme is light or dark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Unknown or no preference.
    #[default]
    None,
    /// Light.
    Light,
    /// Dark.
    Dark,
}

/// The colors of the window itself, before any widget draws.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// Fill behind everything.
    pub background_color: Color,
    /// Text color where a widget does not set one.
    pub text_color: Color,
}

/// What the runtime needs from any theme type.
pub trait Base {
    /// The theme to use when the application does not choose one, given the
    /// system's light/dark preference.
    fn default(preference: Mode) -> Self;

    /// Whether the theme is light or dark.
    fn mode(&self) -> Mode;

    /// Window background and default text color.
    fn base(&self) -> Style;

    /// The palette seed, if the theme has one. Lets tooling (devtools,
    /// overlays) draw in a theme matching the application's.
    fn seed(&self) -> Option<palette::Seed>;

    /// A name unique to this theme, so widgets can detect theme changes
    /// cheaply.
    fn name(&self) -> &str;

    /// The light every material-shaded surface responds to.
    ///
    /// Defaults to [`SceneLight::KEY`](crate::material::SceneLight::KEY), the
    /// renderer's own default, so themes that do not use materials are
    /// unaffected.
    fn light(&self) -> crate::material::SceneLight {
        crate::material::SceneLight::KEY
    }
}

impl Base for Theme {
    /// `TAIM_THEME` wins if it names a built-in theme; otherwise dark for a
    /// dark preference and light for anything else.
    fn default(preference: Mode) -> Self {
        Self::from_environment().unwrap_or(match preference {
            Mode::Dark => Self::Dark,
            Mode::Light | Mode::None => Self::Light,
        })
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
        Some(Theme::seed(self))
    }

    fn name(&self) -> &str {
        match self {
            Self::Custom(custom) => &custom.name,
            builtin => builtin.builtin().map_or("", |builtin| builtin.name),
        }
    }
}

/// The window colors of a built-in [`Theme`]: its base background and the
/// text drawn on it.
pub fn default(theme: &Theme) -> Style {
    let base = theme.palette().background.base;

    Style {
        background_color: base.color,
        text_color: base.text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_names_are_distinct_and_displayed() {
        let names: Vec<_> = Theme::ALL.iter().map(Theme::to_string).collect();

        assert_eq!(names, ["Light", "Dark", "High Contrast"]);
    }

    #[test]
    fn modes_follow_the_palette() {
        assert_eq!(Theme::Light.mode(), Mode::Light);
        assert_eq!(Theme::Dark.mode(), Mode::Dark);
    }

    #[test]
    fn custom_themes_keep_their_seed_and_name() {
        let seed = palette::Seed::DARK;
        let theme = Theme::custom("Midnight", seed);

        assert_eq!(theme.to_string(), "Midnight");
        assert_eq!(Theme::seed(&theme), seed);
        assert_eq!(theme.palette(), &Palette::generate(seed));
    }
}

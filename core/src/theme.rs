//! Use the built-in theme and styles.
pub mod palette;

pub use palette::Palette;

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

// -----------------------------------------------------------------
// TAIM Semantic Design Environment
// -----------------------------------------------------------------

/// Semantic color roles for Taim controls and surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ColorRole {
    /// Primary brand accent color.
    #[default]
    Primary,
    /// High-contrast text/icon color on top of Primary.
    OnPrimary,
    /// Secondary accent color for supporting UI elements.
    Secondary,
    /// High-contrast text/icon color on top of Secondary.
    OnSecondary,
    /// Standard surface container color.
    Surface,
    /// Text and icon color on standard surfaces.
    OnSurface,
    /// Subtle, tinted container color.
    SurfaceVariant,
    /// Secondary text color on variant surfaces.
    OnSurfaceVariant,
    /// Application background color.
    Background,
    /// Body text color on background.
    OnBackground,
    /// Hairline and divider boundary color.
    Outline,
    /// Critical error indicator color.
    Error,
    /// Text on error containers.
    OnError,
    /// Cautionary warning color.
    Warning,
    /// Success / confirmation color.
    Success,
}

/// Semantic surface roles establishing depth and material context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SurfaceRole {
    /// Deepest application background canvas.
    #[default]
    Canvas,
    /// Elevated content card.
    Card,
    /// Bottom sheet or modal drawer.
    Sheet,
    /// High-priority modal dialog.
    Dialog,
    /// Transient popup or dropdown menu.
    Popover,
    /// Floating action button or tool palette.
    Floating,
    /// Translucent frosted physical glass surface.
    Glass,
}

/// Typography semantic roles for scalable layout and text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TypographyRole {
    /// Hero title text.
    Display,
    /// Section header text.
    Headline,
    /// Card and dialog titles.
    Title,
    /// Standard reading text.
    #[default]
    Body,
    /// Control labels and badges.
    Label,
    /// Fixed-width numbers and alignment rows.
    Monospace,
    /// Code editor and terminal blocks.
    Code,
}

/// Elevation levels for layered depth and shadows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum ElevationRole {
    /// Flat on surface.
    #[default]
    Level0,
    /// Resting card or button.
    Level1,
    /// Hovered card or sheet.
    Level2,
    /// Modal dialog.
    Level3,
    /// Nav drawer or floating sheet.
    Level4,
    /// Topmost overlay.
    Level5,
}

/// Semantic motion roles dictating interaction curve physics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MotionRole {
    /// 0ms instant transition.
    Instant,
    /// 100ms micro-interaction (button press, checkbox toggle).
    Fast,
    /// 250ms standard layout settle.
    #[default]
    Standard,
    /// 400ms expressive page transition or sheet slide.
    Expressive,
    /// Physics-based spring with inertia.
    HeavySpring,
}

/// Interactive states for buttons, inputs, and controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum InteractionRole {
    /// Passive resting state.
    #[default]
    Idle,
    /// Pointer cursor within bounds.
    Hovered,
    /// Active mouse button or finger down.
    Pressed,
    /// Keyboard or accessibility focus ring active.
    Focused,
    /// Non-interactive disabled state.
    Disabled,
    /// Dragging in progress.
    Dragged,
}

/// Visual density scaling for high-density desktop vs spacious touch layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Density {
    /// Compact desktop layout (tight paddings, dense rows).
    Compact,
    /// Balanced standard layout.
    #[default]
    Normal,
    /// Spacious touch-first layout (minimum 48px targets).
    Comfortable,
}

/// Target device class for adaptive styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DeviceClass {
    /// Handheld touch device.
    Mobile,
    /// Medium viewport tablet.
    Tablet,
    /// Desktop mouse + keyboard environment.
    #[default]
    Desktop,
    /// Low-power embedded or automotive display.
    Embedded,
}

/// Accessibility preferences influencing color contrast and animations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AccessibilityPreferences {
    /// Enhanced contrast ratio mode.
    pub high_contrast: bool,
    /// Disables decorative motion and spring effects.
    pub reduced_motion: bool,
    /// Scales typography by 1.25x+.
    pub large_text: bool,
}

impl Theme {
    /// Resolves a semantic [`ColorRole`] against this theme's palette.
    pub fn color_role(&self, role: ColorRole) -> Color {
        let palette = self.palette();
        match role {
            ColorRole::Primary => palette.primary.base.color,
            ColorRole::OnPrimary => palette.primary.base.text,
            ColorRole::Secondary => palette.secondary.base.color,
            ColorRole::OnSecondary => palette.secondary.base.text,
            ColorRole::Surface => palette.background.base.color,
            ColorRole::OnSurface => palette.background.base.text,
            ColorRole::SurfaceVariant => palette.background.weakest.color,
            ColorRole::OnSurfaceVariant => palette.background.weakest.text,
            ColorRole::Background => palette.background.base.color,
            ColorRole::OnBackground => palette.background.base.text,
            ColorRole::Outline => palette.background.stronger.color,
            ColorRole::Error => palette.danger.base.color,
            ColorRole::OnError => palette.danger.base.text,
            ColorRole::Warning => palette.warning.base.color,
            ColorRole::Success => palette.success.base.color,
        }
    }

    /// Resolves the shadow blur radius for an [`ElevationRole`].
    pub fn elevation_blur(&self, role: ElevationRole) -> f32 {
        match role {
            ElevationRole::Level0 => 0.0,
            ElevationRole::Level1 => 2.0,
            ElevationRole::Level2 => 6.0,
            ElevationRole::Level3 => 12.0,
            ElevationRole::Level4 => 20.0,
            ElevationRole::Level5 => 32.0,
        }
    }

    /// Resolves standard motion duration in milliseconds for a [`MotionRole`].
    pub fn motion_duration_ms(&self, role: MotionRole) -> u32 {
        match role {
            MotionRole::Instant => 0,
            MotionRole::Fast => 100,
            MotionRole::Standard => 250,
            MotionRole::Expressive => 400,
            MotionRole::HeavySpring => 500,
        }
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    #[test]
    fn semantic_color_roles_resolve_consistently() {
        let theme = Theme::Dark;
        let primary = theme.color_role(ColorRole::Primary);
        let on_primary = theme.color_role(ColorRole::OnPrimary);
        assert_ne!(primary, on_primary);

        let surface = theme.color_role(ColorRole::Surface);
        let on_surface = theme.color_role(ColorRole::OnSurface);
        assert_ne!(surface, on_surface);
    }

    #[test]
    fn elevation_blur_grows_monotonically() {
        let theme = Theme::Light;
        let b0 = theme.elevation_blur(ElevationRole::Level0);
        let b1 = theme.elevation_blur(ElevationRole::Level1);
        let b2 = theme.elevation_blur(ElevationRole::Level2);
        let b3 = theme.elevation_blur(ElevationRole::Level3);
        let b4 = theme.elevation_blur(ElevationRole::Level4);
        let b5 = theme.elevation_blur(ElevationRole::Level5);

        assert!(b0 <= b1 && b1 <= b2 && b2 <= b3 && b3 <= b4 && b4 <= b5);
    }
}

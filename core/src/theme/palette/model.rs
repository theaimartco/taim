use crate::Color;

/// Complete semantic color set generated for a theme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// Background elevation roles.
    pub background: Background,
    /// Primary accent roles.
    pub primary: Swatch,
    /// Secondary accent roles.
    pub secondary: Swatch,
    /// Positive-status roles.
    pub success: Swatch,
    /// Caution-status roles.
    pub warning: Swatch,
    /// Destructive/error roles.
    pub danger: Swatch,
    /// Whether the seed background is perceptually dark.
    pub is_dark: bool,
}

/// A surface color and the best available foreground for it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pair {
    /// Surface or accent color.
    pub color: Color,
    /// Best available foreground color.
    pub text: Color,
}

/// Ordered surface elevation ramp around the theme background.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Background {
    /// Seed surface.
    pub base: Pair,
    /// Smallest elevation delta.
    pub weakest: Pair,
    /// Small elevation delta.
    pub weaker: Pair,
    /// Moderate-low elevation delta.
    pub weak: Pair,
    /// Middle elevation delta.
    pub neutral: Pair,
    /// Moderate-high elevation delta.
    pub strong: Pair,
    /// Large elevation delta.
    pub stronger: Pair,
    /// Largest elevation delta.
    pub strongest: Pair,
}

/// Base, subdued, and emphasized variants of one semantic color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Swatch {
    /// Original semantic role.
    pub base: Pair,
    /// Subdued semantic role.
    pub weak: Pair,
    /// Emphasized semantic role.
    pub strong: Pair,
}

/// Minimal inputs needed to derive a complete palette.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Seed {
    /// Base application surface.
    pub background: Color,
    /// Preferred foreground.
    pub text: Color,
    /// Primary accent.
    pub primary: Color,
    /// Positive-status accent.
    pub success: Color,
    /// Caution-status accent.
    pub warning: Color,
    /// Destructive/error accent.
    pub danger: Color,
}

impl Seed {
    /// Built-in light seed.
    pub const LIGHT: Self = Self {
        background: Color::WHITE,
        text: Color::BLACK,
        primary: Color::from_packed_rgba8(0x5865F2, 1.0),
        success: Color::from_packed_rgba8(0x12664f, 1.0),
        warning: Color::from_packed_rgba8(0xb77e33, 1.0),
        danger: Color::from_packed_rgba8(0xc3423f, 1.0),
    };

    /// Built-in dark seed.
    pub const DARK: Self = Self {
        background: Color::from_packed_rgba8(0x2B2D31, 1.0),
        text: Color::from_rgb(0.90, 0.90, 0.90),
        primary: Color::from_packed_rgba8(0x5865F2, 1.0),
        success: Color::from_packed_rgba8(0x12664f, 1.0),
        warning: Color::from_packed_rgba8(0xffc14e, 1.0),
        danger: Color::from_packed_rgba8(0xc3423f, 1.0),
    };

    /// Built-in maximum-contrast seed.
    pub const HIGH_CONTRAST: Self = Self {
        background: Color::BLACK,
        text: Color::WHITE,
        primary: Color::from_packed_rgba8(0x00ffff, 1.0),
        success: Color::from_packed_rgba8(0x00ff00, 1.0),
        warning: Color::from_packed_rgba8(0xffff00, 1.0),
        danger: Color::from_packed_rgba8(0xff0000, 1.0),
    };
}

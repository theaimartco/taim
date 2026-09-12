//! Define the colors of a theme.
use crate::{Color, color};

use std::sync::LazyLock;

/// An extended set of colors generated from a [`Seed`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// The set of background colors.
    pub background: Background,
    /// The set of primary colors.
    pub primary: Swatch,
    /// The set of secondary colors.
    pub secondary: Swatch,
    /// The set of success colors.
    pub success: Swatch,
    /// The set of warning colors.
    pub warning: Swatch,
    /// The set of danger colors.
    pub danger: Swatch,
    /// Whether the palette is dark or not.
    pub is_dark: bool,
}

impl Palette {
    /// Generates a [`Palette`] from the given [`Seed`].
    pub fn generate(palette: Seed) -> Self {
        Self {
            background: Background::new(palette.background, palette.text),
            primary: Swatch::generate(palette.primary, palette.background, palette.text),
            secondary: Swatch::derive(palette.background, palette.text),
            success: Swatch::generate(palette.success, palette.background, palette.text),
            warning: Swatch::generate(palette.warning, palette.background, palette.text),
            danger: Swatch::generate(palette.danger, palette.background, palette.text),
            is_dark: is_dark(palette.background),
        }
    }
}

/// A pair of background and text colors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pair {
    /// The background color.
    pub color: Color,

    /// The text color.
    ///
    /// It's guaranteed to be readable on top of the background [`color`].
    ///
    /// [`color`]: Self::color
    pub text: Color,
}

impl Pair {
    /// Creates a new [`Pair`] from a background [`Color`] and some text [`Color`].
    pub fn new(color: Color, text: Color) -> Self {
        Self {
            color,
            text: readable(color, text),
        }
    }
}

/// A set of background colors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Background {
    /// The base background color.
    pub base: Pair,
    /// The weakest version of the base background color.
    pub weakest: Pair,
    /// A weaker version of the base background color.
    pub weaker: Pair,
    /// A weak version of the base background color.
    pub weak: Pair,
    /// A neutral version of the base background color, between weak and strong.
    pub neutral: Pair,
    /// A strong version of the base background color.
    pub strong: Pair,
    /// A stronger version of the base background color.
    pub stronger: Pair,
    /// The strongest version of the base background color.
    pub strongest: Pair,
}

impl Background {
    /// Generates a set of [`Background`] colors from the base and text colors.
    pub fn new(base: Color, text: Color) -> Self {
        let weakest = deviate(base, 0.03);
        let weaker = deviate(base, 0.07);
        let weak = deviate(base, 0.1);
        let neutral = deviate(base, 0.125);
        let strong = deviate(base, 0.15);
        let stronger = deviate(base, 0.175);
        let strongest = deviate(base, 0.20);

        Self {
            base: Pair::new(base, text),
            weakest: Pair::new(weakest, text),
            weaker: Pair::new(weaker, text),
            weak: Pair::new(weak, text),
            neutral: Pair::new(neutral, text),
            strong: Pair::new(strong, text),
            stronger: Pair::new(stronger, text),
            strongest: Pair::new(strongest, text),
        }
    }
}

/// A color sample in a palette of colors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Swatch {
    /// The base color.
    pub base: Pair,
    /// A weaker version of the base color.
    pub weak: Pair,
    /// A stronger version of the base color.
    pub strong: Pair,
}

impl Swatch {
    /// Generates a [`Swatch`] from a base, background and text color.
    pub fn generate(base: Color, background: Color, text: Color) -> Self {
        let weak = base.mix(background, 0.4);
        let strong = deviate(base, 0.1);

        Self {
            base: Pair::new(base, text),
            weak: Pair::new(weak, text),
            strong: Pair::new(strong, text),
        }
    }

    /// Derives a [`Swatch`] from a base color and text color.
    pub fn derive(base: Color, text: Color) -> Self {
        let factor = if is_dark(base) { 0.2 } else { 0.4 };

        let weak = deviate(base, 0.1).mix(text, factor);
        let strong = deviate(base, 0.3).mix(text, factor);
        let base = deviate(base, 0.2).mix(text, factor);

        Self {
            base: Pair::new(base, text),
            weak: Pair::new(weak, text),
            strong: Pair::new(strong, text),
        }
    }
}

/// The base set of colors of a [`Palette`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Seed {
    /// The background [`Color`] of the [`Palette`].
    pub background: Color,
    /// The text [`Color`] of the [`Palette`].
    pub text: Color,
    /// The primary [`Color`] of the [`Palette`].
    pub primary: Color,
    /// The success [`Color`] of the [`Palette`].
    pub success: Color,
    /// The warning [`Color`] of the [`Palette`].
    pub warning: Color,
    /// The danger [`Color`] of the [`Palette`].
    pub danger: Color,
}

impl Seed {
    /// The built-in light variant of a [`Palette`].
    pub const LIGHT: Self = Self {
        background: Color::WHITE,
        text: Color::BLACK,
        primary: color!(0x5865F2),
        success: color!(0x12664f),
        warning: color!(0xb77e33),
        danger: color!(0xc3423f),
    };

    /// The built-in dark variant of a [`Palette`].
    pub const DARK: Self = Self {
        background: color!(0x2B2D31),
        text: Color::from_rgb(0.90, 0.90, 0.90),
        primary: color!(0x5865F2),
        success: color!(0x12664f),
        warning: color!(0xffc14e),
        danger: color!(0xc3423f),
    };

    /// The built-in high-contrast variant of a [`Palette`].
    pub const HIGH_CONTRAST: Self = Self {
        background: Color::BLACK,
        text: Color::WHITE,
        primary: color!(0x00ffff),
        success: color!(0x00ff00),
        warning: color!(0xffff00),
        danger: color!(0xff0000),
    };
}

/// The built-in light variant of a [`Palette`].
pub static LIGHT: LazyLock<Palette> = LazyLock::new(|| Palette::generate(Seed::LIGHT));

/// The built-in dark variant of a [`Palette`].
pub static DARK: LazyLock<Palette> = LazyLock::new(|| Palette::generate(Seed::DARK));

/// The built-in high-contrast variant of a [`Palette`].
pub static HIGH_CONTRAST: LazyLock<Palette> =
    LazyLock::new(|| Palette::generate(Seed::HIGH_CONTRAST));

/// Darkens a [`Color`] by the given factor.
pub fn darken(color: Color, amount: f32) -> Color {
    let mut oklch = color.into_oklch();

    // We try to bump the chroma a bit for more colorful palettes
    if oklch.c > 0.0 && oklch.c < (1.0 - oklch.l) / 2.0 {
        // Formula empirically and cluelessly derived
        oklch.c *= 1.0 + (0.2 / oklch.c).min(100.0) * amount;
    }

    oklch.l = if oklch.l - amount < 0.0 {
        0.0
    } else {
        oklch.l - amount
    };

    Color::from_oklch(oklch)
}

/// Lightens a [`Color`] by the given factor.
pub fn lighten(color: Color, amount: f32) -> Color {
    let mut oklch = color.into_oklch();

    // We try to bump the chroma a bit for more colorful palettes
    // Formula empirically and cluelessly derived
    oklch.c *= 1.0 + 2.0 * amount / oklch.l.max(0.05);

    oklch.l = if oklch.l + amount > 1.0 {
        1.0
    } else {
        oklch.l + amount
    };

    Color::from_oklch(oklch)
}

/// Deviates a [`Color`] by the given factor. Lightens if the [`Color`] is
/// dark, darkens otherwise.
pub fn deviate(color: Color, amount: f32) -> Color {
    if is_dark(color) {
        lighten(color, amount)
    } else {
        darken(color, amount)
    }
}

/// Computes a [`Color`] from the given text color that is
/// readable on top of the given background color.
pub fn readable(background: Color, text: Color) -> Color {
    if text.is_readable_on(background) {
        return text;
    }

    let improve = if is_dark(background) { lighten } else { darken };

    // TODO: Compute factor from relative contrast value
    let candidate = improve(text, 0.1);

    if candidate.is_readable_on(background) {
        return candidate;
    }

    let candidate = improve(text, 0.2);

    if candidate.is_readable_on(background) {
        return candidate;
    }

    let white_contrast = background.relative_contrast(Color::WHITE);
    let black_contrast = background.relative_contrast(Color::BLACK);

    if white_contrast >= black_contrast {
        Color::WHITE.mix(background, 0.05)
    } else {
        Color::BLACK.mix(background, 0.05)
    }
}

/// Returns true if the [`Color`] is dark.
pub fn is_dark(color: Color) -> bool {
    color.into_oklch().l < 0.6
}

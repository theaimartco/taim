//! Generate accessible color roles from a compact theme seed.
mod generate;
mod model;

pub use generate::{darken, deviate, is_dark, lighten, readable};
pub use model::{Background, Pair, Palette, Seed, Swatch};

use std::sync::LazyLock;

/// Lazily generated built-in light palette.
pub static LIGHT: LazyLock<Palette> = LazyLock::new(|| Palette::generate(Seed::LIGHT));

/// Lazily generated built-in dark palette.
pub static DARK: LazyLock<Palette> = LazyLock::new(|| Palette::generate(Seed::DARK));

/// Lazily generated maximum-contrast palette.
pub static HIGH_CONTRAST: LazyLock<Palette> =
    LazyLock::new(|| Palette::generate(Seed::HIGH_CONTRAST));

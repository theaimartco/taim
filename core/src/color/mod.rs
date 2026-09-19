//! Color values, transfer functions, display transforms, and color spaces.
pub mod display;
pub mod radiance;
pub mod space;

mod oklch;
mod parse;
mod value;

pub use display::{Display, DisplayTransform, GamutMap, ToneMap};
pub use oklch::Oklch;
pub use parse::ParseError;
pub use radiance::Radiance;
pub use space::{ColorSpace, Primaries, Transfer, WhitePoint};

/// An sRGB color with straight alpha.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Color {
    /// Red channel in `0.0..=1.0`.
    pub r: f32,
    /// Green channel in `0.0..=1.0`.
    pub g: f32,
    /// Blue channel in `0.0..=1.0`.
    pub b: f32,
    /// Alpha channel in `0.0..=1.0`.
    pub a: f32,
}

/// Creates a [`Color`] from byte channels or a packed hexadecimal literal.
#[macro_export]
macro_rules! color {
    ($r:expr, $g:expr, $b:expr) => {
        $crate::Color::from_rgb8($r, $g, $b)
    };
    ($r:expr, $g:expr, $b:expr, $a:expr) => {{ $crate::Color::from_rgba8($r, $g, $b, $a) }};
    ($hex:literal) => {{
        let mut hex = $hex as u32;
        if stringify!($hex).len() == 5 {
            let r = hex & 0xF00;
            let g = hex & 0xF0;
            let b = hex & 0xF;
            hex = (r << 12) | (r << 8) | (g << 8) | (g << 4) | (b << 4) | b;
        }
        debug_assert!(hex <= 0xffffff, "color! value must not exceed 0xffffff");
        $crate::Color::from_packed_rgba8(hex, 1.0)
    }};
    ($hex:literal, $a:expr) => {{
        let mut hex = $hex as u32;

        if stringify!($hex).len() == 5 {
            let r = hex & 0xF00;
            let g = hex & 0xF0;
            let b = hex & 0xF;
            hex = (r << 12) | (r << 8) | (g << 8) | (g << 4) | (b << 4) | b;
        }

        debug_assert!(hex <= 0xffffff, "color! value must not exceed 0xffffff");
        $crate::Color::from_packed_rgba8(hex, $a)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_format_supported_hex_forms() {
        let cases = [
            ("#ff0000", [255, 0, 0, 255], "#ff0000"),
            ("00ff0080", [0, 255, 0, 128], "#00ff0080"),
            ("#F80", [255, 136, 0, 255], "#ff8800"),
            ("#00f1", [0, 0, 255, 17], "#0000ff11"),
            ("#00ff", [0, 0, 255, 255], "#0000ff"),
        ];

        for (input, rgba, formatted) in cases {
            let color = input.parse::<Color>().expect("valid color");
            assert_eq!(color.into_rgba8(), rgba);
            assert_eq!(color.to_string(), formatted);
        }

        assert!("invalid".parse::<Color>().is_err());
    }

    const SHORTHAND: Color = color!(0x123);

    #[test]
    fn packed_shorthand_expands_each_nibble() {
        assert_eq!(SHORTHAND, Color::from_rgb8(0x11, 0x22, 0x33));
    }
}

//! Semantic colour roles and contrast checks.
#![allow(missing_docs)]
use taim_core::Color;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scheme {
    Light,
    #[default]
    Dark,
    HighContrast,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorTokens {
    pub background: Color,
    pub surface: Color,
    pub primary: Color,
    pub on_primary: Color,
    pub on_surface: Color,
    pub on_surface_variant: Color,
    pub error: Color,
    pub focus: Color,
}
impl ColorTokens {
    pub const fn for_scheme(s: Scheme) -> Self {
        match s {
            Scheme::Light => Self {
                background: Color::from_rgb8(248, 249, 252),
                surface: Color::WHITE,
                primary: Color::from_rgb8(44, 82, 180),
                on_primary: Color::WHITE,
                on_surface: Color::from_rgb8(22, 26, 35),
                on_surface_variant: Color::from_rgb8(76, 85, 99),
                error: Color::from_rgb8(186, 26, 26),
                focus: Color::from_rgb8(36, 99, 235),
            },
            Scheme::Dark => Self {
                background: Color::from_rgb8(16, 18, 24),
                surface: Color::from_rgb8(29, 32, 41),
                primary: Color::from_rgb8(158, 184, 255),
                on_primary: Color::from_rgb8(9, 39, 99),
                on_surface: Color::from_rgb8(229, 231, 235),
                on_surface_variant: Color::from_rgb8(174, 181, 194),
                error: Color::from_rgb8(255, 180, 171),
                focus: Color::from_rgb8(158, 184, 255),
            },
            Scheme::HighContrast => Self {
                background: Color::BLACK,
                surface: Color::BLACK,
                primary: Color::WHITE,
                on_primary: Color::BLACK,
                on_surface: Color::WHITE,
                on_surface_variant: Color::WHITE,
                error: Color::from_rgb8(255, 213, 0),
                focus: Color::from_rgb8(0, 229, 255),
            },
        }
    }
}
pub struct Contrast;
impl Contrast {
    pub fn ratio(f: Color, b: Color) -> f32 {
        fn l(x: f32) -> f32 {
            if x <= 0.04045 {
                x / 12.92
            } else {
                ((x + 0.055) / 1.055).powf(2.4)
            }
        }
        let y = |c: Color| 0.2126 * l(c.r) + 0.7152 * l(c.g) + 0.0722 * l(c.b);
        let (a, d) = (y(f), y(b));
        (a.max(d) + 0.05) / (a.min(d) + 0.05)
    }
    pub fn body_text(f: Color, b: Color) -> bool {
        Self::ratio(f, b) >= 4.5
    }
}

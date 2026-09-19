use crate::animation::Interpolable;

use super::Color;

impl Color {
    /// Opaque black.
    pub const BLACK: Self = Self::from_rgb(0.0, 0.0, 0.0);
    /// Opaque white.
    pub const WHITE: Self = Self::from_rgb(1.0, 1.0, 1.0);
    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self::from_rgba(0.0, 0.0, 0.0, 0.0);

    const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        debug_assert!(r >= 0.0 && r <= 1.0, "red must be in [0, 1]");
        debug_assert!(g >= 0.0 && g <= 1.0, "green must be in [0, 1]");
        debug_assert!(b >= 0.0 && b <= 1.0, "blue must be in [0, 1]");
        debug_assert!(a >= 0.0 && a <= 1.0, "alpha must be in [0, 1]");
        Self { r, g, b, a }
    }

    /// Creates an opaque sRGB color.
    #[inline]
    pub const fn from_rgb(r: f32, g: f32, b: f32) -> Self {
        Self::new(r, g, b, 1.0)
    }

    /// Creates an sRGB color with alpha.
    #[inline]
    pub const fn from_rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self::new(r, g, b, a)
    }

    /// Creates an opaque color from 8-bit channels.
    #[inline]
    pub const fn from_rgb8(r: u8, g: u8, b: u8) -> Self {
        Self::from_rgba8(r, g, b, 1.0)
    }

    /// Creates a color from 8-bit RGB channels and floating alpha.
    #[inline]
    pub const fn from_rgba8(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a)
    }

    /// Creates an opaque color from packed `0xRRGGBB` channels.
    #[inline]
    pub const fn from_packed_rgb8(rgb: u32) -> Self {
        Self::from_packed_rgba8(rgb, 1.0)
    }

    /// Creates a color from packed `0xRRGGBB` channels and alpha.
    pub const fn from_packed_rgba8(rgb: u32, a: f32) -> Self {
        Self::from_rgba8(
            ((rgb & 0xff0000) >> 16) as u8,
            ((rgb & 0xff00) >> 8) as u8,
            (rgb & 0xff) as u8,
            a,
        )
    }

    /// Converts linear-light RGB channels to sRGB.
    pub fn from_linear_rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        #[inline]
        fn gamma(channel: f32) -> f32 {
            if channel < 0.0031308 {
                12.92 * channel
            } else {
                1.055 * channel.powf(1.0 / 2.4) - 0.055
            }
        }

        Self::new(gamma(r), gamma(g), gamma(b), a)
    }

    /// Inverts RGB channels in place.
    pub const fn invert(&mut self) {
        self.r = 1.0 - self.r;
        self.g = 1.0 - self.g;
        self.b = 1.0 - self.b;
    }

    /// Returns a copy with inverted RGB channels.
    pub const fn inverse(self) -> Self {
        Self::new(1.0 - self.r, 1.0 - self.g, 1.0 - self.b, self.a)
    }

    /// Multiplies alpha by a factor.
    pub const fn scale_alpha(self, factor: f32) -> Self {
        Self {
            a: self.a * factor,
            ..self
        }
    }

    /// Mixes two colors in linear-light space.
    pub fn mix(self, other: Self, factor: f32) -> Self {
        let other_amount = if factor.is_finite() {
            factor.clamp(0.0, 1.0)
        } else {
            0.0
        };

        if other_amount == 0.0 {
            return self;
        }
        if other_amount == 1.0 {
            return other;
        }

        let self_amount = 1.0 - other_amount;
        let a = self.into_linear().map(|channel| channel * self_amount);
        let b = other.into_linear().map(|channel| channel * other_amount);

        Self::from_linear_rgba(a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3])
    }

    /// Returns WCAG relative luminance.
    #[must_use]
    pub fn relative_luminance(self) -> f32 {
        let linear = self.into_linear();
        0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]
    }

    /// Returns WCAG contrast ratio against another color.
    #[must_use]
    pub fn relative_contrast(self, other: Self) -> f32 {
        let a = self.relative_luminance();
        let b = other.relative_luminance();
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// Returns whether this color is readable on the background at TAIM's threshold.
    #[must_use]
    pub fn is_readable_on(self, background: Self) -> bool {
        background.relative_contrast(self) >= 6.0
    }

    /// Converts to 8-bit RGBA channels.
    #[must_use]
    pub const fn into_rgba8(self) -> [u8; 4] {
        [
            (self.r * 255.0).round() as u8,
            (self.g * 255.0).round() as u8,
            (self.b * 255.0).round() as u8,
            (self.a * 255.0).round() as u8,
        ]
    }

    /// Converts sRGB channels to linear-light values.
    #[must_use]
    pub fn into_linear(self) -> [f32; 4] {
        #[inline]
        fn linear(channel: f32) -> f32 {
            if channel < 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        }

        [linear(self.r), linear(self.g), linear(self.b), self.a]
    }
}

impl From<[f32; 3]> for Color {
    fn from([r, g, b]: [f32; 3]) -> Self {
        Self::new(r, g, b, 1.0)
    }
}

impl From<[f32; 4]> for Color {
    fn from([r, g, b, a]: [f32; 4]) -> Self {
        Self::new(r, g, b, a)
    }
}

impl Interpolable for Color {
    fn interpolated(&self, other: Self, ratio: f32) -> Self {
        self.mix(other, ratio)
    }
}

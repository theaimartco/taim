use super::Color;

/// A color in the [Oklab color space](https://en.wikipedia.org/wiki/Oklab_color_space),
/// represented as Oklch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklch {
    /// Perceptual lightness: 0 is pure black, 1 is pure white.
    pub l: f32,
    /// Chromatic intensity: 0 is achromatic, +0.5 is usually the upper limit.
    pub c: f32,
    /// Hue angle, in radians.
    pub h: f32,
    /// Alpha channel.
    pub a: f32,
}

impl Color {
    /// Creates the most approximate [`Color`] from its [`Oklch`] representation.
    pub fn from_oklch(oklch: Oklch) -> Color {
        let Oklch { l, c, h, a: alpha } = oklch;

        let a = c * h.cos();
        let b = c * h.sin();

        let l_ = l + 0.39633778 * a + 0.21580376 * b;
        let m_ = l - 0.105561346 * a - 0.06385417 * b;
        let s_ = l - 0.08948418 * a - 1.2914855 * b;

        let l = l_ * l_ * l_;
        let m = m_ * m_ * m_;
        let s = s_ * s_ * s_;

        let r = 4.0767417 * l - 3.3077116 * m + 0.23096994 * s;
        let g = -1.268438 * l + 2.6097574 * m - 0.34131938 * s;
        let b = -0.0041960863 * l - 0.7034186 * m + 1.7076147 * s;

        Color::from_linear_rgba(
            r.clamp(0.0, 1.0),
            g.clamp(0.0, 1.0),
            b.clamp(0.0, 1.0),
            alpha,
        )
    }

    /// Converts the [`Color`] into its [`Oklch`] representation.
    pub fn into_oklch(self) -> Oklch {
        let [r, g, b, alpha] = self.into_linear();

        let l = 0.41222146 * r + 0.53633255 * g + 0.051445995 * b;
        let m = 0.2119035 * r + 0.6806995 * g + 0.10739696 * b;
        let s = 0.08830246 * r + 0.28171885 * g + 0.6299787 * b;

        let l_ = l.cbrt();
        let m_ = m.cbrt();
        let s_ = s.cbrt();

        let l = 0.21045426 * l_ + 0.7936178 * m_ - 0.004072047 * s_;
        let a = 1.9779985 * l_ - 2.4285922 * m_ + 0.4505937 * s_;
        let b = 0.025904037 * l_ + 0.78277177 * m_ - 0.80867577 * s_;

        Oklch {
            l,
            c: (a * a + b * b).sqrt(),
            h: b.atan2(a),
            a: alpha,
        }
    }
}

impl From<Oklch> for Color {
    fn from(oklch: Oklch) -> Self {
        Self::from_oklch(oklch)
    }
}

impl From<Color> for Oklch {
    fn from(color: Color) -> Self {
        color.into_oklch()
    }
}

use super::{Background, Pair, Palette, Seed, Swatch};
use crate::Color;

const BACKGROUND_STEPS: [f32; 7] = [0.03, 0.07, 0.10, 0.125, 0.15, 0.175, 0.20];
const CONTRAST_SEARCH_STEPS: usize = 10;

impl Palette {
    /// Expands a seed into stable semantic roles.
    pub fn generate(seed: Seed) -> Self {
        Self {
            background: Background::new(seed.background, seed.text),
            primary: Swatch::generate(seed.primary, seed.background, seed.text),
            secondary: Swatch::derive(seed.background, seed.text),
            success: Swatch::generate(seed.success, seed.background, seed.text),
            warning: Swatch::generate(seed.warning, seed.background, seed.text),
            danger: Swatch::generate(seed.danger, seed.background, seed.text),
            is_dark: is_dark(seed.background),
        }
    }
}

impl Pair {
    /// Resolves the best available foreground for `color`.
    pub fn new(color: Color, requested_text: Color) -> Self {
        Self {
            color,
            text: readable(color, requested_text),
        }
    }
}

impl Background {
    /// Builds an elevation ramp around a base surface.
    pub fn new(base: Color, text: Color) -> Self {
        let [weakest, weaker, weak, neutral, strong, stronger, strongest] =
            BACKGROUND_STEPS.map(|amount| Pair::new(deviate(base, amount), text));

        Self {
            base: Pair::new(base, text),
            weakest,
            weaker,
            weak,
            neutral,
            strong,
            stronger,
            strongest,
        }
    }
}

impl Swatch {
    /// Builds semantic variants around an explicit accent.
    pub fn generate(base: Color, background: Color, text: Color) -> Self {
        Self {
            base: Pair::new(base, text),
            weak: Pair::new(base.mix(background, 0.4), text),
            strong: Pair::new(deviate(base, 0.1), text),
        }
    }

    /// Derives a neutral accent from a surface and foreground.
    pub fn derive(base: Color, text: Color) -> Self {
        let blend = if is_dark(base) { 0.2 } else { 0.4 };
        let role = |offset| Pair::new(deviate(base, offset).mix(text, blend), text);

        Self {
            base: role(0.2),
            weak: role(0.1),
            strong: role(0.3),
        }
    }
}

/// Darkens a color in perceptual Oklch space.
pub fn darken(color: Color, amount: f32) -> Color {
    shift_lightness(color, -sanitize_amount(amount))
}

/// Lightens a color in perceptual Oklch space.
pub fn lighten(color: Color, amount: f32) -> Color {
    shift_lightness(color, sanitize_amount(amount))
}

/// Moves a color away from the current surface brightness.
pub fn deviate(color: Color, amount: f32) -> Color {
    if is_dark(color) {
        lighten(color, amount)
    } else {
        darken(color, amount)
    }
}

/// Keeps the requested foreground when possible. Otherwise, finds the least
/// blend toward the highest-contrast endpoint that reaches TAIM's readability
/// threshold. Some mid-luminance surfaces cannot reach 6:1 with any sRGB
/// foreground; in that case the endpoint itself is the mathematical optimum.
pub fn readable(background: Color, requested: Color) -> Color {
    if requested.is_readable_on(background) {
        return requested;
    }

    let endpoint = if background.relative_contrast(Color::WHITE)
        >= background.relative_contrast(Color::BLACK)
    {
        Color::WHITE
    } else {
        Color::BLACK
    };

    if !endpoint.is_readable_on(background) {
        return endpoint;
    }

    let mut unreadable = 0.0;
    let mut readable = 1.0;

    for _ in 0..CONTRAST_SEARCH_STEPS {
        let factor = (unreadable + readable) * 0.5;
        if requested.mix(endpoint, factor).is_readable_on(background) {
            readable = factor;
        } else {
            unreadable = factor;
        }
    }

    requested.mix(endpoint, readable)
}

#[inline]
/// Returns whether a color belongs to TAIM's dark-surface range.
pub fn is_dark(color: Color) -> bool {
    color.relative_luminance() < 0.30
}

fn shift_lightness(color: Color, delta: f32) -> Color {
    if delta == 0.0 {
        return color;
    }

    let mut value = color.into_oklch();
    value.l = (value.l + delta).clamp(0.0, 1.0);

    // Preserve hue while gently reducing chroma near the gamut extremes.
    let headroom = (value.l * (1.0 - value.l) * 4.0).sqrt();
    value.c *= headroom;
    Color::from_oklch(value)
}

#[inline]
fn sanitize_amount(amount: f32) -> f32 {
    if amount.is_finite() {
        amount.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readable_keeps_an_already_accessible_foreground_exactly() {
        assert_eq!(readable(Color::WHITE, Color::BLACK), Color::BLACK);
    }

    #[test]
    fn impossible_threshold_uses_the_best_endpoint() {
        let background = Color::from_rgb(0.46, 0.46, 0.46);
        let foreground = readable(background, background);
        let best = if background.relative_contrast(Color::WHITE)
            >= background.relative_contrast(Color::BLACK)
        {
            Color::WHITE
        } else {
            Color::BLACK
        };

        assert_eq!(foreground, best);
    }

    #[test]
    fn invalid_adjustments_leave_the_color_unchanged() {
        let color = Color::from_rgb(0.2, 0.3, 0.4);
        assert_eq!(lighten(color, f32::NAN), color);
        assert_eq!(darken(color, f32::INFINITY), color);
    }
}

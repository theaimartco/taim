//! Scene-referred color.

use crate::color::Color;
use crate::color::space::{self, ColorSpace};

use glam::Vec3;

/// A scene-referred color: linear light, in Aura's compositing space.
///
/// [`Color`] is an *authored* color. Its components are bounded to `0.0..=1.0`
/// and encoded with the sRGB transfer function, because that is what a
/// designer types and what every color in every existing widget already is.
///
/// [`Radiance`] is what that color *becomes* once it is light. It is linear,
/// it is in [`ColorSpace::COMPOSITING`]'s Rec. 2020 primaries, and — the
/// entire point — **its components are not bounded above**. On the
/// scene-referred scale documented in [`space`], `1.0` is SDR diffuse white at
/// [`space::DIFFUSE_WHITE_NITS`]; a 1,000-nit specular highlight is `4.93`.
///
/// Clamping that highlight to `1.0` is exactly the information loss that makes
/// every user interface shipping today look like a picture of a user interface
/// instead of a light source. Nothing in this type clamps.
///
/// ```
/// use aura_core::color::{Color, Radiance};
///
/// // An ordinary authored color becomes ordinary diffuse light.
/// let plain = Radiance::from(Color::WHITE);
/// assert!((plain.luminance_nits() - 203.0).abs() < 0.1);
///
/// // The same color, asked to emit.
/// let bright = Radiance::from(Color::WHITE).at_nits(1_000.0);
/// assert!(bright.red > 4.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Radiance {
    /// Red, in Rec. 2020 linear light. Unbounded above.
    pub red: f32,
    /// Green, in Rec. 2020 linear light. Unbounded above.
    pub green: f32,
    /// Blue, in Rec. 2020 linear light. Unbounded above.
    pub blue: f32,
    /// Opacity, `0.0..=1.0`. Bounded, because opacity is a coverage fraction
    /// rather than a quantity of light.
    pub alpha: f32,
}

impl Radiance {
    /// Perfect black, fully opaque.
    pub const BLACK: Self = Self {
        red: 0.0,
        green: 0.0,
        blue: 0.0,
        alpha: 1.0,
    };

    /// SDR diffuse white, fully opaque.
    pub const WHITE: Self = Self {
        red: 1.0,
        green: 1.0,
        blue: 1.0,
        alpha: 1.0,
    };

    /// No light and no coverage.
    pub const TRANSPARENT: Self = Self {
        red: 0.0,
        green: 0.0,
        blue: 0.0,
        alpha: 0.0,
    };

    /// Creates a [`Radiance`] from linear components already in the
    /// compositing space.
    pub const fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    /// Converts an authored [`Color`], interpreted in `source`, into
    /// scene-referred light.
    ///
    /// Use [`Radiance::from`] for the overwhelmingly common case where the
    /// color is sRGB.
    #[must_use]
    pub fn from_color_in(color: Color, source: ColorSpace) -> Self {
        let linear = Vec3::new(
            source.transfer.decode(color.r),
            source.transfer.decode(color.g),
            source.transfer.decode(color.b),
        );

        let scaled = if source.transfer.is_absolute() {
            // An absolute encoding already carries its own luminance, so
            // rebase it onto the scene-referred scale rather than assuming
            // its 1.0 means diffuse white. Getting this wrong is a factor of
            // 49, which is not the kind of error that hides.
            linear * (space::PQ_PEAK_NITS / space::DIFFUSE_WHITE_NITS)
        } else {
            linear
        };

        let converted = source.convert_linear(ColorSpace::COMPOSITING, scaled);

        Self {
            red: converted.x,
            green: converted.y,
            blue: converted.z,
            alpha: color.a,
        }
    }

    /// Returns the linear components as a vector, without alpha.
    #[must_use]
    pub const fn components(self) -> Vec3 {
        Vec3::new(self.red, self.green, self.blue)
    }

    /// Returns this color's luminance, on the scene-referred scale.
    ///
    /// `1.0` is diffuse white.
    #[must_use]
    pub fn luminance(self) -> f32 {
        // Row 1 of the Rec. 2020 RGB->XYZ matrix is the luminance vector by
        // construction, so this stays correct if the primaries ever change.
        let to_xyz = ColorSpace::COMPOSITING.primaries.to_xyz();

        (to_xyz * self.components()).y
    }

    /// Returns this color's luminance in candelas per square metre.
    #[must_use]
    pub fn luminance_nits(self) -> f32 {
        space::scene_to_nits(self.luminance())
    }

    /// Returns this color scaled so that it emits exactly `nits`, preserving
    /// its chromaticity.
    ///
    /// This is the authoring primitive for HDR user interface design: pick a
    /// hue the ordinary way, then say how bright it actually is.
    ///
    /// A color with no luminance to scale — black, or fully transparent — is
    /// returned unchanged, because there is no chromaticity to preserve and
    /// the alternative is a division by zero.
    #[must_use]
    pub fn at_nits(self, nits: f32) -> Self {
        let current = self.luminance();

        if current <= 0.0 {
            return self;
        }

        self.scale_light(space::nits_to_scene(nits) / current)
    }

    /// Multiplies the light this color emits, leaving opacity alone.
    #[must_use]
    pub fn scale_light(self, factor: f32) -> Self {
        Self {
            red: self.red * factor,
            green: self.green * factor,
            blue: self.blue * factor,
            alpha: self.alpha,
        }
    }

    /// Multiplies this color's opacity, leaving the light it emits alone.
    #[must_use]
    pub fn scale_alpha(self, factor: f32) -> Self {
        Self {
            alpha: (self.alpha * factor).clamp(0.0, 1.0),
            ..self
        }
    }

    /// Returns whether this color carries more light than an SDR display can
    /// show.
    ///
    /// A scene with no such color anywhere can take the cheap presentation
    /// path, which is worth knowing before allocating a half-float target.
    #[must_use]
    pub fn is_hdr(self) -> bool {
        self.red > 1.0 || self.green > 1.0 || self.blue > 1.0
    }

    /// Converts back to an authored [`Color`] in `target`, clipping anything
    /// that space cannot represent.
    ///
    /// This is a lossy, last-resort operation for paths that have not been
    /// converted to scene-referred color yet. It clips rather than tone maps
    /// on purpose: tone mapping is a display transform, it needs to know the
    /// panel, and silently applying a guess here would make two paths through
    /// the renderer disagree about what a color looks like.
    pub fn to_color_in(self, target: ColorSpace) -> Color {
        let converted = ColorSpace::COMPOSITING.convert_linear(target, self.components());

        Color {
            r: target.transfer.encode(converted.x).clamp(0.0, 1.0),
            g: target.transfer.encode(converted.y).clamp(0.0, 1.0),
            b: target.transfer.encode(converted.z).clamp(0.0, 1.0),
            a: self.alpha,
        }
    }
}

impl From<Color> for Radiance {
    fn from(color: Color) -> Self {
        Self::from_color_in(color, ColorSpace::SRGB)
    }
}

impl From<Radiance> for Color {
    fn from(radiance: Radiance) -> Self {
        radiance.to_color_in(ColorSpace::SRGB)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diffuse_white_is_one() {
        let white = Radiance::from(Color::WHITE);

        assert!((white.red - 1.0).abs() < 1e-4);
        assert!((white.green - 1.0).abs() < 1e-4);
        assert!((white.blue - 1.0).abs() < 1e-4);
        assert!((white.luminance() - 1.0).abs() < 1e-4);
        assert!((white.luminance_nits() - space::DIFFUSE_WHITE_NITS).abs() < 1e-2);
    }

    #[test]
    fn black_is_zero_and_stays_zero() {
        let black = Radiance::from(Color::BLACK);

        assert!(black.luminance().abs() < 1e-6);
        // No division by zero, and no NaN leaking into a frame.
        assert_eq!(black.at_nits(1_000.0), black);
    }

    #[test]
    fn authored_colors_survive_a_round_trip() {
        for color in [
            Color::WHITE,
            Color::BLACK,
            Color::from_rgb8(0x00, 0xA8, 0xFF),
            Color::from_rgb8(0xFF, 0x33, 0x66),
            Color::from_rgba8(0x12, 0x34, 0x56, 0.5),
        ] {
            let round_tripped = Color::from(Radiance::from(color));

            assert!(
                (round_tripped.r - color.r).abs() < 1e-3
                    && (round_tripped.g - color.g).abs() < 1e-3
                    && (round_tripped.b - color.b).abs() < 1e-3
                    && (round_tripped.a - color.a).abs() < 1e-6,
                "{color:?} became {round_tripped:?}"
            );
        }
    }

    #[test]
    fn at_nits_hits_the_requested_luminance() {
        for nits in [1.0, 203.0, 600.0, 1_000.0, 4_000.0, 10_000.0] {
            let bright = Radiance::from(Color::from_rgb8(0x00, 0xA8, 0xFF)).at_nits(nits);

            assert!(
                (bright.luminance_nits() - nits).abs() < nits * 1e-3,
                "asked for {nits}, got {}",
                bright.luminance_nits()
            );
        }
    }

    #[test]
    fn at_nits_preserves_chromaticity() {
        let original = Radiance::from(Color::from_rgb8(0x00, 0xA8, 0xFF));
        let bright = original.at_nits(1_000.0);

        // Ratios between channels are what a hue is. Scaling luminance must
        // not shift them, or every HDR accent in the design system drifts hue
        // as it gets brighter.
        let ratio = bright.green / original.green;

        assert!((bright.red / original.red - ratio).abs() < 1e-3);
        assert!((bright.blue / original.blue - ratio).abs() < 1e-3);
    }

    #[test]
    fn a_thousand_nits_is_about_five() {
        // The number this whole pillar exists to make representable.
        let bright = Radiance::from(Color::WHITE).at_nits(1_000.0);

        assert!(
            (bright.red - 4.926).abs() < 1e-2,
            "1000 nits should be ~4.93 scene-referred, got {}",
            bright.red
        );
        assert!(bright.is_hdr());
        assert!(!Radiance::from(Color::WHITE).is_hdr());
    }

    #[test]
    fn converting_back_to_srgb_clips_rather_than_tone_maps() {
        // Documented behaviour, and worth pinning: a caller that has not been
        // ported yet must get a clipped white, not a dimmed one, so the
        // failure is visible rather than a silent global exposure change.
        let clipped = Color::from(Radiance::from(Color::WHITE).at_nits(1_000.0));

        assert!((clipped.r - 1.0).abs() < 1e-6);
        assert!((clipped.g - 1.0).abs() < 1e-6);
        assert!((clipped.b - 1.0).abs() < 1e-6);
    }

    #[test]
    fn an_absolute_encoding_is_rebased_onto_the_scene_scale() {
        // A PQ signal at diffuse white must land on 1.0, not on 0.0203. This
        // is the factor-of-49 error the conversion exists to prevent.
        let encoded = ColorSpace::REC2020_PQ.encode_from_nits(space::DIFFUSE_WHITE_NITS);

        let radiance = Radiance::from_color_in(
            Color::from_rgb(encoded, encoded, encoded),
            ColorSpace::REC2020_PQ,
        );

        assert!(
            (radiance.luminance() - 1.0).abs() < 1e-2,
            "PQ diffuse white landed at {}",
            radiance.luminance()
        );
    }

    #[test]
    fn a_wide_gamut_source_stays_wide() {
        // Display P3 red is outside sRGB. Going through Radiance must not
        // quietly shrink it to sRGB's gamut on the way in.
        let p3_red =
            Radiance::from_color_in(Color::from_rgb(1.0, 0.0, 0.0), ColorSpace::DISPLAY_P3);
        let srgb_red = Radiance::from(Color::from_rgb(1.0, 0.0, 0.0));

        assert!(
            p3_red.red > srgb_red.red,
            "P3 red ({}) should be more saturated than sRGB red ({})",
            p3_red.red,
            srgb_red.red
        );
    }

    #[test]
    fn scaling_light_leaves_opacity_alone_and_the_reverse() {
        let color = Radiance::from(Color::from_rgba8(0x40, 0x80, 0xC0, 0.5));

        assert!((color.scale_light(2.0).alpha - 0.5).abs() < 1e-6);
        assert!((color.scale_alpha(0.5).red - color.red).abs() < 1e-6);
        // Opacity is coverage, so it stays bounded even when light does not.
        assert!((color.scale_alpha(10.0).alpha - 1.0).abs() < 1e-6);
    }

    #[test]
    fn extreme_values_stay_finite() {
        for nits in [0.0, 1e-6, 1e6] {
            let color = Radiance::from(Color::WHITE).at_nits(nits);

            assert!(color.red.is_finite() && color.luminance().is_finite());
        }
    }
}

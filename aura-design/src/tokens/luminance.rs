//! Emissive levels, in candelas per square metre.
//!
//! Every other token in this system describes what a surface *is*. These
//! describe how much light it emits, which is a separate question and one no
//! mainstream design system asks: an sRGB palette can say "white" but it
//! cannot say "brighter than the page".
//!
//! A level is an absolute luminance in nits. Turning one into something a
//! renderer accepts is [`LuminanceTokens::multiplier`], whose result goes in
//! [`aura_core::renderer::Quad::luminance`].
//!
//! Values above SDR diffuse white only survive on a surface that can hold
//! them; on an ordinary one they brighten up to white and no further, so a
//! design that uses these degrades rather than breaking.

use aura_core::color::space;

/// Named emission levels, in nits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LuminanceTokens {
    /// Ordinary surfaces, text, and controls. SDR diffuse white.
    pub surface: f32,
    /// A raised surface that should read as lit rather than merely lighter:
    /// a focused field, a selected row.
    pub emphasis: f32,
    /// An active accent or a specular highlight. Visibly emissive.
    pub highlight: f32,
    /// The brightest any interface element may be.
    ///
    /// A ceiling rather than a level. Interfaces are looked at for hours, and
    /// a control that can reach a panel's full output is a control that can
    /// hurt; the limit exists so a design cannot accidentally ship one.
    pub peak: f32,
}

impl LuminanceTokens {
    /// Everything at diffuse white: what every interface built before this
    /// decade is, and what this framework drew until now.
    pub const STANDARD: Self = Self {
        surface: space::DIFFUSE_WHITE_NITS,
        emphasis: space::DIFFUSE_WHITE_NITS,
        highlight: space::DIFFUSE_WHITE_NITS,
        peak: space::DIFFUSE_WHITE_NITS,
    };

    /// A restrained high-dynamic-range set.
    ///
    /// `emphasis` is deliberately near twice diffuse white rather than ten
    /// times it. The effect that reads as quality is a surface that looks
    /// *lit*, and the effect that reads as broken is a user interface that
    /// makes someone squint.
    pub const HDR: Self = Self {
        surface: space::DIFFUSE_WHITE_NITS,
        emphasis: 400.0,
        highlight: 1_000.0,
        peak: 1_600.0,
    };

    /// Returns the multiplier for an absolute luminance.
    ///
    /// This is what [`aura_core::renderer::Quad::luminance`] takes: a factor
    /// against SDR diffuse white, so 203 nits is `1.0` and 1,000 nits is
    /// about `4.93`.
    #[must_use]
    pub fn multiplier(nits: f32) -> f32 {
        space::nits_to_scene(nits)
    }

    /// Returns `nits`, clamped to this set's [`LuminanceTokens::peak`].
    #[must_use]
    pub fn clamp(self, nits: f32) -> f32 {
        nits.clamp(0.0, self.peak)
    }

    /// Returns whether this set asks for more light than SDR can show.
    #[must_use]
    pub fn is_hdr(self) -> bool {
        self.peak > space::DIFFUSE_WHITE_NITS
    }
}

impl Default for LuminanceTokens {
    /// [`LuminanceTokens::STANDARD`].
    ///
    /// High dynamic range is opt-in throughout this framework, at every
    /// layer, because a default that changed how every existing interface
    /// looks is not a default.
    fn default() -> Self {
        Self::STANDARD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_changes_nothing() {
        let tokens = LuminanceTokens::default();

        assert_eq!(tokens, LuminanceTokens::STANDARD);
        assert!(!tokens.is_hdr());

        // Every level must resolve to a multiplier of exactly 1.0, or
        // adopting these tokens would shift the brightness of an interface
        // that asked for no such thing.
        for nits in [
            tokens.surface,
            tokens.emphasis,
            tokens.highlight,
            tokens.peak,
        ] {
            assert!((LuminanceTokens::multiplier(nits) - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn the_hdr_set_is_ordered_and_emissive() {
        let tokens = LuminanceTokens::HDR;

        assert!(tokens.surface < tokens.emphasis);
        assert!(tokens.emphasis < tokens.highlight);
        assert!(tokens.highlight <= tokens.peak);
        assert!(tokens.is_hdr());
    }

    #[test]
    fn a_thousand_nits_is_about_five_times_diffuse_white() {
        assert!((LuminanceTokens::multiplier(1_000.0) - 4.926).abs() < 1e-2);
        assert!((LuminanceTokens::multiplier(space::DIFFUSE_WHITE_NITS) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_ceiling_holds() {
        let tokens = LuminanceTokens::HDR;

        assert!((tokens.clamp(10_000.0) - tokens.peak).abs() < 1e-6);
        assert!((tokens.clamp(500.0) - 500.0).abs() < 1e-6);
        assert!(tokens.clamp(-5.0) >= 0.0);
    }

    #[test]
    fn the_standard_set_cannot_ask_for_more_than_sdr() {
        // The property that makes `STANDARD` safe as a default: nothing it
        // can produce exceeds what an ordinary surface shows.
        let tokens = LuminanceTokens::STANDARD;

        assert!(LuminanceTokens::multiplier(tokens.clamp(10_000.0)) <= 1.0);
    }
}

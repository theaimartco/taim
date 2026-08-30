//! Panel colour description, negotiated from the compositor.
//!
//! Every `Display` the renderer tone maps into has to come from somewhere.
//! Until this module existed the caller supplied one and `present()` defaulted
//! to [`Display::SDR`], which is correct for an ordinary monitor and wrong for
//! every HDR panel — the renderer would tone map a 1,000-nit scene into 203
//! nits of headroom that the panel did not have to give up.
//!
//! Wayland's `wp_color_manager_v1` is what reports the real thing: the panel's
//! primaries, its transfer function, and its luminance range. This module
//! turns that report into an [`taim_core::color::Display`].
//!
//! # What is verified here, and what is not
//!
//! The mapping and its edge cases are covered by the tests below and run on
//! every platform. **The protocol binding is not exercised anywhere**: that
//! needs a compositor advertising `wp_color_manager_v1`, and this crate has
//! never connected to one. Treat [`PanelDescription`] as correct and the wire
//! format as unverified.

use taim_core::color::display::Display;
use taim_core::color::space::{self, ColorSpace, Primaries, Transfer};

/// Named primaries, as `wp_color_manager_v1` enumerates them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedPrimaries {
    /// ITU-R BT.709 / sRGB.
    Srgb,
    /// SMPTE RP 431-2 on a D65 white, as Display P3.
    DisplayP3,
    /// ITU-R BT.2020.
    Rec2020,
}

impl NamedPrimaries {
    /// The renderer's equivalent.
    #[must_use]
    pub const fn primaries(self) -> Primaries {
        match self {
            Self::Srgb => Primaries::Srgb,
            Self::DisplayP3 => Primaries::DisplayP3,
            Self::Rec2020 => Primaries::Rec2020,
        }
    }
}

/// Named transfer functions, as `wp_color_manager_v1` enumerates them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedTransfer {
    /// The sRGB transfer function.
    Srgb,
    /// Extended linear, as an EDR or scRGB surface expects.
    ExtendedLinear,
    /// SMPTE ST 2084 Perceptual Quantizer.
    Pq,
    /// ITU-R BT.2100 Hybrid Log-Gamma.
    Hlg,
}

impl NamedTransfer {
    /// The renderer's equivalent.
    #[must_use]
    pub const fn transfer(self) -> Transfer {
        match self {
            Self::Srgb => Transfer::Srgb,
            Self::ExtendedLinear => Transfer::Linear,
            Self::Pq => Transfer::Pq,
            Self::Hlg => Transfer::Hlg,
        }
    }

    /// Whether this transfer function carries its own absolute luminance.
    #[must_use]
    pub const fn is_absolute(self) -> bool {
        matches!(self, Self::Pq)
    }
}

/// What a compositor reports about an output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelDescription {
    /// The panel's primaries.
    pub primaries: NamedPrimaries,
    /// The transfer function the surface is expected to encode with.
    pub transfer: NamedTransfer,
    /// The luminance a signal of `1.0` produces, in nits.
    ///
    /// Reported separately from `peak_nits`, and they are not the same number
    /// on any HDR panel: an extended-linear surface puts SDR white at `1.0`
    /// and the panel's headroom *above* it.
    pub reference_white_nits: f32,
    /// The brightest the panel can emit, in nits.
    pub peak_nits: f32,
    /// The darkest the panel can emit, in nits. Zero for OLED.
    pub min_nits: f32,
}

impl PanelDescription {
    /// A conservative description for a compositor that reports nothing.
    ///
    /// Standard dynamic range, sRGB, at the BT.2408 reference level. Chosen
    /// because it is the assumption that cannot make an interface *worse*: it
    /// gives up headroom that may exist rather than driving a panel past what
    /// it can show.
    pub const UNKNOWN: Self = Self {
        primaries: NamedPrimaries::Srgb,
        transfer: NamedTransfer::Srgb,
        reference_white_nits: space::DIFFUSE_WHITE_NITS,
        peak_nits: space::DIFFUSE_WHITE_NITS,
        min_nits: 0.0,
    };

    /// Returns the [`Display`] the renderer should tone map into.
    ///
    /// Values are sanitised rather than trusted. A compositor that reports a
    /// peak below its reference white, a non-finite luminance, or a black
    /// level above its peak would otherwise put a division by zero or an
    /// inverted range straight into the tone mapper, and the symptom would be
    /// a black or blown-out screen rather than an error.
    #[must_use]
    pub fn to_display(self) -> Display {
        let reference_white = sanitise(self.reference_white_nits, space::DIFFUSE_WHITE_NITS);

        // A panel cannot be dimmer than its own reference white.
        let peak = sanitise(self.peak_nits, reference_white).max(reference_white);
        let black = sanitise(self.min_nits, 0.0).clamp(0.0, peak);

        Display {
            color_space: ColorSpace {
                primaries: self.primaries.primaries(),
                transfer: self.transfer.transfer(),
                peak_nits: peak,
            },
            black_nits: black,
            reference_white_nits: reference_white,
        }
    }

    /// Whether this panel can show more light than SDR diffuse white.
    #[must_use]
    pub fn is_hdr(self) -> bool {
        self.to_display().is_hdr()
    }
}

impl Default for PanelDescription {
    fn default() -> Self {
        Self::UNKNOWN
    }
}

/// Replaces a value that cannot be used with `fallback`.
fn sanitise(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A typical HDR10 television.
    fn hdr10() -> PanelDescription {
        PanelDescription {
            primaries: NamedPrimaries::Rec2020,
            transfer: NamedTransfer::Pq,
            reference_white_nits: space::DIFFUSE_WHITE_NITS,
            peak_nits: 1_000.0,
            min_nits: 0.0,
        }
    }

    /// A typical laptop panel presenting through EDR.
    fn edr() -> PanelDescription {
        PanelDescription {
            primaries: NamedPrimaries::DisplayP3,
            transfer: NamedTransfer::ExtendedLinear,
            reference_white_nits: space::DIFFUSE_WHITE_NITS,
            peak_nits: 1_600.0,
            min_nits: 0.0,
        }
    }

    #[test]
    fn an_unreported_panel_is_assumed_standard_range() {
        let display = PanelDescription::UNKNOWN.to_display();

        assert!(!display.is_hdr());
        assert!((display.peak_nits() - space::DIFFUSE_WHITE_NITS).abs() < 1e-3);
    }

    #[test]
    fn a_reported_hdr_panel_becomes_an_hdr_display() {
        assert!(hdr10().is_hdr());
        assert!(edr().is_hdr());
        assert!(!PanelDescription::UNKNOWN.is_hdr());
    }

    #[test]
    fn reference_white_and_peak_stay_distinct() {
        // The distinction four separate defects came from collapsing. An EDR
        // panel puts SDR white at 1.0 with headroom above it; conflating the
        // two put diffuse white at 203/1600 and rendered interfaces at 12.7%.
        let display = edr().to_display();

        assert!((display.reference_white_nits - 203.0).abs() < 1e-3);
        assert!((display.peak_nits() - 1_600.0).abs() < 1e-3);
    }

    #[test]
    fn diffuse_white_survives_every_reported_panel() {
        // The invariant that matters more than any other: whatever a
        // compositor reports, ordinary interface content must reach the panel
        // as ordinary interface content.
        for panel in [PanelDescription::UNKNOWN, hdr10(), edr()] {
            let display = panel.to_display();
            let reached = display.decode_nits(display.encode_nits(space::DIFFUSE_WHITE_NITS));

            assert!(
                (reached - space::DIFFUSE_WHITE_NITS).abs() < 3.0,
                "{panel:?} received diffuse white as {reached} nits"
            );
        }
    }

    #[test]
    fn a_nonsensical_report_cannot_break_the_tone_mapper() {
        // A compositor is not a trusted source of arithmetic. Each of these
        // would otherwise reach the tone mapper as a division by zero or an
        // inverted range, and show as a black or blown-out screen rather than
        // as an error.
        let nonsense = [
            PanelDescription {
                peak_nits: 0.0,
                ..hdr10()
            },
            PanelDescription {
                peak_nits: f32::NAN,
                ..hdr10()
            },
            PanelDescription {
                peak_nits: f32::INFINITY,
                ..hdr10()
            },
            PanelDescription {
                reference_white_nits: 0.0,
                ..hdr10()
            },
            PanelDescription {
                reference_white_nits: -5.0,
                ..hdr10()
            },
            PanelDescription {
                min_nits: 5_000.0,
                ..hdr10()
            },
            PanelDescription {
                peak_nits: 50.0,
                reference_white_nits: 203.0,
                ..hdr10()
            },
        ];

        for panel in nonsense {
            let display = panel.to_display();

            assert!(display.peak_nits().is_finite() && display.peak_nits() > 0.0);
            assert!(display.reference_white_nits.is_finite());
            assert!(display.reference_white_nits > 0.0);
            assert!(display.black_nits >= 0.0);
            assert!(
                display.black_nits <= display.peak_nits(),
                "{panel:?} produced a black level above its peak"
            );

            let encoded = display.encode_nits(space::DIFFUSE_WHITE_NITS);
            assert!(
                encoded.is_finite(),
                "{panel:?} encoded diffuse white as {encoded}"
            );
        }
    }

    #[test]
    fn a_panel_cannot_be_dimmer_than_its_own_reference_white() {
        let panel = PanelDescription {
            peak_nits: 50.0,
            reference_white_nits: 203.0,
            ..hdr10()
        };

        assert!(panel.to_display().peak_nits() >= 203.0);
    }

    #[test]
    fn every_named_value_maps_to_a_renderer_equivalent() {
        // A guard against a variant being added here and silently defaulting
        // to sRGB in the mapping.
        for named in [
            NamedPrimaries::Srgb,
            NamedPrimaries::DisplayP3,
            NamedPrimaries::Rec2020,
        ] {
            let _ = named.primaries();
        }

        for named in [
            NamedTransfer::Srgb,
            NamedTransfer::ExtendedLinear,
            NamedTransfer::Pq,
            NamedTransfer::Hlg,
        ] {
            assert_eq!(named.transfer().is_absolute(), named.is_absolute());
        }
    }
}

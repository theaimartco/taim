//! Colorimetry: primaries, white points, transfer functions, and the
//! conversions between color spaces.
//!
//! # The scene-referred scale
//!
//! Aura composites in a *scene-referred* linear signal. Throughout the
//! framework, and unless a function says otherwise in its own
//! documentation, a linear value of
//!
//! > `1.0` == SDR diffuse white == [`DIFFUSE_WHITE_NITS`] (203 cd/m²)
//!
//! following ITU-R BT.2408. Values above `1.0` are legal, expected, and are
//! the entire reason this module exists: a specular highlight on a 1,000-nit
//! panel is a linear value of ~4.9, and clamping it to `1.0` is exactly the
//! information loss that makes a user interface look like a picture of a user
//! interface rather than a light source.
//!
//! Individual [`Transfer`] functions decode to their *own* native
//! normalization, because that is what their specifications define. Use
//! [`ColorSpace::decode_to_nits`] and [`ColorSpace::encode_from_nits`] when an
//! absolute luminance is what you actually want, and [`nits_to_scene`] /
//! [`scene_to_nits`] to move between absolute luminance and the framework's
//! scene-referred scale.
//!
//! # What this module does not do
//!
//! It carries no opinion about tone mapping or gamut compression. Converting a
//! 4,000-nit signal for a 600-nit panel is a *display transform*, it is lossy,
//! it is a matter of taste, and it belongs at the end of the pipeline rather
//! than in the type that describes a color space.

use glam::{Mat3, Vec3};

/// SDR diffuse white, in candelas per square metre.
///
/// ITU-R BT.2408's reference level for graphics white in an HDR container.
/// This is the anchor for the whole framework: a scene-referred value of `1.0`
/// is this many nits.
pub const DIFFUSE_WHITE_NITS: f32 = 203.0;

/// The peak luminance the PQ transfer function is defined against, in nits.
pub const PQ_PEAK_NITS: f32 = 10_000.0;

/// Converts an absolute luminance into Aura's scene-referred scale.
#[must_use]
pub fn nits_to_scene(nits: f32) -> f32 {
    nits / DIFFUSE_WHITE_NITS
}

/// Converts a scene-referred value into an absolute luminance.
#[must_use]
pub fn scene_to_nits(scene: f32) -> f32 {
    scene * DIFFUSE_WHITE_NITS
}

/// A CIE 1931 *xy* chromaticity coordinate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Chromaticity {
    /// The *x* coordinate.
    pub x: f32,
    /// The *y* coordinate.
    pub y: f32,
}

impl Chromaticity {
    /// Creates a [`Chromaticity`].
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Returns the CIE XYZ tristimulus of this chromaticity, normalized so
    /// that `Y == 1.0`.
    #[must_use]
    pub fn xyz(self) -> Vec3 {
        Vec3::new(self.x / self.y, 1.0, (1.0 - self.x - self.y) / self.y)
    }
}

/// A reference white.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WhitePoint {
    /// CIE D65. The white of sRGB, Display P3, and Rec. 2020.
    D65,
    /// CIE D50. The white of the ICC profile connection space.
    D50,
    /// The ACES white, approximately D60.
    D60,
}

impl WhitePoint {
    /// Returns the chromaticity of this white point.
    ///
    /// D65 is the value the sRGB, Display P3, and Rec. 2020 specifications
    /// all literally declare, not the longer CIE illuminant expansion. This
    /// matters: matrices derived from the two differ in the fourth decimal,
    /// and published reference matrices disagree with each other about which
    /// was used. Following the specifications reproduces the Display P3 and
    /// Rec. 2020 matrices to 5e-8 and the sRGB specification's own printed
    /// matrix to 4e-5, which is the closest any single choice gets.
    #[must_use]
    pub const fn chromaticity(self) -> Chromaticity {
        match self {
            Self::D65 => Chromaticity::new(0.3127, 0.3290),
            Self::D50 => Chromaticity::new(0.34567, 0.35850),
            Self::D60 => Chromaticity::new(0.32168, 0.33767),
        }
    }

    /// Returns the CIE XYZ tristimulus of this white point, normalized so that
    /// `Y == 1.0`.
    #[must_use]
    pub fn xyz(self) -> Vec3 {
        self.chromaticity().xyz()
    }
}

/// A set of additive RGB primaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Primaries {
    /// ITU-R BT.709, as used by sRGB. The 1990s.
    Srgb,
    /// SMPTE RP 431-2 primaries on a D65 white. What every panel worth
    /// shipping on has covered since about 2016.
    DisplayP3,
    /// ITU-R BT.2020. The container every HDR standard is defined in.
    Rec2020,
    /// ACES AP1, on the ACES ~D60 white. A working space wide enough that
    /// intermediate math never leaves it.
    AcesCg,
}

impl Primaries {
    /// Returns the red, green, and blue chromaticities, in that order.
    #[must_use]
    pub const fn chromaticities(self) -> [Chromaticity; 3] {
        match self {
            Self::Srgb => [
                Chromaticity::new(0.640, 0.330),
                Chromaticity::new(0.300, 0.600),
                Chromaticity::new(0.150, 0.060),
            ],
            Self::DisplayP3 => [
                Chromaticity::new(0.680, 0.320),
                Chromaticity::new(0.265, 0.690),
                Chromaticity::new(0.150, 0.060),
            ],
            Self::Rec2020 => [
                Chromaticity::new(0.708, 0.292),
                Chromaticity::new(0.170, 0.797),
                Chromaticity::new(0.131, 0.046),
            ],
            Self::AcesCg => [
                Chromaticity::new(0.713, 0.293),
                Chromaticity::new(0.165, 0.830),
                Chromaticity::new(0.128, 0.044),
            ],
        }
    }

    /// Returns the white point these primaries are defined against.
    #[must_use]
    pub const fn white_point(self) -> WhitePoint {
        match self {
            Self::Srgb | Self::DisplayP3 | Self::Rec2020 => WhitePoint::D65,
            Self::AcesCg => WhitePoint::D60,
        }
    }

    /// Returns the matrix taking linear RGB in these primaries to CIE XYZ
    /// under this set's own white point.
    ///
    /// Derived from the chromaticities rather than tabulated, so there is one
    /// source of truth and a typo in a primary shows up as a failing test
    /// against the published matrices rather than as a subtly wrong color.
    #[must_use]
    pub fn to_xyz(self) -> Mat3 {
        let [red, green, blue] = self.chromaticities();

        let scaled = from_rows(
            [red.xyz().x, green.xyz().x, blue.xyz().x],
            [red.xyz().y, green.xyz().y, blue.xyz().y],
            [red.xyz().z, green.xyz().z, blue.xyz().z],
        );

        let scale = scaled.inverse() * self.white_point().xyz();

        scaled * Mat3::from_diagonal(scale)
    }

    /// Returns the matrix taking CIE XYZ to linear RGB in these primaries.
    #[must_use]
    pub fn from_xyz(self) -> Mat3 {
        self.to_xyz().inverse()
    }
}

/// A transfer function: the mapping between an encoded signal and a linear
/// one.
///
/// Each variant decodes to its own specification's native normalization. See
/// [`Transfer::decode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Transfer {
    /// The identity. Already linear.
    Linear,
    /// The sRGB transfer function (IEC 61966-2-1).
    Srgb,
    /// Perceptual Quantizer (SMPTE ST 2084), as used by HDR10 and Dolby
    /// Vision. Absolute: an encoded value maps to a specific luminance
    /// regardless of the display.
    Pq,
    /// Hybrid Log-Gamma (ITU-R BT.2100). Relative: scene-referred, with the
    /// display's own OOTF applied at presentation.
    Hlg,
}

impl Transfer {
    /// Decodes an encoded signal into a linear one.
    ///
    /// The normalization of the result depends on the variant, because the
    /// specifications disagree and pretending otherwise is how colors end up
    /// 49× too bright:
    ///
    /// - [`Transfer::Linear`] and [`Transfer::Srgb`] return `1.0` for diffuse
    ///   white.
    /// - [`Transfer::Pq`] returns `1.0` for [`PQ_PEAK_NITS`], so diffuse white
    ///   decodes to about `0.0203`.
    /// - [`Transfer::Hlg`] returns scene-referred light in `0.0..=1.0` with
    ///   the display OOTF *not* applied.
    ///
    /// Use [`ColorSpace::decode_to_nits`] when you want an absolute answer.
    #[must_use]
    pub fn decode(self, encoded: f32) -> f32 {
        match self {
            Self::Linear => encoded,
            Self::Srgb => {
                if encoded <= 0.04045 {
                    encoded / 12.92
                } else {
                    ((encoded + 0.055) / 1.055).powf(2.4)
                }
            }
            Self::Pq => {
                let encoded = encoded.clamp(0.0, 1.0);
                let powed = encoded.powf(1.0 / PQ_M2);
                let numerator = (powed - PQ_C1).max(0.0);
                let denominator = PQ_C2 - PQ_C3 * powed;

                if denominator <= 0.0 {
                    0.0
                } else {
                    (numerator / denominator).powf(1.0 / PQ_M1)
                }
            }
            Self::Hlg => {
                // BT.2100 defines HLG only on 0..=1. The upper branch is an
                // exponential, so an out-of-domain input does not merely give
                // a wrong answer -- it gives an infinity, which then spreads
                // through every blend it touches for the rest of the frame.
                let encoded = encoded.clamp(0.0, 1.0);

                if encoded <= 0.5 {
                    encoded * encoded / 3.0
                } else {
                    (((encoded - HLG_C) / HLG_A).exp() + HLG_B) / 12.0
                }
            }
        }
    }

    /// Encodes a linear signal.
    ///
    /// The exact inverse of [`Transfer::decode`], and normalized the same way.
    #[must_use]
    pub fn encode(self, linear: f32) -> f32 {
        match self {
            Self::Linear => linear,
            Self::Srgb => {
                if linear <= 0.003_130_8 {
                    12.92 * linear
                } else {
                    1.055 * linear.powf(1.0 / 2.4) - 0.055
                }
            }
            Self::Pq => {
                let powed = linear.max(0.0).powf(PQ_M1);

                ((PQ_C1 + PQ_C2 * powed) / (1.0 + PQ_C3 * powed)).powf(PQ_M2)
            }
            Self::Hlg => {
                let linear = linear.max(0.0);

                if linear <= 1.0 / 12.0 {
                    (3.0 * linear).sqrt()
                } else {
                    HLG_A * (12.0 * linear - HLG_B).ln() + HLG_C
                }
            }
        }
    }

    /// Returns whether this transfer function is defined against an absolute
    /// luminance rather than a relative one.
    #[must_use]
    pub const fn is_absolute(self) -> bool {
        matches!(self, Self::Pq)
    }
}

// SMPTE ST 2084, table 4. Every one of these is exactly representable in
// `f32`, so the constants are the specification's rationals rather than a
// rounding of them.
const PQ_M1: f32 = 2610.0 / 16384.0;
const PQ_M2: f32 = 2523.0 / 4096.0 * 128.0;
const PQ_C1: f32 = 3424.0 / 4096.0;
const PQ_C2: f32 = 2413.0 / 4096.0 * 32.0;
const PQ_C3: f32 = 2392.0 / 4096.0 * 32.0;

// ITU-R BT.2100, table 5.
const HLG_A: f32 = 0.178_832_77;
const HLG_B: f32 = 1.0 - 4.0 * HLG_A;
const HLG_C: f32 = 0.559_910_7;

/// A complete description of what a set of color components means.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorSpace {
    /// The primaries the components are expressed in.
    pub primaries: Primaries,
    /// The transfer function relating the encoded components to light.
    pub transfer: Transfer,
    /// The peak luminance this space is being presented at, in nits.
    ///
    /// For an absolute transfer function this is a property of the display
    /// rather than of the encoding, and it is what a display transform needs
    /// in order to tone map into it.
    pub peak_nits: f32,
}

impl ColorSpace {
    /// sRGB. What almost every color in almost every user interface written
    /// before this decade actually is.
    pub const SRGB: Self = Self {
        primaries: Primaries::Srgb,
        transfer: Transfer::Srgb,
        peak_nits: DIFFUSE_WHITE_NITS,
    };

    /// Display P3 with the sRGB transfer function, as used by Apple's
    /// displays and by CSS `color(display-p3 ...)`.
    pub const DISPLAY_P3: Self = Self {
        primaries: Primaries::DisplayP3,
        transfer: Transfer::Srgb,
        peak_nits: DIFFUSE_WHITE_NITS,
    };

    /// HDR10: Rec. 2020 primaries, PQ transfer, 1,000-nit mastering.
    pub const REC2020_PQ: Self = Self {
        primaries: Primaries::Rec2020,
        transfer: Transfer::Pq,
        peak_nits: 1_000.0,
    };

    /// Aura's compositing space: Rec. 2020 primaries, linear light, on the
    /// scene-referred scale described in this module's documentation.
    pub const COMPOSITING: Self = Self {
        primaries: Primaries::Rec2020,
        transfer: Transfer::Linear,
        peak_nits: DIFFUSE_WHITE_NITS,
    };

    /// The ACEScg working space.
    pub const ACESCG: Self = Self {
        primaries: Primaries::AcesCg,
        transfer: Transfer::Linear,
        peak_nits: DIFFUSE_WHITE_NITS,
    };

    /// Returns this space with a different peak luminance.
    #[must_use]
    pub const fn with_peak_nits(mut self, peak_nits: f32) -> Self {
        self.peak_nits = peak_nits;
        self
    }

    /// Decodes an encoded component into an absolute luminance, in nits.
    ///
    /// This is the unambiguous form: it does not matter which transfer
    /// function is in play, the answer is in cd/m².
    #[must_use]
    pub fn decode_to_nits(self, encoded: f32) -> f32 {
        let linear = self.transfer.decode(encoded);

        match self.transfer {
            Transfer::Pq => linear * PQ_PEAK_NITS,
            Transfer::Hlg => linear * self.peak_nits,
            Transfer::Linear | Transfer::Srgb => linear * DIFFUSE_WHITE_NITS,
        }
    }

    /// Encodes an absolute luminance, in nits, into a component.
    ///
    /// The exact inverse of [`ColorSpace::decode_to_nits`].
    #[must_use]
    pub fn encode_from_nits(self, nits: f32) -> f32 {
        let linear = match self.transfer {
            Transfer::Pq => nits / PQ_PEAK_NITS,
            Transfer::Hlg => nits / self.peak_nits,
            Transfer::Linear | Transfer::Srgb => nits / DIFFUSE_WHITE_NITS,
        };

        self.transfer.encode(linear)
    }

    /// Returns the matrix converting linear RGB in `self`'s primaries to
    /// linear RGB in `target`'s, chromatically adapting between their white
    /// points when they differ.
    ///
    /// Note that this operates on *linear* components. Decode first.
    #[must_use]
    pub fn conversion_matrix(self, target: Self) -> Mat3 {
        let source_white = self.primaries.white_point();
        let target_white = target.primaries.white_point();

        let adaptation = if source_white == target_white {
            Mat3::IDENTITY
        } else {
            bradford(source_white, target_white)
        };

        target.primaries.from_xyz() * adaptation * self.primaries.to_xyz()
    }

    /// Converts a linear RGB triple from `self`'s primaries into `target`'s.
    #[must_use]
    pub fn convert_linear(self, target: Self, linear: Vec3) -> Vec3 {
        self.conversion_matrix(target) * linear
    }
}

impl Default for ColorSpace {
    fn default() -> Self {
        Self::SRGB
    }
}

/// Returns the Bradford chromatic adaptation matrix taking XYZ under `from`'s
/// white to XYZ under `to`'s.
///
/// Bradford rather than von Kries or XYZ scaling because it is what ICC and
/// every color management stack in production actually use, so matching it
/// means matching what a designer sees in their tools.
#[must_use]
pub fn bradford(from: WhitePoint, to: WhitePoint) -> Mat3 {
    // The cone response domain of Lam (1985), as tabulated by the ICC.
    let cone = from_rows(
        [0.8951, 0.2664, -0.1614],
        [-0.7502, 1.7135, 0.0367],
        [0.0389, -0.0685, 1.0296],
    );

    let source = cone * from.xyz();
    let target = cone * to.xyz();

    let scale = Mat3::from_diagonal(target / source);

    cone.inverse() * scale * cone
}

/// Builds a [`Mat3`] from its rows.
///
/// `glam` is column-major and colorimetry literature is row-major, so every
/// matrix in this module is written the way its source writes it and
/// transposed exactly once, here.
fn from_rows(r0: [f32; 3], r1: [f32; 3], r2: [f32; 3]) -> Mat3 {
    Mat3::from_cols(
        Vec3::new(r0[0], r1[0], r2[0]),
        Vec3::new(r0[1], r1[1], r2[1]),
        Vec3::new(r0[2], r1[2], r2[2]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Asserts that a matrix matches a published one, written in row order.
    #[track_caller]
    fn assert_matrix(actual: Mat3, expected: [[f32; 3]; 3], tolerance: f32) {
        for (row, wanted) in expected.iter().enumerate() {
            for (column, want) in wanted.iter().enumerate() {
                let got = actual.col(column)[row];

                assert!(
                    (got - want).abs() < tolerance,
                    "[{row}][{column}]: got {got}, want {want}"
                );
            }
        }
    }

    // The three matrices below are published reference matrices. Deriving
    // ours from chromaticities and checking against them means a typo in a
    // primary is a test failure rather than a color that is subtly wrong
    // forever.
    //
    // Note that published matrices disagree with each other about which D65
    // expansion produced them: the Display P3 and Rec. 2020 matrices here
    // reproduce to 5e-8 from the specifications' D65, while widely circulated
    // sRGB matrices were derived from a longer expansion and land 2e-4 away.
    // The sRGB case below therefore checks against IEC 61966-2-1's own
    // printed matrix, which is the one the specification is willing to sign.

    #[test]
    fn srgb_to_xyz_matches_the_published_matrix() {
        assert_matrix(
            Primaries::Srgb.to_xyz(),
            [
                [0.4124, 0.3576, 0.1805],
                [0.2126, 0.7152, 0.0722],
                [0.0193, 0.1192, 0.9505],
            ],
            1e-4,
        );
    }

    #[test]
    fn rec2020_to_xyz_matches_the_published_matrix() {
        assert_matrix(
            Primaries::Rec2020.to_xyz(),
            [
                [0.636958, 0.1446169, 0.168881],
                [0.2627002, 0.6779981, 0.0593017],
                [0.0, 0.0280727, 1.0609851],
            ],
            1e-4,
        );
    }

    #[test]
    fn display_p3_to_xyz_matches_the_published_matrix() {
        assert_matrix(
            Primaries::DisplayP3.to_xyz(),
            [
                [0.4865709, 0.2656677, 0.1982173],
                [0.2289746, 0.6917385, 0.0792869],
                [0.0, 0.0451134, 1.0439444],
            ],
            1e-4,
        );
    }

    #[test]
    fn white_maps_to_the_white_point() {
        // Every set of primaries must take (1,1,1) to its own white. This is
        // the definition of the scaling factors, so if it fails the
        // derivation is wrong rather than imprecise.
        for primaries in [
            Primaries::Srgb,
            Primaries::DisplayP3,
            Primaries::Rec2020,
            Primaries::AcesCg,
        ] {
            let white = primaries.to_xyz() * Vec3::ONE;
            let expected = primaries.white_point().xyz();

            assert!(
                (white - expected).abs().max_element() < 1e-5,
                "{primaries:?}: got {white:?}, want {expected:?}"
            );
        }
    }

    #[test]
    fn xyz_round_trips_through_every_primary_set() {
        for primaries in [
            Primaries::Srgb,
            Primaries::DisplayP3,
            Primaries::Rec2020,
            Primaries::AcesCg,
        ] {
            let original = Vec3::new(0.2, 0.5, 0.8);
            let round_tripped = primaries.from_xyz() * (primaries.to_xyz() * original);

            assert!(
                (round_tripped - original).abs().max_element() < 1e-5,
                "{primaries:?}: {round_tripped:?}"
            );
        }
    }

    #[test]
    fn srgb_transfer_round_trips() {
        // Including both sides of the 0.0031308 piecewise join, which is
        // where an off-by-one in the threshold hides.
        for step in 0..=1000 {
            let encoded = step as f32 / 1000.0;
            let round_tripped = Transfer::Srgb.encode(Transfer::Srgb.decode(encoded));

            assert!(
                (round_tripped - encoded).abs() < 1e-5,
                "{encoded} became {round_tripped}"
            );
        }
    }

    #[test]
    fn srgb_transfer_matches_known_values() {
        // Mid-grey: sRGB 0.5 is famously ~0.214 in linear light, not 0.5.
        assert!((Transfer::Srgb.decode(0.5) - 0.214_041).abs() < 1e-4);
        assert!(Transfer::Srgb.decode(0.0).abs() < 1e-6);
        assert!((Transfer::Srgb.decode(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn srgb_transfer_agrees_with_the_legacy_color_conversion() {
        // `Color::into_linear` predates this module and is used by every
        // widget in the framework. The two must not disagree, or adopting
        // this module anywhere would silently shift every existing color.
        for step in 0..=255u8 {
            let component = f32::from(step) / 255.0;
            let legacy = crate::Color::from_rgb(component, component, component).into_linear()[0];
            let ours = Transfer::Srgb.decode(component);

            assert!(
                (legacy - ours).abs() < 1e-6,
                "{component}: legacy {legacy}, ours {ours}"
            );
        }
    }

    #[test]
    fn pq_round_trips() {
        for step in 0..=1000 {
            let encoded = step as f32 / 1000.0;
            let round_tripped = Transfer::Pq.encode(Transfer::Pq.decode(encoded));

            assert!(
                (round_tripped - encoded).abs() < 1e-4,
                "{encoded} became {round_tripped}"
            );
        }
    }

    #[test]
    fn pq_matches_the_specification_anchors() {
        // ST 2084 defines 1.0 as 10,000 nits and 0.0 as zero.
        assert!((Transfer::Pq.decode(1.0) - 1.0).abs() < 1e-4);
        assert!(Transfer::Pq.decode(0.0).abs() < 1e-6);

        // The well-known midpoint: PQ code 0.5 is about 92 nits, which is why
        // an HDR signal looks dim if you mistake it for sRGB.
        let nits = Transfer::Pq.decode(0.5) * PQ_PEAK_NITS;
        assert!((nits - 92.245).abs() < 0.5, "PQ 0.5 decoded to {nits} nits");
    }

    #[test]
    fn hlg_round_trips() {
        for step in 0..=1000 {
            let encoded = step as f32 / 1000.0;
            let round_tripped = Transfer::Hlg.encode(Transfer::Hlg.decode(encoded));

            assert!(
                (round_tripped - encoded).abs() < 1e-4,
                "{encoded} became {round_tripped}"
            );
        }
    }

    #[test]
    fn hlg_matches_the_specification_anchors() {
        // BT.2100: the piecewise join sits at scene light 1/12, encoded 0.5.
        assert!((Transfer::Hlg.encode(1.0 / 12.0) - 0.5).abs() < 1e-4);
        assert!((Transfer::Hlg.encode(1.0) - 1.0).abs() < 1e-3);
        assert!(Transfer::Hlg.encode(0.0).abs() < 1e-6);
    }

    #[test]
    fn diffuse_white_is_the_scene_referred_unit() {
        assert!((nits_to_scene(DIFFUSE_WHITE_NITS) - 1.0).abs() < 1e-6);
        assert!((scene_to_nits(1.0) - DIFFUSE_WHITE_NITS).abs() < 1e-6);

        // The whole point of the exercise: a 1,000-nit highlight is a legal
        // scene-referred value, and it is not 1.0.
        assert!((nits_to_scene(1_000.0) - 4.926_108).abs() < 1e-3);
    }

    #[test]
    fn absolute_luminance_round_trips_in_every_space() {
        for space in [
            ColorSpace::SRGB,
            ColorSpace::DISPLAY_P3,
            ColorSpace::REC2020_PQ,
            ColorSpace::COMPOSITING,
        ] {
            let nits = 100.0;
            let round_tripped = space.decode_to_nits(space.encode_from_nits(nits));

            assert!(
                (round_tripped - nits).abs() < 0.1,
                "{space:?}: {nits} became {round_tripped}"
            );
        }
    }

    #[test]
    fn srgb_diffuse_white_decodes_to_the_reference_level() {
        assert!((ColorSpace::SRGB.decode_to_nits(1.0) - DIFFUSE_WHITE_NITS).abs() < 1e-3);
    }

    #[test]
    fn a_conversion_to_the_same_space_is_the_identity() {
        for space in [
            ColorSpace::SRGB,
            ColorSpace::DISPLAY_P3,
            ColorSpace::REC2020_PQ,
            ColorSpace::ACESCG,
        ] {
            let matrix = space.conversion_matrix(space);

            assert!(
                (matrix - Mat3::IDENTITY)
                    .to_cols_array()
                    .iter()
                    .all(|component| component.abs() < 1e-5),
                "{space:?} did not convert to itself"
            );
        }
    }

    #[test]
    fn primary_conversions_round_trip() {
        let original = Vec3::new(0.25, 0.6, 0.9);

        for target in [
            ColorSpace::DISPLAY_P3,
            ColorSpace::REC2020_PQ,
            ColorSpace::ACESCG,
        ] {
            let there = ColorSpace::SRGB.convert_linear(target, original);
            let back = target.convert_linear(ColorSpace::SRGB, there);

            assert!(
                (back - original).abs().max_element() < 1e-4,
                "{target:?}: {original:?} became {back:?}"
            );
        }
    }

    #[test]
    fn srgb_is_a_subset_of_rec2020() {
        // Any in-gamut sRGB color must be representable in Rec. 2020 without
        // a negative component. The converse is false, which is the point of
        // compositing in the wider space.
        for red in 0..=4u8 {
            for green in 0..=4u8 {
                for blue in 0..=4u8 {
                    let source = Vec3::new(
                        f32::from(red) / 4.0,
                        f32::from(green) / 4.0,
                        f32::from(blue) / 4.0,
                    );

                    let wide = ColorSpace::SRGB.convert_linear(ColorSpace::COMPOSITING, source);

                    assert!(
                        wide.min_element() > -1e-4,
                        "{source:?} left the Rec. 2020 gamut: {wide:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn wide_gamut_conversion_actually_moves_a_saturated_color() {
        // A regression guard with teeth: if `conversion_matrix` were ever
        // stubbed to the identity, every round-trip test above would still
        // pass. Pure sRGB red is a long way from pure Rec. 2020 red.
        let red = Vec3::new(1.0, 0.0, 0.0);
        let wide = ColorSpace::SRGB.convert_linear(ColorSpace::COMPOSITING, red);

        assert!(
            wide.x < 0.75 && wide.y > 0.01,
            "sRGB red should desaturate into Rec. 2020, got {wide:?}"
        );
    }

    #[test]
    fn bradford_adapts_white_to_white() {
        // The defining property: the source white must land exactly on the
        // target white, or every conversion across white points is tinted.
        for (from, to) in [
            (WhitePoint::D65, WhitePoint::D50),
            (WhitePoint::D50, WhitePoint::D65),
            (WhitePoint::D65, WhitePoint::D60),
            (WhitePoint::D60, WhitePoint::D65),
        ] {
            let adapted = bradford(from, to) * from.xyz();
            let expected = to.xyz();

            assert!(
                (adapted - expected).abs().max_element() < 1e-5,
                "{from:?}->{to:?}: got {adapted:?}, want {expected:?}"
            );
        }
    }

    #[test]
    fn bradford_to_the_same_white_is_the_identity() {
        let matrix = bradford(WhitePoint::D65, WhitePoint::D65);

        assert!(
            (matrix - Mat3::IDENTITY)
                .to_cols_array()
                .iter()
                .all(|component| component.abs() < 1e-5)
        );
    }

    #[test]
    fn negative_and_extreme_inputs_stay_finite() {
        // Scene-referred math produces out-of-range intermediates constantly.
        // A transfer function that returns NaN for one of them poisons an
        // entire frame, and it will be a wide-gamut conversion that puts it
        // there rather than anything the application wrote.
        for transfer in [
            Transfer::Linear,
            Transfer::Srgb,
            Transfer::Pq,
            Transfer::Hlg,
        ] {
            for value in [-1.0, -0.001, 0.0, 1.0, 4.9, 100.0] {
                assert!(
                    transfer.decode(value).is_finite(),
                    "{transfer:?}.decode({value}) was not finite"
                );
                assert!(
                    transfer.encode(value).is_finite(),
                    "{transfer:?}.encode({value}) was not finite"
                );
            }
        }
    }
}

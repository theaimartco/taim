//! Display transforms: turning scene-referred light into panel signals.
//!
//! Everything upstream of this module works in unbounded, scene-referred
//! light: Rec. 2020 primaries, linear, `1.0` at
//! [`DIFFUSE_WHITE_NITS`](super::space::DIFFUSE_WHITE_NITS). That is the right
//! space to composite in and the wrong space to send anywhere, because no
//! panel accepts it.
//!
//! A [`DisplayTransform`] is the last thing that happens to a frame. It does
//! three jobs, in this order, and the order is not negotiable:
//!
//! 1. **Tone map** the luminance into what the panel can emit.
//! 2. **Gamut map** the chromaticity into what the panel can show.
//! 3. **Encode** with the panel's transfer function.
//!
//! Doing these in any other order produces hue shifts that look like a bug in
//! the design system rather than a bug in the renderer, which is a
//! spectacularly expensive way to lose a week.
//!
//! # Why this is a separate step and not folded into compositing
//!
//! It is lossy and it is panel-specific. Two windows of the same application
//! on two different monitors need two different display transforms over one
//! identical scene. Folding it into the compositing math would bake one
//! panel's limits into the scene and make the second monitor wrong.

use crate::color::Color;
use crate::color::radiance::Radiance;
use crate::color::space::{self, ColorSpace, Transfer};

use glam::Vec3;

/// A panel, described well enough to send it a correct signal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    /// The color space the panel accepts, including its peak luminance.
    pub color_space: ColorSpace,
    /// The panel's black level in nits.
    ///
    /// Zero for OLED. A backlit panel that cannot reach zero wants its real
    /// value here, because the tone mapper lifts shadows to match it and
    /// crushing them to a black the panel cannot produce loses detail for
    /// nothing.
    pub black_nits: f32,

    /// The luminance the panel emits for a signal of `1.0`, in nits.
    ///
    /// This is what separates a *display-referred* surface from an *extended*
    /// one, and the two need opposite arithmetic:
    ///
    /// - An sRGB monitor is display-referred. `1.0` is its peak, which is
    ///   also its diffuse white, so reference white equals peak.
    /// - A macOS EDR or scRGB surface is extended. `1.0` is SDR reference
    ///   white and the panel's headroom lives *above* `1.0`, so reference
    ///   white is far below peak.
    ///
    /// Encoding relative to peak on an extended surface puts diffuse white at
    /// `203 / 1600 == 0.127` and renders the whole interface at an eighth of
    /// its intended brightness. Unused when the transfer function is absolute
    /// (PQ carries its own luminance).
    pub reference_white_nits: f32,
}

impl Display {
    /// An ordinary sRGB monitor.
    pub const SDR: Self = Self {
        color_space: ColorSpace::SRGB,
        black_nits: 0.1,
        reference_white_nits: space::DIFFUSE_WHITE_NITS,
    };

    /// An HDR10 panel mastered at 1,000 nits.
    pub const HDR10_1000: Self = Self {
        color_space: ColorSpace::REC2020_PQ,
        black_nits: 0.0,
        reference_white_nits: space::DIFFUSE_WHITE_NITS,
    };

    /// A wide-gamut panel that accepts extended linear signals, which is how
    /// macOS and iOS present HDR to a layer.
    pub const P3_EXTENDED: Self = Self {
        color_space: ColorSpace {
            primaries: space::Primaries::DisplayP3,
            transfer: Transfer::Linear,
            peak_nits: 1_600.0,
        },
        black_nits: 0.0,
        reference_white_nits: space::DIFFUSE_WHITE_NITS,
    };

    /// Returns this display's peak luminance in nits.
    #[must_use]
    pub const fn peak_nits(self) -> f32 {
        self.color_space.peak_nits
    }

    /// Returns whether this display can show more light than SDR diffuse
    /// white.
    #[must_use]
    pub fn is_hdr(self) -> bool {
        self.peak_nits() > space::DIFFUSE_WHITE_NITS * 1.5
    }

    /// Encodes an absolute luminance into a signal this panel understands.
    ///
    /// Absolute transfer functions carry their own luminance and are encoded
    /// directly. Everything else is relative to
    /// [`Display::reference_white_nits`] -- *not* to peak. See that field.
    #[must_use]
    pub fn encode_nits(self, nits: f32) -> f32 {
        if self.color_space.transfer.is_absolute() {
            return self.color_space.encode_from_nits(nits);
        }

        let reference = self.reference_white_nits.max(f32::MIN_POSITIVE);

        self.color_space.transfer.encode(nits / reference)
    }

    /// Decodes a panel signal back into an absolute luminance.
    ///
    /// The exact inverse of [`Display::encode_nits`].
    #[must_use]
    pub fn decode_nits(self, signal: f32) -> f32 {
        if self.color_space.transfer.is_absolute() {
            return self.color_space.decode_to_nits(signal);
        }

        self.color_space.transfer.decode(signal) * self.reference_white_nits
    }
}

/// How luminance beyond the panel's reach should be handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToneMap {
    /// Compress with the ITU-R BT.2390 EETF.
    ///
    /// A Hermite roll-off applied in PQ space, so the compression is spread
    /// perceptually rather than linearly. Below the knee it is *exactly* the
    /// identity, which is the property that matters most here: ordinary user
    /// interface content at diffuse white must reach the panel untouched. A
    /// tone mapper that dims a white page because a highlight exists
    /// elsewhere on screen has made the whole interface worse in order to
    /// render one pixel correctly.
    #[default]
    Bt2390,
    /// Clip. Faster, and correct only when the scene never exceeds the panel.
    Clip,
}

/// How chromaticity outside the panel's gamut should be handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GamutMap {
    /// Desaturate toward the same luminance until the color fits.
    ///
    /// Preserves how bright a color looks and gives up some of how colorful
    /// it looks, which is the trade every viewer prefers and the opposite of
    /// what clipping does.
    #[default]
    Desaturate,
    /// Clamp each component independently.
    ///
    /// Cheap, and it shifts hue: clipping a saturated red's green channel up
    /// and leaving blue alone moves the color, and it moves different colors
    /// by different amounts.
    Clip,
}

/// The final stage of the pipeline: scene-referred light in, panel signal out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayTransform {
    display: Display,
    tone_map: ToneMap,
    gamut_map: GamutMap,
    /// The brightest luminance the scene is allowed to contain, in nits.
    ///
    /// This is the tone mapper's input range. It is a property of the content
    /// rather than of the panel.
    source_peak_nits: f32,
}

impl DisplayTransform {
    /// Creates a transform targeting `display`, assuming the scene fits
    /// inside it.
    ///
    /// The default source peak is the *panel's* peak, not the PQ container's
    /// 10,000 nits, and the difference is not academic. BT.2390 places its
    /// knee relative to the source range, so declaring a 10,000-nit source
    /// puts an ordinary 203-nit white above the knee and rolls it off:
    /// pointing a default transform at an sRGB monitor encoded diffuse white
    /// at 0.78 rather than 1.0, dimming every interface by 22% to leave
    /// headroom for content that was not there.
    ///
    /// A caller whose scene really is brighter than the panel must say so
    /// with [`DisplayTransform::with_source_peak_nits`]. The renderer knows
    /// the brightest element it drew, so it can say so exactly, per frame,
    /// and get a curve fitted to the content instead of to a guess.
    #[must_use]
    pub fn new(display: Display) -> Self {
        Self {
            display,
            tone_map: ToneMap::default(),
            gamut_map: GamutMap::default(),
            source_peak_nits: display.peak_nits(),
        }
    }

    /// Sets the brightest luminance the scene contains.
    ///
    /// Telling the transform the truth about the content is worth real image
    /// quality in both directions. A scene whose brightest pixel is 600 nits,
    /// presented on a 600-nit panel, needs no compression at all, and saying
    /// so keeps the roll-off out of the picture entirely. A scene containing
    /// a 4,000-nit highlight on that same panel needs a curve, and without
    /// this call it will clip instead.
    #[must_use]
    pub const fn with_source_peak_nits(mut self, nits: f32) -> Self {
        self.source_peak_nits = nits;
        self
    }

    /// Sets the tone mapping operator.
    #[must_use]
    pub const fn with_tone_map(mut self, tone_map: ToneMap) -> Self {
        self.tone_map = tone_map;
        self
    }

    /// Sets the gamut mapping operator.
    #[must_use]
    pub const fn with_gamut_map(mut self, gamut_map: GamutMap) -> Self {
        self.gamut_map = gamut_map;
        self
    }

    /// Returns the display this transform targets.
    #[must_use]
    pub const fn display(&self) -> Display {
        self.display
    }

    /// Returns the brightest luminance the scene is declared to contain.
    #[must_use]
    pub const fn source_peak_nits(&self) -> f32 {
        self.source_peak_nits
    }

    /// Returns the tone mapping operator in use.
    #[must_use]
    pub const fn tone_map(&self) -> ToneMap {
        self.tone_map
    }

    /// Returns the gamut mapping operator in use.
    #[must_use]
    pub const fn gamut_map(&self) -> GamutMap {
        self.gamut_map
    }

    /// Returns the luminance vector of a set of primaries.
    ///
    /// Row 1 of their RGB->XYZ matrix. Exposed because a GPU implementation
    /// of this transform needs the same vector, and deriving it twice is how
    /// the two implementations drift apart.
    #[must_use]
    pub fn luminance_vector(primaries: space::Primaries) -> Vec3 {
        let to_xyz = primaries.to_xyz();

        Vec3::new(to_xyz.x_axis.y, to_xyz.y_axis.y, to_xyz.z_axis.y)
    }

    /// Returns the matrix taking compositing-space linear light into the
    /// target display's primaries.
    #[must_use]
    pub fn conversion_matrix(&self) -> glam::Mat3 {
        ColorSpace::COMPOSITING.conversion_matrix(self.display.color_space)
    }

    /// Applies the transform, returning components encoded for the panel.
    ///
    /// Alpha passes through untouched: it is coverage, not light, and nothing
    /// about a panel's luminance range has an opinion about it.
    #[must_use]
    pub fn apply(&self, radiance: Radiance) -> [f32; 4] {
        let toned = self.apply_tone_map(radiance.components());
        let mapped = self.apply_gamut_map(toned);

        let encoded = |component: f32| self.display.encode_nits(space::scene_to_nits(component));

        [
            encoded(mapped.x),
            encoded(mapped.y),
            encoded(mapped.z),
            radiance.alpha,
        ]
    }

    /// Applies the transform and packs the result into a [`Color`].
    ///
    /// Lossy for any panel wider or brighter than sRGB; useful for tests,
    /// screenshots, and paths that still speak in [`Color`].
    pub fn apply_to_color(&self, radiance: Radiance) -> Color {
        let [r, g, b, a] = self.apply(radiance);

        Color {
            r: r.clamp(0.0, 1.0),
            g: g.clamp(0.0, 1.0),
            b: b.clamp(0.0, 1.0),
            a,
        }
    }

    /// Tone maps a scene-referred triple, preserving its chromaticity.
    fn apply_tone_map(&self, scene: Vec3) -> Vec3 {
        let luminance = luminance_of(ColorSpace::COMPOSITING.primaries, scene);

        if luminance <= 0.0 {
            return scene;
        }

        let mapped_nits = match self.tone_map {
            ToneMap::Clip => space::scene_to_nits(luminance).min(self.display.peak_nits()),
            ToneMap::Bt2390 => self.eetf(space::scene_to_nits(luminance)),
        };

        // Scale the whole triple by the luminance ratio rather than tone
        // mapping each channel. Per-channel compression desaturates bright
        // colors toward white, which is why so much HDR content looks washed
        // out at the top end.
        scene * (space::nits_to_scene(mapped_nits) / luminance)
    }

    /// The ITU-R BT.2390 electro-optical transfer function adjustment.
    ///
    /// Operates entirely in PQ space, which is what makes the roll-off
    /// perceptually even rather than bunched up at the top.
    fn eetf(&self, nits: f32) -> f32 {
        let source_black = pq_from_nits(0.0);
        let source_peak = pq_from_nits(self.source_peak_nits);
        let range = source_peak - source_black;

        if range <= f32::EPSILON {
            return nits.min(self.display.peak_nits());
        }

        let max_luminance = (pq_from_nits(self.display.peak_nits()) - source_black) / range;
        let min_luminance = (pq_from_nits(self.display.black_nits) - source_black) / range;

        let normalized = (pq_from_nits(nits) - source_black) / range;

        // The knee. When the panel can already show the source peak this
        // lands at or above 1.0, every sample takes the identity branch, and
        // the transform costs nothing but a comparison.
        let knee = 1.5 * max_luminance - 0.5;

        let rolled = if normalized < knee || knee >= 1.0 {
            normalized
        } else {
            let t = (normalized - knee) / (1.0 - knee);
            let t2 = t * t;
            let t3 = t2 * t;

            (2.0 * t3 - 3.0 * t2 + 1.0) * knee
                + (t3 - 2.0 * t2 + t) * (1.0 - knee)
                + (-2.0 * t3 + 3.0 * t2) * max_luminance
        };

        // Lift the floor to the panel's real black.
        let lifted = rolled + min_luminance * (1.0 - rolled).powi(4);

        // Clamp to what the panel can physically emit. The black-level lift
        // can push the top of the curve a fraction above the peak, and a
        // caller who under-declares the source range gets values straight
        // through the identity branch. Neither should be able to hand the
        // encoder a luminance the panel cannot produce.
        nits_from_pq(lifted * range + source_black).clamp(0.0, self.display.peak_nits())
    }

    /// Converts into the panel's primaries and brings the result into gamut.
    fn apply_gamut_map(&self, scene: Vec3) -> Vec3 {
        let converted = ColorSpace::COMPOSITING.convert_linear(self.display.color_space, scene);

        let ceiling = space::nits_to_scene(self.display.peak_nits());

        match self.gamut_map {
            GamutMap::Clip => converted.clamp(Vec3::ZERO, Vec3::splat(ceiling)),
            GamutMap::Desaturate => {
                desaturate_into(converted, ceiling, self.display.color_space.primaries)
            }
        }
    }
}

/// Returns the scene-referred luminance of a triple in `primaries`.
///
/// The basis matters: luminance coefficients differ by up to 5% between
/// sRGB, Display P3, and Rec. 2020, and gamut mapping runs *after* the
/// conversion into the panel's primaries. Using the compositing space's
/// coefficients there would preserve the wrong quantity.
fn luminance_of(primaries: space::Primaries, color: Vec3) -> f32 {
    // Row 1 of the RGB->XYZ matrix is the luminance vector by construction,
    // so this follows the primaries rather than restating their coefficients.
    (primaries.to_xyz() * color).y
}

/// Desaturates `color` toward its own luminance until every component fits in
/// `0.0..=ceiling`.
///
/// Finds the largest blend toward grey that fits, in closed form, rather than
/// iterating: the constraint is linear in the blend factor, so each component
/// gives one bound and the answer is their minimum.
fn desaturate_into(color: Vec3, ceiling: f32, primaries: space::Primaries) -> Vec3 {
    if color.min_element() >= 0.0 && color.max_element() <= ceiling {
        return color;
    }

    let grey = luminance_of(primaries, color).clamp(0.0, ceiling);
    let offset = color - Vec3::splat(grey);

    let mut factor = 1.0f32;

    for axis in 0..3 {
        let delta = offset[axis];

        // A component that does not move cannot be brought into range by
        // blending, and dividing by it would produce an infinity.
        if delta.abs() <= f32::EPSILON {
            continue;
        }

        let bound = if delta > 0.0 {
            (ceiling - grey) / delta
        } else {
            -grey / delta
        };

        factor = factor.min(bound.max(0.0));
    }

    // Clamp defensively: floating point can leave a component a hair outside
    // even after an exact solve, and a hair outside is still a validation
    // error on some backends.
    (Vec3::splat(grey) + offset * factor).clamp(Vec3::ZERO, Vec3::splat(ceiling))
}

/// Encodes an absolute luminance as a PQ signal.
fn pq_from_nits(nits: f32) -> f32 {
    Transfer::Pq.encode(nits / space::PQ_PEAK_NITS)
}

/// Decodes a PQ signal into an absolute luminance.
fn nits_from_pq(signal: f32) -> f32 {
    Transfer::Pq.decode(signal) * space::PQ_PEAK_NITS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scene-referred value of an ordinary white surface.
    fn diffuse_white() -> Radiance {
        Radiance::from(Color::WHITE)
    }

    /// A highlight far beyond anything sRGB can express.
    fn highlight(nits: f32) -> Radiance {
        Radiance::from(Color::WHITE).at_nits(nits)
    }

    #[test]
    fn diffuse_white_reaches_an_hdr_panel_untouched() {
        // The single most important property in this module. Ordinary
        // interface content must not dim because a bright highlight exists
        // somewhere else in the scene. Every HDR user interface that looks
        // washed out or murky has got this wrong.
        let transform = DisplayTransform::new(Display::HDR10_1000);
        let [red, ..] = transform.apply(diffuse_white());

        let reached = Display::HDR10_1000.decode_nits(red);

        assert!(
            (reached - space::DIFFUSE_WHITE_NITS).abs() < 2.0,
            "diffuse white reached the panel at {reached} nits"
        );
    }

    #[test]
    fn diffuse_white_is_full_scale_on_an_sdr_panel() {
        let transform = DisplayTransform::new(Display::SDR);
        let white = transform.apply_to_color(diffuse_white());

        assert!(
            (white.r - 1.0).abs() < 0.02,
            "sRGB white encoded to {}",
            white.r
        );
    }

    #[test]
    fn a_thousand_nit_highlight_survives_on_a_thousand_nit_panel() {
        // The gate this pillar exists for, in numeric form.
        let transform = DisplayTransform::new(Display::HDR10_1000).with_source_peak_nits(1_000.0);

        let [red, ..] = transform.apply(highlight(1_000.0));
        let reached = Display::HDR10_1000.decode_nits(red);

        assert!(
            (reached - 1_000.0).abs() < 10.0,
            "a 1000-nit highlight reached the panel at {reached} nits"
        );

        // And it must be dramatically brighter than diffuse white, or the
        // whole exercise bought nothing.
        let [white, ..] = transform.apply(diffuse_white());
        assert!(
            red > white,
            "the highlight ({red}) did not encode brighter than white ({white})"
        );
    }

    #[test]
    fn an_sdr_panel_compresses_rather_than_producing_nonsense() {
        let transform = DisplayTransform::new(Display::SDR);
        let clipped = transform.apply_to_color(highlight(4_000.0));

        assert!(clipped.r.is_finite() && clipped.r <= 1.0 && clipped.r > 0.9);
    }

    #[test]
    fn tone_mapping_is_monotonic() {
        // A tone curve that ever decreases inverts contrast somewhere, which
        // shows up as a visible band rather than as a subtle error.
        let transform = DisplayTransform::new(Display::HDR10_1000);

        let mut previous = -1.0;

        for step in 0..=400u16 {
            let nits = f32::from(step) * 25.0;
            let mapped = transform.eetf(nits);

            assert!(
                mapped >= previous - 1e-3,
                "tone curve went backwards at {nits} nits: {mapped} after {previous}"
            );

            previous = mapped;
        }
    }

    #[test]
    fn tone_mapping_never_exceeds_the_panel() {
        let transform = DisplayTransform::new(Display::HDR10_1000);

        for step in 0..=400u16 {
            let nits = f32::from(step) * 25.0;
            let mapped = transform.eetf(nits);

            assert!(
                mapped <= Display::HDR10_1000.peak_nits() + 1.0,
                "{nits} nits mapped to {mapped}, above the panel's peak"
            );
        }
    }

    #[test]
    fn a_panel_that_covers_the_source_applies_no_curve_at_all() {
        // When the knee lands at or above 1.0 every sample takes the identity
        // branch. This is what keeps the transform free for the overwhelming
        // majority of frames, which contain nothing bright.
        let transform = DisplayTransform::new(Display::HDR10_1000).with_source_peak_nits(1_000.0);

        for nits in [0.0, 1.0, 100.0, 203.0, 500.0, 999.0] {
            let mapped = transform.eetf(nits);

            assert!(
                (mapped - nits).abs() < 1.0,
                "{nits} nits should pass through, became {mapped}"
            );
        }
    }

    #[test]
    fn black_stays_black_on_a_panel_that_can_reach_it() {
        let transform = DisplayTransform::new(Display::HDR10_1000);
        let [red, green, blue, alpha] = transform.apply(Radiance::BLACK);

        assert!(red.abs() < 1e-3 && green.abs() < 1e-3 && blue.abs() < 1e-3);
        assert!((alpha - 1.0).abs() < 1e-6);
    }

    #[test]
    fn shadows_lift_to_a_panel_that_cannot_reach_black() {
        // `Display::SDR` cannot do better than 0.1 nits, so crushing to zero
        // would throw away shadow detail the panel could have shown.
        let transform = DisplayTransform::new(Display::SDR);
        let lifted = transform.eetf(0.0);

        assert!(
            (0.0..1.0).contains(&lifted),
            "black lifted to {lifted} nits"
        );
    }

    #[test]
    fn alpha_is_never_touched() {
        let transform = DisplayTransform::new(Display::HDR10_1000);

        for alpha in [0.0, 0.25, 0.5, 1.0] {
            let radiance = Radiance {
                alpha,
                ..highlight(2_000.0)
            };

            assert!((transform.apply(radiance)[3] - alpha).abs() < 1e-6);
        }
    }

    #[test]
    fn an_in_gamut_color_is_left_alone_by_desaturation() {
        let inside = Vec3::new(0.2, 0.5, 0.8);

        assert_eq!(desaturate_into(inside, 1.0, space::Primaries::Srgb), inside);
    }

    #[test]
    fn desaturation_brings_a_color_into_gamut() {
        // A negative component is the signature of a color outside the
        // target's gamut, and it is a validation error on the way to a
        // texture, not merely a wrong color.
        for outside in [
            Vec3::new(1.4, -0.2, 0.3),
            Vec3::new(-0.5, 1.2, 1.9),
            Vec3::new(2.0, 2.0, -1.0),
        ] {
            let fitted = desaturate_into(outside, 1.0, space::Primaries::Srgb);

            assert!(
                fitted.min_element() >= -1e-5 && fitted.max_element() <= 1.0 + 1e-5,
                "{outside:?} desaturated to {fitted:?}, still out of gamut"
            );
        }
    }

    #[test]
    fn desaturation_preserves_luminance_where_it_can() {
        // The trade this operator makes: give up colorfulness, keep
        // brightness. If it moved luminance too, a gamut-clipped accent would
        // also change its apparent weight in the layout.
        let outside = Vec3::new(1.0, -0.15, 0.4);
        let fitted = desaturate_into(outside, 1.0, space::Primaries::Srgb);

        let wanted = luminance_of(space::Primaries::Srgb, outside).clamp(0.0, 1.0);
        let got = luminance_of(space::Primaries::Srgb, fitted);

        assert!(
            (got - wanted).abs() < 1e-3,
            "luminance moved from {wanted} to {got}"
        );
    }

    #[test]
    fn clipping_and_desaturating_disagree_and_both_stay_in_gamut() {
        // A guard against either operator being silently stubbed to the
        // other: they must produce different pixels for an out-of-gamut input.
        let scene = Radiance::from_color_in(Color::from_rgb(1.0, 0.0, 0.0), ColorSpace::DISPLAY_P3);

        let clipped = DisplayTransform::new(Display::SDR)
            .with_gamut_map(GamutMap::Clip)
            .apply_to_color(scene);
        let desaturated = DisplayTransform::new(Display::SDR)
            .with_gamut_map(GamutMap::Desaturate)
            .apply_to_color(scene);

        assert!(
            (clipped.g - desaturated.g).abs() > 1e-3,
            "clip and desaturate produced the same green: {clipped:?}"
        );
    }

    #[test]
    fn every_display_survives_every_extreme_without_a_nan() {
        // Scene-referred math produces absurd intermediates. One NaN reaching
        // a texture is a frame of garbage, and it will have come from the
        // last stage in the pipeline where nobody is looking.
        for display in [Display::SDR, Display::HDR10_1000, Display::P3_EXTENDED] {
            for tone_map in [ToneMap::Bt2390, ToneMap::Clip] {
                for gamut_map in [GamutMap::Desaturate, GamutMap::Clip] {
                    let transform = DisplayTransform::new(display)
                        .with_tone_map(tone_map)
                        .with_gamut_map(gamut_map);

                    for scene in [
                        Radiance::BLACK,
                        Radiance::TRANSPARENT,
                        Radiance::new(-1.0, 0.5, 2.0, 1.0),
                        Radiance::new(1e6, 1e6, 1e6, 1.0),
                        Radiance::new(1e-9, 0.0, 0.0, 1.0),
                    ] {
                        let output = transform.apply(scene);

                        assert!(
                            output.iter().all(|c| c.is_finite()),
                            "{display:?} {tone_map:?} {gamut_map:?} on {scene:?} gave {output:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn an_hdr_panel_is_recognised_as_one() {
        assert!(Display::HDR10_1000.is_hdr());
        assert!(Display::P3_EXTENDED.is_hdr());
        assert!(!Display::SDR.is_hdr());
    }
}

#[cfg(test)]
mod default_range_tests {
    use super::*;

    #[test]
    fn the_default_assumes_the_scene_fits_the_panel() {
        // Pins the fix for a real defect: defaulting the source peak to the
        // PQ container's 10,000 nits put ordinary white above BT.2390's knee
        // and rolled it off, encoding diffuse white at 0.78 instead of 1.0 --
        // a 22% dimming of every interface on every SDR monitor, to leave
        // headroom for content that was not in the scene.
        for display in [Display::SDR, Display::HDR10_1000, Display::P3_EXTENDED] {
            let transform = DisplayTransform::new(display);

            assert!(
                (transform.eetf(display.peak_nits()) - display.peak_nits()).abs()
                    < display.peak_nits() * 0.01,
                "{display:?} rolled off its own peak by default"
            );
        }
    }

    #[test]
    fn declaring_a_brighter_source_does_engage_the_curve() {
        // The other half: once a caller admits the scene exceeds the panel,
        // compression must actually happen, or `with_source_peak_nits` is
        // decoration.
        let transform = DisplayTransform::new(Display::SDR).with_source_peak_nits(4_000.0);

        let peak = Display::SDR.peak_nits();

        assert!(transform.eetf(2_000.0) <= peak + 1e-3);

        // And it must be a curve rather than a clamp: a mid-tone well below
        // the panel's peak is pulled down too, which is what spreads the
        // compression perceptually instead of flattening only the top.
        let mid = transform.eetf(150.0);

        assert!(
            mid < 150.0 && mid > 0.0,
            "a 150-nit mid-tone came through at {mid}, so nothing was compressed"
        );
    }
}

#[cfg(test)]
mod encoding_tests {
    use super::*;

    /// Every display this module ships, so a new one cannot skip the rules.
    const ALL: &[Display] = &[Display::SDR, Display::HDR10_1000, Display::P3_EXTENDED];

    #[test]
    fn diffuse_white_reaches_every_panel_as_diffuse_white() {
        // The invariant that catches an entire class of bug, and the one that
        // was missing when `apply` encoded relative to peak: on an extended
        // surface that put white at 203/1600 == 0.127 and rendered the whole
        // interface at an eighth of its brightness. Every existing test still
        // passed, because none of them asked what a panel actually received.
        for display in ALL {
            let transform = DisplayTransform::new(*display);
            let [red, ..] = transform.apply(Radiance::from(Color::WHITE));

            let reached = display.decode_nits(red);

            assert!(
                (reached - space::DIFFUSE_WHITE_NITS).abs() < 3.0,
                "{display:?} received diffuse white as {reached} nits"
            );
        }
    }

    #[test]
    fn encoding_round_trips_on_every_panel() {
        for display in ALL {
            for nits in [0.0, 1.0, 50.0, space::DIFFUSE_WHITE_NITS, 500.0] {
                if nits > display.peak_nits() {
                    continue;
                }

                let round_tripped = display.decode_nits(display.encode_nits(nits));

                assert!(
                    (round_tripped - nits).abs() < nits.max(1.0) * 0.01,
                    "{display:?}: {nits} nits round-tripped to {round_tripped}"
                );
            }
        }
    }

    #[test]
    fn an_extended_surface_puts_headroom_above_one() {
        // The defining property of an extended-linear surface, and what makes
        // it different from a display-referred one: 1.0 is SDR white, and a
        // brighter highlight is a number greater than 1.0 rather than a
        // clipped 1.0.
        let transform = DisplayTransform::new(Display::P3_EXTENDED).with_source_peak_nits(1_000.0);

        let [white, ..] = transform.apply(Radiance::from(Color::WHITE));
        let [bright, ..] = transform.apply(Radiance::from(Color::WHITE).at_nits(1_000.0));

        assert!(
            (white - 1.0).abs() < 0.02,
            "SDR white should encode to 1.0, got {white}"
        );
        assert!(
            bright > 4.0,
            "a 1000-nit highlight should encode well above 1.0, got {bright}"
        );
    }

    #[test]
    fn a_display_referred_surface_keeps_everything_at_or_below_one() {
        // The mirror image: an sRGB monitor has no headroom, so nothing may
        // encode above full scale no matter how bright the scene was.
        let transform = DisplayTransform::new(Display::SDR).with_source_peak_nits(4_000.0);

        for nits in [1.0, 203.0, 1_000.0, 4_000.0] {
            let [red, ..] = transform.apply(Radiance::from(Color::WHITE).at_nits(nits));

            assert!(
                red <= 1.0 + 1e-3,
                "{nits} nits encoded to {red} on a display-referred surface"
            );
        }
    }
}

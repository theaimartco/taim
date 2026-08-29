//! Physically-grounded surface materials.
//!
//! Every "glass" surface shipping in a mainstream user interface today is the
//! same trick: sample what is behind, blur it, tint it, composite. Apple's
//! Liquid Glass, Windows Mica, and Flutter's `BackdropFilter` are all that
//! operation. It is a screen-space approximation with no thickness, no
//! refraction, no dispersion, and no consistent light source, and it stops
//! being convincing the moment something moves behind it: a real pane bends
//! what it transmits, and a blur does not.
//!
//! This module describes surfaces in terms a renderer can actually shade:
//! how rough they are, how thick, how much they bend light, and how their
//! reflectance varies with viewing angle. Those are the inputs to a lighting
//! pass rather than to a filter chain.
//!
//! # Why this is affordable here and not in a game
//!
//! Interface geometry is trivial — a few thousand quads and paths, no meshes,
//! no shadow maps, no global illumination. The per-pixel work a game spends
//! its entire budget on is nearly free at this scale, which is why a user
//! interface can afford shading a game would have to approximate.
//!
//! # Scope
//!
//! Pure functions over material parameters, and nothing about how they are
//! rasterised or stored. The G-buffer layout and the lighting pass that
//! consume these live in the renderer; keeping the physics here means it can
//! be tested against closed-form answers on any machine, with no GPU.

use crate::color::Radiance;

/// Index of refraction of air, near enough for an interface.
pub const IOR_AIR: f32 = 1.0;

/// Index of refraction of common soda-lime glass.
pub const IOR_GLASS: f32 = 1.52;

/// How a surface interacts with light.
///
/// Values are physical rather than artistic: `roughness` is a real microfacet
/// distribution width, `thickness_mm` is a real distance, `ior` is a real
/// refractive index. A designer picks a named [`Material`] constant; the
/// numbers exist so the renderer can shade rather than guess.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    /// Microfacet roughness, `0.0..=1.0`.
    ///
    /// `0.0` is a perfect mirror and `1.0` is fully diffuse. Values below
    /// about `0.03` alias badly under any finite sampling, so they are
    /// clamped rather than honoured.
    pub roughness: f32,

    /// Physical thickness in millimetres.
    ///
    /// Drives how far refraction displaces what is seen through the surface.
    /// Zero means a surface with no depth, which refracts nothing however
    /// high its index of refraction.
    pub thickness_mm: f32,

    /// Index of refraction.
    ///
    /// `1.0` does not bend light at all. Glass is about `1.52`.
    pub ior: f32,

    /// Difference in index of refraction between the red and blue ends of the
    /// spectrum.
    ///
    /// Real glass separates colour at its edges, and the absence of that is
    /// one of the reasons a blurred rectangle does not read as a pane. Zero
    /// disables the effect.
    pub dispersion: f32,

    /// How much light the surface transmits, `0.0..=1.0`.
    ///
    /// `0.0` is opaque; `1.0` transmits everything not reflected.
    pub transmission: f32,
}

impl Material {
    /// The roughness below which specular highlights alias rather than sharpen.
    pub const MIN_ROUGHNESS: f32 = 0.03;

    /// An ordinary opaque surface: a card, a sheet, a filled shape.
    pub const OPAQUE: Self = Self {
        roughness: 1.0,
        thickness_mm: 0.0,
        ior: IOR_AIR,
        dispersion: 0.0,
        transmission: 0.0,
    };

    /// Clear glass, 1.5 mm thick.
    pub const GLASS: Self = Self {
        roughness: 0.05,
        thickness_mm: 1.5,
        ior: IOR_GLASS,
        dispersion: 0.012,
        transmission: 0.92,
    };

    /// Etched or sandblasted glass: transmits, but scatters what it transmits.
    pub const FROSTED: Self = Self {
        roughness: 0.45,
        thickness_mm: 1.5,
        ior: IOR_GLASS,
        dispersion: 0.006,
        transmission: 0.78,
    };

    /// A polished, slightly reflective surface with no transmission.
    pub const POLISHED: Self = Self {
        roughness: 0.18,
        thickness_mm: 0.0,
        ior: 1.35,
        dispersion: 0.0,
        transmission: 0.0,
    };

    /// Returns this material with every parameter brought into a range the
    /// shading model is defined on.
    ///
    /// Materials are authored, and authored values arrive out of range. An
    /// index of refraction below one inverts refraction, a negative roughness
    /// produces a negative distribution, and both show as a black or exploded
    /// surface rather than as an error.
    #[must_use]
    pub fn sanitised(self) -> Self {
        Self {
            roughness: clamp_finite(self.roughness, Self::MIN_ROUGHNESS, 1.0, 1.0),
            thickness_mm: clamp_finite(self.thickness_mm, 0.0, 1_000.0, 0.0),
            ior: clamp_finite(self.ior, 1.0, 4.0, IOR_AIR),
            dispersion: clamp_finite(self.dispersion, 0.0, 0.5, 0.0),
            transmission: clamp_finite(self.transmission, 0.0, 1.0, 0.0),
        }
    }

    /// Reflectance at normal incidence, the `F0` term of a Fresnel model.
    ///
    /// Derived from the index of refraction rather than authored separately,
    /// so a material cannot describe a surface that bends light one way and
    /// reflects it as though it bent another.
    #[must_use]
    pub fn normal_reflectance(self) -> f32 {
        let ior = self.sanitised().ior;
        let ratio = (ior - IOR_AIR) / (ior + IOR_AIR);

        ratio * ratio
    }

    /// Returns the per-channel indices of refraction, red through blue.
    ///
    /// Dispersion is applied symmetrically about [`Material::ior`]: red bends
    /// least, blue most, which is the direction real glass disperses.
    #[must_use]
    pub fn dispersed_ior(self) -> [f32; 3] {
        let material = self.sanitised();
        let half = material.dispersion * 0.5;

        [material.ior - half, material.ior, material.ior + half]
    }

    /// Whether this material transmits enough to be worth sampling a backdrop
    /// for.
    ///
    /// A backdrop capture costs a full-surface read, so a material that
    /// transmits nothing should never trigger one.
    #[must_use]
    pub fn needs_backdrop(self) -> bool {
        self.sanitised().transmission > 0.0
    }
}

impl Default for Material {
    /// [`Material::OPAQUE`] — what every existing widget already is.
    fn default() -> Self {
        Self::OPAQUE
    }
}

/// The single light every surface in the interface responds to.
///
/// One light, owned by the design system rather than by an application, is
/// what makes highlights across an entire interface move together. A per-app
/// light produces surfaces that disagree about where the window is, which
/// reads as inconsistency rather than as lighting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneLight {
    /// Direction *towards* the light, normalised.
    ///
    /// Interface convention: `+x` right, `+y` down, `+z` out of the screen
    /// towards the viewer.
    pub direction: [f32; 3],

    /// The light's colour and intensity, scene-referred.
    pub radiance: Radiance,
}

impl SceneLight {
    /// A key light from above and slightly to the left, at diffuse white.
    ///
    /// Above because every real environment lights from above, and an
    /// interface lit from below reads as uncanny for the same reason a face
    /// does.
    pub const KEY: Self = Self {
        direction: [-0.32, -0.78, 0.54],
        radiance: Radiance::WHITE,
    };

    /// Returns the direction as a unit vector.
    #[must_use]
    pub fn unit_direction(self) -> [f32; 3] {
        let [x, y, z] = self.direction;
        let length = (x * x + y * y + z * z).sqrt();

        if !length.is_finite() || length <= f32::EPSILON {
            return [0.0, 0.0, 1.0];
        }

        [x / length, y / length, z / length]
    }
}

impl Default for SceneLight {
    fn default() -> Self {
        Self::KEY
    }
}

/// Schlick's approximation to the Fresnel reflectance.
///
/// `cos_theta` is the cosine of the angle between the view direction and the
/// surface normal. Reflectance rises to `1.0` at grazing incidence on every
/// material, which is why a glass edge is bright regardless of its tint — and
/// why a blur-and-tint approximation never looks like glass.
#[must_use]
pub fn fresnel_schlick(cos_theta: f32, normal_reflectance: f32) -> f32 {
    // `f32::clamp` propagates NaN, so clamping is not a guard. These are
    // reached from a shader and from authored materials, and a single NaN
    // here becomes a NaN pixel that spreads through every blend it touches.
    let cos_theta = clamp_finite(cos_theta, 0.0, 1.0, 1.0);
    let f0 = clamp_finite(normal_reflectance, 0.0, 1.0, 0.0);

    f0 + (1.0 - f0) * (1.0 - cos_theta).powi(5)
}

/// The Trowbridge-Reitz (GGX) normal distribution function.
///
/// `cos_normal_half` is the cosine of the angle between the surface normal and
/// the half vector. Returns the density of microfacets oriented to reflect
/// light towards the viewer.
#[must_use]
pub fn ggx_distribution(cos_normal_half: f32, roughness: f32) -> f32 {
    let cos = clamp_finite(cos_normal_half, 0.0, 1.0, 1.0);
    let alpha = clamp_finite(roughness, Material::MIN_ROUGHNESS, 1.0, 1.0);
    let alpha2 = alpha * alpha;

    let denominator = cos * cos * (alpha2 - 1.0) + 1.0;

    alpha2 / (std::f32::consts::PI * denominator * denominator).max(f32::MIN_POSITIVE)
}

/// How far refraction displaces what is seen through a surface, in
/// millimetres.
///
/// `cos_theta` is the cosine of the angle between the view direction and the
/// surface normal. The displacement is zero looking straight through — which
/// is exactly why a screen-space blur passes for glass until the viewer or the
/// content moves.
#[must_use]
pub fn refraction_offset_mm(cos_theta: f32, material: Material) -> f32 {
    let material = material.sanitised();
    let cos_theta = clamp_finite(cos_theta, 0.0, 1.0, 1.0);

    if material.thickness_mm <= 0.0 || material.ior <= IOR_AIR {
        return 0.0;
    }

    let sin_incident = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
    let sin_refracted = (sin_incident / material.ior).clamp(0.0, 1.0);

    let tan_incident = sin_incident / cos_theta.max(f32::MIN_POSITIVE);
    let tan_refracted = sin_refracted
        / (1.0 - sin_refracted * sin_refracted)
            .max(f32::MIN_POSITIVE)
            .sqrt();

    // Lateral displacement through a slab: the difference between where the
    // ray would have gone and where it does go, over the slab's thickness.
    (material.thickness_mm * (tan_incident - tan_refracted)).max(0.0)
}

/// Clamps a value into a range, replacing anything non-finite with `fallback`.
fn clamp_finite(value: f32, low: f32, high: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(low, high)
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every material this module ships, so a new one cannot skip the rules.
    const ALL: &[Material] = &[
        Material::OPAQUE,
        Material::GLASS,
        Material::FROSTED,
        Material::POLISHED,
    ];

    #[test]
    fn fresnel_is_f0_head_on_and_total_at_grazing() {
        // The defining property, and the one a blur-and-tint approximation
        // cannot reproduce: every material becomes a mirror at a glancing
        // angle, which is why a glass edge is bright whatever its tint.
        for material in ALL {
            let f0 = material.normal_reflectance();

            assert!(
                (fresnel_schlick(1.0, f0) - f0).abs() < 1e-5,
                "{material:?} head-on reflectance is not F0"
            );
            assert!(
                (fresnel_schlick(0.0, f0) - 1.0).abs() < 1e-5,
                "{material:?} is not fully reflective at grazing incidence"
            );
        }
    }

    #[test]
    fn fresnel_rises_monotonically_towards_grazing() {
        let f0 = Material::GLASS.normal_reflectance();
        let mut previous = f0;

        for step in (0..=100u8).rev() {
            let cos_theta = f32::from(step) / 100.0;
            let reflectance = fresnel_schlick(cos_theta, f0);

            assert!(
                reflectance >= previous - 1e-6,
                "reflectance fell at cos {cos_theta}: {reflectance} after {previous}"
            );

            previous = reflectance;
        }
    }

    #[test]
    fn glass_reflects_about_four_percent_head_on() {
        // The textbook number for an air-glass interface, and a check that
        // F0 really is derived from the index of refraction rather than
        // authored independently of it.
        let reflectance = Material::GLASS.normal_reflectance();

        assert!(
            (reflectance - 0.043).abs() < 0.005,
            "glass reflects {reflectance} head-on, expected ~0.04"
        );
    }

    #[test]
    fn a_material_that_does_not_bend_light_reflects_nothing_head_on() {
        assert!(Material::OPAQUE.normal_reflectance() < 1e-6);
    }

    #[test]
    fn the_ggx_lobe_narrows_as_a_surface_smooths() {
        // A smoother surface concentrates the same energy into a tighter
        // highlight, so its peak must be higher.
        let smooth = ggx_distribution(1.0, 0.05);
        let rough = ggx_distribution(1.0, 0.9);

        assert!(
            smooth > rough * 10.0,
            "smooth peak {smooth} is not sharper than rough peak {rough}"
        );
    }

    #[test]
    fn the_ggx_lobe_peaks_on_the_normal_and_never_goes_negative() {
        for roughness in [0.05_f32, 0.3, 0.6, 1.0] {
            let peak = ggx_distribution(1.0, roughness);

            for step in 0..=100u8 {
                let cos = f32::from(step) / 100.0;
                let value = ggx_distribution(cos, roughness);

                assert!(value >= 0.0, "negative microfacet density at {cos}");
                assert!(value.is_finite(), "non-finite density at {cos}");
                assert!(
                    value <= peak + 1e-3,
                    "roughness {roughness} peaks off the normal: {value} > {peak}"
                );
            }
        }
    }

    #[test]
    fn refraction_displaces_nothing_when_looking_straight_through() {
        // Zero at normal incidence is exactly why a screen-space blur passes
        // for glass in a screenshot and stops passing the moment anything
        // moves.
        for material in ALL {
            assert!(
                refraction_offset_mm(1.0, *material).abs() < 1e-5,
                "{material:?} displaced content when viewed head-on"
            );
        }
    }

    #[test]
    fn refraction_grows_with_viewing_angle() {
        let mut previous = 0.0;

        for step in (10..=100u8).rev() {
            let cos_theta = f32::from(step) / 100.0;
            let offset = refraction_offset_mm(cos_theta, Material::GLASS);

            assert!(
                offset >= previous - 1e-4,
                "displacement fell at cos {cos_theta}: {offset} after {previous}"
            );

            previous = offset;
        }

        assert!(
            previous > 0.0,
            "glass never displaced anything at any angle"
        );
    }

    #[test]
    fn a_surface_with_no_thickness_refracts_nothing() {
        // However high its index of refraction: there is no slab to traverse.
        let paper_thin = Material {
            thickness_mm: 0.0,
            ..Material::GLASS
        };

        for step in 0..=100u8 {
            let cos_theta = f32::from(step) / 100.0;

            assert!(refraction_offset_mm(cos_theta, paper_thin).abs() < 1e-6);
        }
    }

    #[test]
    fn dispersion_separates_the_channels_in_the_right_direction() {
        // Red bends least, blue most. Getting this backwards produces edges
        // that look wrong without looking obviously wrong.
        let [red, green, blue] = Material::GLASS.dispersed_ior();

        assert!(red < green, "red should bend less than green");
        assert!(green < blue, "blue should bend more than green");
        assert!((green - Material::GLASS.ior).abs() < 1e-6);
    }

    #[test]
    fn a_material_without_dispersion_keeps_its_channels_together() {
        let [red, green, blue] = Material::POLISHED.dispersed_ior();

        assert!((red - green).abs() < 1e-6 && (green - blue).abs() < 1e-6);
    }

    #[test]
    fn only_transmissive_materials_ask_for_a_backdrop() {
        // A backdrop capture costs a full-surface read; an opaque card must
        // never trigger one.
        assert!(!Material::OPAQUE.needs_backdrop());
        assert!(!Material::POLISHED.needs_backdrop());
        assert!(Material::GLASS.needs_backdrop());
        assert!(Material::FROSTED.needs_backdrop());
    }

    #[test]
    fn an_authored_material_cannot_break_the_shading_model() {
        // Materials are authored, and authored values arrive out of range.
        // An index of refraction below one inverts refraction and a negative
        // roughness gives a negative microfacet density; both show as a black
        // or exploded surface rather than as an error.
        let nonsense = [
            Material {
                roughness: -1.0,
                ..Material::GLASS
            },
            Material {
                roughness: f32::NAN,
                ..Material::GLASS
            },
            Material {
                roughness: 50.0,
                ..Material::GLASS
            },
            Material {
                ior: 0.2,
                ..Material::GLASS
            },
            Material {
                ior: f32::INFINITY,
                ..Material::GLASS
            },
            Material {
                thickness_mm: -5.0,
                ..Material::GLASS
            },
            Material {
                thickness_mm: f32::NAN,
                ..Material::GLASS
            },
            Material {
                dispersion: -3.0,
                ..Material::GLASS
            },
            Material {
                transmission: 9.0,
                ..Material::GLASS
            },
        ];

        for material in nonsense {
            let clean = material.sanitised();

            assert!(clean.roughness >= Material::MIN_ROUGHNESS && clean.roughness <= 1.0);
            assert!(clean.ior >= IOR_AIR && clean.ior.is_finite());
            assert!(clean.thickness_mm >= 0.0 && clean.thickness_mm.is_finite());
            assert!(clean.transmission >= 0.0 && clean.transmission <= 1.0);

            assert!(clean.normal_reflectance().is_finite());
            assert!(clean.dispersed_ior().iter().all(|ior| ior.is_finite()));

            for step in 0..=20u8 {
                let cos = f32::from(step) / 20.0;

                assert!(
                    refraction_offset_mm(cos, material).is_finite(),
                    "{material:?} produced a non-finite displacement"
                );
                assert!(ggx_distribution(cos, material.roughness).is_finite());
            }
        }
    }

    #[test]
    fn the_default_material_is_what_every_widget_already_is() {
        // Adding a material to a primitive must change nothing until someone
        // asks for one.
        let default = Material::default();

        assert_eq!(default, Material::OPAQUE);
        assert!(!default.needs_backdrop());
        assert!(default.normal_reflectance() < 1e-6);
        assert!(refraction_offset_mm(0.5, default).abs() < 1e-6);
    }

    #[test]
    fn the_scene_light_is_normalised_and_comes_from_above() {
        let direction = SceneLight::KEY.unit_direction();
        let length = direction
            .iter()
            .map(|component| component * component)
            .sum::<f32>()
            .sqrt();

        assert!(
            (length - 1.0).abs() < 1e-4,
            "key light is not a unit vector"
        );

        // `+y` is down, so a light from above has a negative y.
        assert!(direction[1] < 0.0, "the key light comes from below");
        assert!(direction[2] > 0.0, "the key light is behind the surface");
    }

    #[test]
    fn a_degenerate_light_direction_falls_back_rather_than_producing_nan() {
        for direction in [
            [0.0, 0.0, 0.0],
            [f32::NAN, 0.0, 0.0],
            [f32::INFINITY, 0.0, 0.0],
        ] {
            let light = SceneLight {
                direction,
                ..SceneLight::KEY
            };

            assert!(light.unit_direction().iter().all(|value| value.is_finite()));
        }
    }
}

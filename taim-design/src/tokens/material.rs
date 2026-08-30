//! Semantic surface materials.
//!
//! The colour tokens say what a surface *is*; the luminance tokens say how
//! much light it emits. These say what it is *made of* — how rough, how thick,
//! how much it bends what passes through it.
//!
//! A design system that stops at colour can describe a card and a sheet of
//! glass only by making the glass semi-transparent, which is why every
//! "frosted" surface in every framework looks like the same grey wash. These
//! are physical descriptions, so two surfaces that differ only in roughness
//! genuinely look different under the same light.
//!
//! Opt-in throughout, exactly like [`super::LuminanceTokens`]: the default is
//! [`Material::OPAQUE`], which is what every widget already is.

use taim_core::material::{Material, SceneLight};

/// Materials for the surface roles the design system names.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialTokens {
    /// The application background. Never transmissive: there is nothing
    /// behind it to transmit.
    pub background: Material,
    /// An ordinary raised surface — a card, a list, a panel.
    pub surface: Material,
    /// A surface that floats over content and lets it through: a sheet, a
    /// drawer, a navigation bar over a scrolling page.
    pub overlay: Material,
    /// A modal surface, which should read as heavier and less transmissive
    /// than an overlay so it separates rather than floats.
    pub scrim: Material,
    /// An interactive accent — a selected chip, a focused control.
    pub accent: Material,
    /// The one light every surface responds to.
    pub light: SceneLight,
}

impl MaterialTokens {
    /// Everything opaque: what every interface built before this decade is,
    /// and what this framework drew until now.
    pub const FLAT: Self = Self {
        background: Material::OPAQUE,
        surface: Material::OPAQUE,
        overlay: Material::OPAQUE,
        scrim: Material::OPAQUE,
        accent: Material::OPAQUE,
        light: SceneLight::KEY,
    };

    /// A restrained material set.
    ///
    /// `overlay` is frosted rather than clear, because a clear pane over
    /// running text is unreadable however beautiful it is. `scrim` is thicker
    /// and less transmissive so a modal separates rather than floats. `accent`
    /// is polished rather than transmissive: an interactive element should
    /// catch the light, not show what is behind it.
    pub const PHYSICAL: Self = Self {
        background: Material::OPAQUE,
        surface: Material::OPAQUE,
        overlay: Material::FROSTED,
        scrim: Material {
            thickness_mm: 3.0,
            transmission: 0.45,
            ..Material::FROSTED
        },
        accent: Material::POLISHED,
        light: SceneLight::KEY,
    };

    /// Returns whether any role in this set transmits light.
    ///
    /// A backdrop capture costs a full-surface read, so a set that transmits
    /// nothing should never trigger one.
    #[must_use]
    pub fn needs_backdrop(self) -> bool {
        [
            self.background,
            self.surface,
            self.overlay,
            self.scrim,
            self.accent,
        ]
        .iter()
        .any(|material| material.needs_backdrop())
    }

    /// Returns every role's material, sanitised.
    #[must_use]
    pub fn sanitised(self) -> Self {
        Self {
            background: self.background.sanitised(),
            surface: self.surface.sanitised(),
            overlay: self.overlay.sanitised(),
            scrim: self.scrim.sanitised(),
            accent: self.accent.sanitised(),
            light: self.light,
        }
    }
}

impl Default for MaterialTokens {
    /// [`MaterialTokens::FLAT`].
    ///
    /// Materials are opt-in at every layer, because a default that changed
    /// how every existing surface looks is not a default.
    fn default() -> Self {
        Self::FLAT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_changes_nothing() {
        let tokens = MaterialTokens::default();

        assert_eq!(tokens, MaterialTokens::FLAT);
        assert!(
            !tokens.needs_backdrop(),
            "the flat set asked for a backdrop"
        );

        for material in [
            tokens.background,
            tokens.surface,
            tokens.overlay,
            tokens.scrim,
            tokens.accent,
        ] {
            assert_eq!(material, Material::OPAQUE);
        }
    }

    #[test]
    fn a_background_never_transmits() {
        // There is nothing behind it. A transmissive background samples
        // whatever the last frame left in the backdrop buffer.
        for tokens in [MaterialTokens::FLAT, MaterialTokens::PHYSICAL] {
            assert!(!tokens.background.needs_backdrop());
        }
    }

    #[test]
    fn an_overlay_is_frosted_rather_than_clear() {
        // Clear glass over running text is unreadable however good it looks.
        let overlay = MaterialTokens::PHYSICAL.overlay;

        assert!(overlay.needs_backdrop());
        assert!(
            overlay.roughness > 0.2,
            "a clear overlay would leave text underneath legible through it"
        );
    }

    #[test]
    fn a_scrim_separates_more_than_an_overlay_floats() {
        let tokens = MaterialTokens::PHYSICAL;

        assert!(
            tokens.scrim.transmission < tokens.overlay.transmission,
            "a modal scrim should transmit less than an overlay"
        );
        assert!(tokens.scrim.thickness_mm > tokens.overlay.thickness_mm);
    }

    #[test]
    fn an_accent_catches_light_without_showing_what_is_behind_it() {
        let accent = MaterialTokens::PHYSICAL.accent;

        assert!(!accent.needs_backdrop());
        assert!(
            accent.normal_reflectance() > Material::OPAQUE.normal_reflectance(),
            "an accent should be more reflective than a plain surface"
        );
    }

    #[test]
    fn every_role_survives_sanitising() {
        let nonsense = MaterialTokens {
            surface: Material {
                roughness: f32::NAN,
                ior: -1.0,
                thickness_mm: f32::INFINITY,
                dispersion: 99.0,
                transmission: -4.0,
            },
            ..MaterialTokens::PHYSICAL
        };

        let clean = nonsense.sanitised();

        assert!(clean.surface.roughness.is_finite());
        assert!(clean.surface.ior >= 1.0);
        assert!(clean.surface.thickness_mm.is_finite());
        assert!(clean.surface.transmission >= 0.0 && clean.surface.transmission <= 1.0);
        assert!(clean.surface.normal_reflectance().is_finite());
    }

    #[test]
    fn the_physical_set_asks_for_a_backdrop_and_the_flat_set_does_not() {
        assert!(MaterialTokens::PHYSICAL.needs_backdrop());
        assert!(!MaterialTokens::FLAT.needs_backdrop());
    }
}

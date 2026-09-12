//! Physical glassmorphic card component (Pillar III & XIV).
//!
//! Pairs physical material parameters (refraction, roughness, slab thickness)
//! with the deferred G-buffer lighting pass while maintaining 100% readability
//! and full accessibility on non-GPU/recovery devices.

#![allow(missing_docs)]

use super::{Appearance, ComponentSpec, Density, FocusRing, InputMethods, StateSupport};
use crate::state::Interaction;
use crate::theme::Theme;
use crate::tokens::{Elevation, Radius};

use taim_core::Size;
use taim_core::material::Material;
use taim_core::semantics::{Actions, Role};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlassCard {
    pub interactive: bool,
    pub selected: bool,
    pub thickness_mm: f32,
    pub roughness: f32,
    pub transmission: f32,
    pub ior: f32,
}

impl Default for GlassCard {
    fn default() -> Self {
        Self {
            interactive: false,
            selected: false,
            thickness_mm: 4.0,
            roughness: 0.15,
            transmission: 0.85,
            ior: 1.52, // Standard soda-lime glass
        }
    }
}

impl GlassCard {
    /// Creates a new physical glass card.
    pub fn new(interactive: bool) -> Self {
        Self {
            interactive,
            ..Default::default()
        }
    }

    /// Converts this card's physical properties into a [`Material`] for the G-buffer pass.
    pub fn material(&self) -> Material {
        Material {
            roughness: self.roughness.clamp(0.03, 1.0),
            thickness_mm: self.thickness_mm.max(0.0),
            ior: self.ior.max(1.0),
            dispersion: 0.02, // Subtle physical chromatic dispersion
            transmission: self.transmission.clamp(0.0, 1.0),
        }
    }

    /// Elevation of the glass card.
    pub const fn elevation(&self) -> Elevation {
        if self.selected {
            Elevation::Raised
        } else {
            Elevation::Flat
        }
    }
}

impl ComponentSpec for GlassCard {
    fn role(&self) -> Role {
        if self.interactive {
            Role::Button
        } else {
            Role::Group
        }
    }

    fn actions(&self) -> Actions {
        if self.interactive {
            Actions::ACTIVATE | Actions::FOCUS
        } else {
            Actions::empty()
        }
    }

    fn min_size(&self, _density: Density) -> Size {
        Size::new(48.0, 48.0)
    }

    fn supported_states(&self) -> StateSupport {
        if self.interactive {
            StateSupport::INTERACTIVE | StateSupport::SELECTED
        } else {
            StateSupport::empty()
        }
    }

    fn input_methods(&self) -> InputMethods {
        if self.interactive {
            InputMethods::ALL
        } else {
            InputMethods::empty()
        }
    }

    fn radius(&self) -> Radius {
        Radius::Medium
    }

    fn appearance(&self, interaction: Interaction, theme: &Theme) -> Appearance {
        let colors = theme.colors();
        let resolved = interaction.resolve(colors);

        Appearance {
            background: Some(colors.surface),
            foreground: resolved.foreground,
            border: if self.selected {
                Some(colors.primary)
            } else if interaction.hovered {
                Some(colors.focus)
            } else {
                Some(colors.on_surface_variant)
            },
            focus_ring: resolved.focus_ring.map(FocusRing::standard),
            overlay_alpha: resolved.overlay_alpha,
            radius: self.radius(),
            enabled: resolved.enabled,
            busy: resolved.busy,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glass_card_material_mapping() {
        let card = GlassCard::default();
        let mat = card.material();
        assert_eq!(mat.ior, 1.52);
        assert_eq!(mat.thickness_mm, 4.0);
        assert_eq!(mat.transmission, 0.85);
        assert_eq!(mat.roughness, 0.15);
        assert_eq!(card.elevation(), Elevation::Flat);

        let selected_card = GlassCard {
            selected: true,
            ..card
        };
        assert_eq!(selected_card.elevation(), Elevation::Raised);
    }
}


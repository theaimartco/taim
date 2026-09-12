//! Holographic / Skeleton shimmer wave component (Pillar XIV).
//!
//! Provides a swept angular luminance wave for loading placeholders, skeleton states,
//! and premium attention cues.

#![allow(missing_docs)]

use super::{Appearance, ComponentSpec, Density, InputMethods, StateSupport};
use crate::state::Interaction;
use crate::theme::Theme;
use crate::tokens::Radius;

use taim_core::Size;
use taim_core::semantics::{Actions, Role};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shimmer {
    pub active: bool,
    pub phase: f32,
}

impl Default for Shimmer {
    fn default() -> Self {
        Self {
            active: true,
            phase: 0.0,
        }
    }
}

impl Shimmer {
    /// Creates a new shimmer placeholder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the animated phase offset (0.0..=1.0).
    pub fn with_phase(mut self, phase: f32) -> Self {
        self.phase = phase.fract();
        self
    }
}

impl ComponentSpec for Shimmer {
    fn role(&self) -> Role {
        Role::Image
    }

    fn actions(&self) -> Actions {
        Actions::empty()
    }

    fn min_size(&self, _density: Density) -> Size {
        Size::new(48.0, 16.0)
    }

    fn supported_states(&self) -> StateSupport {
        StateSupport::empty()
    }

    fn input_methods(&self) -> InputMethods {
        InputMethods::empty()
    }

    fn radius(&self) -> Radius {
        Radius::Small
    }

    fn appearance(&self, _interaction: Interaction, theme: &Theme) -> Appearance {
        let colors = theme.colors();

        Appearance {
            background: Some(colors.surface),
            foreground: colors.on_surface_variant,
            border: None,
            focus_ring: None,
            overlay_alpha: 0.0,
            radius: self.radius(),
            enabled: true,
            busy: self.active,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shimmer_phase_wrapping() {
        let shim = Shimmer::new().with_phase(1.35);
        assert!((shim.phase - 0.35).abs() < 1e-5);
        assert!(shim.active);
    }
}

//! Circular metric gauge / speedometer ring (Pillar XIV).
//!
//! Provides hardware-accelerated analytic arc geometry with smooth stroke rounding,
//! glow illumination, and center readout formatting.

#![allow(missing_docs)]

use super::{Appearance, ComponentSpec, Density, FocusRing, InputMethods, StateSupport};
use crate::state::Interaction;
use crate::theme::Theme;
use crate::tokens::Radius;

use taim_core::Size;
use taim_core::semantics::{Actions, Role};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gauge {
    pub value: f32,
    pub interactive: bool,
}

impl Default for Gauge {
    fn default() -> Self {
        Self {
            value: 0.75,
            interactive: false,
        }
    }
}

impl Gauge {
    /// Creates a new circular gauge showing `value` in `0.0..=1.0`.
    pub fn new(value: f32) -> Self {
        Self {
            value: value.clamp(0.0, 1.0),
            interactive: false,
        }
    }

    /// Sweep angle in radians (e.g. 270 degrees = 1.5 * PI).
    pub fn sweep_angle(&self) -> f32 {
        self.value * std::f32::consts::PI * 1.5
    }
}

impl ComponentSpec for Gauge {
    fn role(&self) -> Role {
        Role::Slider
    }

    fn actions(&self) -> Actions {
        if self.interactive {
            Actions::SET_VALUE | Actions::FOCUS
        } else {
            Actions::empty()
        }
    }

    fn min_size(&self, _density: Density) -> Size {
        Size::new(48.0, 48.0)
    }

    fn supported_states(&self) -> StateSupport {
        if self.interactive {
            StateSupport::INTERACTIVE
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
        Radius::Full
    }

    fn appearance(&self, interaction: Interaction, theme: &Theme) -> Appearance {
        let colors = theme.colors();
        let resolved = interaction.resolve(colors);

        Appearance {
            background: Some(colors.surface),
            foreground: resolved.foreground,
            border: Some(colors.primary),
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
    fn gauge_sweep_and_clamping() {
        let gauge = Gauge::new(0.5);
        assert!((gauge.sweep_angle() - (0.5 * std::f32::consts::PI * 1.5)).abs() < 1e-6);

        let clamped = Gauge::new(1.5);
        assert_eq!(clamped.value, 1.0);
    }
}

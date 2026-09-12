//! Interactive segmented control / animated tabs (Pillar XIV).
//!
//! Provides tactile multi-segment selection with smooth spring-animated pill
//! transitions and high-contrast accessibility compliance.

#![allow(missing_docs)]

use super::{Appearance, ComponentSpec, Density, FocusRing, InputMethods, StateSupport};
use crate::state::Interaction;
use crate::theme::Theme;
use crate::tokens::Radius;

use taim_core::Size;
use taim_core::semantics::{Actions, Role};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentedControl {
    pub segments: usize,
    pub selected_index: usize,
    pub interactive: bool,
}

impl Default for SegmentedControl {
    fn default() -> Self {
        Self {
            segments: 3,
            selected_index: 0,
            interactive: true,
        }
    }
}

impl SegmentedControl {
    /// Creates a segmented control with `segments` items.
    pub fn new(segments: usize) -> Self {
        Self {
            segments: segments.max(1),
            selected_index: 0,
            interactive: true,
        }
    }

    /// Selects an active segment index (clamped to valid segment range).
    pub fn select(mut self, index: usize) -> Self {
        self.selected_index = index.min(self.segments.saturating_sub(1));
        self
    }
}

impl ComponentSpec for SegmentedControl {
    fn role(&self) -> Role {
        Role::Tab
    }

    fn actions(&self) -> Actions {
        if self.interactive {
            Actions::ACTIVATE | Actions::FOCUS
        } else {
            Actions::empty()
        }
    }

    fn min_size(&self, _density: Density) -> Size {
        Size::new(48.0 * self.segments as f32, 48.0)
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
        Radius::Full
    }

    fn appearance(&self, interaction: Interaction, theme: &Theme) -> Appearance {
        let colors = theme.colors();
        let resolved = interaction.resolve(colors);

        Appearance {
            background: Some(colors.surface),
            foreground: resolved.foreground,
            border: Some(colors.on_surface_variant),
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
    fn segmented_control_selection_and_clamping() {
        let ctrl = SegmentedControl::new(4);
        assert_eq!(ctrl.selected_index, 0);
        let ctrl = ctrl.select(2);
        assert_eq!(ctrl.selected_index, 2);
        let ctrl = ctrl.select(10);
        assert_eq!(ctrl.selected_index, 3);
        assert_eq!(ctrl.min_size(Density::Comfortable), Size::new(192.0, 48.0));
    }
}

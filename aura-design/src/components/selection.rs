#![allow(missing_docs)]

use super::{
    Appearance, ComponentSpec, Density, FocusRing, InputMethods, StateSupport, touch_size,
};
use crate::state::Interaction;
use crate::theme::Theme;
use crate::tokens::Radius;

use aura_core::Size;
use aura_core::semantics::{Actions, Role};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKind {
    Switch,
    Checkbox,
    Radio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub kind: SelectionKind,
    pub checked: bool,
    pub disabled: bool,
}

impl ComponentSpec for Selection {
    fn role(&self) -> Role {
        match self.kind {
            SelectionKind::Switch | SelectionKind::Checkbox => Role::Checkbox,
            SelectionKind::Radio => Role::RadioButton,
        }
    }

    fn actions(&self) -> Actions {
        if self.disabled {
            Actions::empty()
        } else {
            Actions::ACTIVATE | Actions::FOCUS
        }
    }

    fn min_size(&self, density: Density) -> Size {
        touch_size(density)
    }

    fn supported_states(&self) -> StateSupport {
        StateSupport::INTERACTIVE | StateSupport::SELECTED
    }

    fn input_methods(&self) -> InputMethods {
        InputMethods::ALL
    }

    fn radius(&self) -> Radius {
        match self.kind {
            // A switch track and a radio are fully rounded; a checkbox is
            // squared-off. That shape difference is what distinguishes
            // them without relying on colour alone.
            SelectionKind::Switch | SelectionKind::Radio => Radius::Full,
            SelectionKind::Checkbox => Radius::Small,
        }
    }

    fn appearance(&self, interaction: Interaction, theme: &Theme) -> Appearance {
        let colors = theme.colors();
        let resolved = interaction.resolve(colors);

        // Checked state is carried by the fill, unchecked by the border.
        // Both are shape/colour, never an effect, so the control reads at
        // every quality tier.
        let (background, border) = if interaction.selected {
            (Some(colors.primary), Some(colors.primary))
        } else {
            (None, Some(colors.on_surface_variant))
        };

        Appearance {
            background,
            foreground: if interaction.selected {
                colors.on_primary
            } else {
                resolved.foreground
            },
            border,
            focus_ring: resolved.focus_ring.map(FocusRing::standard),
            overlay_alpha: resolved.overlay_alpha,
            radius: self.radius(),
            enabled: resolved.enabled,
            busy: resolved.busy,
        }
    }
}

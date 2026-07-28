#![allow(missing_docs)]

use super::{Appearance, ComponentSpec, Density, FocusRing, InputMethods, StateSupport};
use crate::state::Interaction;
use crate::theme::Theme;
use crate::tokens::{Elevation, Radius};

use aura_core::Size;
use aura_core::semantics::{Actions, Role};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Card {
    pub interactive: bool,
    pub selected: bool,
}

impl Card {
    /// Returns the elevation this card sits at. A static card is a plain
    /// raised surface; an interactive one is identical at rest -- press
    /// feedback comes from the state layer, not from a bigger shadow, so
    /// the card still reads with effects disabled.
    pub const fn elevation(&self) -> Elevation {
        Elevation::Raised
    }
}

impl ComponentSpec for Card {
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
            // A selected card is outlined, not merely tinted: an outline
            // survives high-contrast and colour-blind viewing where a
            // subtle fill shift does not.
            border: interaction.selected.then_some(colors.primary),
            focus_ring: resolved.focus_ring.map(FocusRing::standard),
            overlay_alpha: resolved.overlay_alpha,
            radius: self.radius(),
            enabled: resolved.enabled,
            busy: resolved.busy,
        }
    }
}

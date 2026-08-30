#![allow(missing_docs)]

use super::{
    Appearance, ComponentSpec, Density, FocusRing, InputMethods, StateSupport, touch_size,
};
use crate::state::Interaction;
use crate::theme::Theme;
use crate::tokens::Radius;

use taim_core::Size;
use taim_core::semantics::{Actions, Role};

/// The visual weight of a button, which determines its fill and border
/// but never its behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonKind {
    /// Highest emphasis: filled with the primary token.
    #[default]
    Filled,
    /// Medium emphasis: a tinted surface fill.
    Tonal,
    /// Medium emphasis: a border with no fill.
    Outlined,
    /// Lowest emphasis: content only.
    Text,
    /// Highest emphasis for a destructive action: filled with `error`.
    Destructive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Button {
    pub kind: ButtonKind,
    pub loading: bool,
    pub disabled: bool,
}

impl Default for Button {
    fn default() -> Self {
        Self {
            kind: ButtonKind::Filled,
            loading: false,
            disabled: false,
        }
    }
}

impl ComponentSpec for Button {
    fn role(&self) -> Role {
        Role::Button
    }

    fn actions(&self) -> Actions {
        // A loading button is busy, not inert: it stays focusable so a
        // keyboard user is not ejected mid-interaction, but it cannot be
        // activated again until its work completes.
        if self.disabled {
            Actions::empty()
        } else if self.loading {
            Actions::FOCUS
        } else {
            Actions::ACTIVATE | Actions::FOCUS
        }
    }

    fn min_size(&self, density: Density) -> Size {
        touch_size(density)
    }

    fn supported_states(&self) -> StateSupport {
        StateSupport::INTERACTIVE | StateSupport::LOADING
    }

    fn input_methods(&self) -> InputMethods {
        InputMethods::ALL
    }

    fn radius(&self) -> Radius {
        Radius::Full
    }

    fn appearance(&self, interaction: Interaction, theme: &Theme) -> Appearance {
        let colors = theme.colors();
        let resolved = interaction.resolve(colors);

        // Every colour below is a theme token; a button never invents one.
        let (background, foreground, border) = match self.kind {
            ButtonKind::Filled => (Some(colors.primary), colors.on_primary, None),
            ButtonKind::Destructive => (Some(colors.error), colors.on_primary, None),
            ButtonKind::Tonal => (Some(colors.surface), colors.primary, None),
            ButtonKind::Outlined => (None, colors.primary, Some(colors.primary)),
            ButtonKind::Text => (None, colors.primary, None),
        };

        Appearance {
            background,
            foreground,
            border,
            focus_ring: resolved.focus_ring.map(FocusRing::standard),
            overlay_alpha: resolved.overlay_alpha,
            radius: self.radius(),
            enabled: resolved.enabled,
            busy: resolved.busy,
        }
    }
}

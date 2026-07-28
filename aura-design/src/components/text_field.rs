#![allow(missing_docs)]

use super::{
    Appearance, ComponentSpec, Density, FocusRing, InputMethods, StateSupport, touch_size,
};
use crate::state::Interaction;
use crate::theme::Theme;
use crate::tokens::Radius;

use aura_core::Size;
use aura_core::semantics::{Actions, Role};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextField {
    /// A secure field never exposes its value to assistive technology,
    /// the clipboard, or surrounding-text APIs (see `phase.md` Phase 10's
    /// security requirements); [`Role::SecureField`] is the contract that
    /// carries that rule to consumers.
    pub secure: bool,
    pub multiline: bool,
    pub error: bool,
    pub disabled: bool,
}

impl ComponentSpec for TextField {
    fn role(&self) -> Role {
        if self.secure {
            Role::SecureField
        } else {
            Role::TextField
        }
    }

    fn actions(&self) -> Actions {
        if self.disabled {
            Actions::empty()
        } else {
            Actions::FOCUS | Actions::SET_VALUE
        }
    }

    fn min_size(&self, density: Density) -> Size {
        let touch = touch_size(density);

        if self.multiline {
            // A multi-line field is at least three lines tall so its
            // content is legible before the user scrolls.
            Size::new(touch.width, touch.height * 3.0)
        } else {
            touch
        }
    }

    fn supported_states(&self) -> StateSupport {
        StateSupport::INTERACTIVE | StateSupport::ERROR
    }

    fn input_methods(&self) -> InputMethods {
        InputMethods::ALL
    }

    fn radius(&self) -> Radius {
        Radius::Small
    }

    fn appearance(&self, interaction: Interaction, theme: &Theme) -> Appearance {
        let colors = theme.colors();
        let resolved = interaction.resolve(colors);

        // An errored field is bordered with the `error` token; the border
        // is what communicates the state, never a shadow or glow, so the
        // field still reads at the Recovery quality tier.
        let border = if interaction.error {
            Some(colors.error)
        } else if interaction.focus_visible {
            Some(colors.focus)
        } else {
            Some(colors.on_surface_variant)
        };

        Appearance {
            background: Some(colors.surface),
            foreground: resolved.foreground,
            border,
            focus_ring: resolved.focus_ring.map(FocusRing::standard),
            overlay_alpha: resolved.overlay_alpha,
            radius: self.radius(),
            enabled: resolved.enabled,
            busy: resolved.busy,
        }
    }
}

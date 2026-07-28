#![allow(missing_docs)]

use super::{ComponentSpec, Density, InputMethods, StateSupport};
use crate::tokens::Radius;

use aura_core::Size;
use aura_core::semantics::{Actions, Role};

/// What caused a tooltip to appear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// A pointer came to rest over the target.
    Hover,
    /// The target received visible focus (keyboard/gamepad).
    Focus,
    /// A touch was held on the target.
    LongPress,
}

/// A transient label describing its target.
///
/// A tooltip is never the *only* place information lives -- it is
/// supplementary by definition, because it cannot be reached by every
/// input on every device. An icon-only control still carries its own
/// accessible name (see
/// [`icon_button::IconButton::has_accessible_name`](super::icon_button::IconButton::has_accessible_name));
/// the tooltip merely surfaces it visually.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tooltip {
    pub focus_enabled: bool,
    pub long_press_enabled: bool,
}

impl Tooltip {
    /// Whether `trigger` shows this tooltip.
    ///
    /// Hover always does; focus and long-press are opt-in because a
    /// target that already shows a visible label does not need to repeat
    /// it for every input method.
    pub const fn responds_to(&self, trigger: Trigger) -> bool {
        match trigger {
            Trigger::Hover => true,
            Trigger::Focus => self.focus_enabled,
            Trigger::LongPress => self.long_press_enabled,
        }
    }

    /// Whether this tooltip is reachable without a pointer. A tooltip
    /// carrying information a touch or keyboard user needs must answer
    /// `true`, or that information is pointer-only.
    pub const fn reachable_without_pointer(&self) -> bool {
        self.focus_enabled || self.long_press_enabled
    }
}

impl ComponentSpec for Tooltip {
    fn role(&self) -> Role {
        Role::StaticText
    }

    fn actions(&self) -> Actions {
        // A tooltip is described, never operated: it takes no focus and
        // offers no action of its own.
        Actions::empty()
    }

    fn min_size(&self, _density: Density) -> Size {
        // A tooltip is sized entirely by its text.
        Size::ZERO
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
}

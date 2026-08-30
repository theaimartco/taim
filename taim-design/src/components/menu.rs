#![allow(missing_docs)]

use super::{ComponentSpec, Density, InputMethods, StateSupport};
use crate::tokens::Radius;

use taim_core::Size;
use taim_core::semantics::{Actions, Role};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Menu {
    pub expanded: bool,
    pub has_submenu: bool,
}

impl Menu {
    /// Returns whether `typed` matches `label` for typeahead selection.
    ///
    /// Typeahead is case-insensitive and prefix-based, which is what lets
    /// a keyboard user reach an item without arrowing through the whole
    /// menu. An empty query matches nothing rather than everything, so a
    /// stray keystroke never jumps the selection.
    pub fn matches_typeahead(label: &str, typed: &str) -> bool {
        let typed = typed.trim();

        if typed.is_empty() {
            return false;
        }

        label.to_lowercase().starts_with(&typed.to_lowercase())
    }
}

impl ComponentSpec for Menu {
    fn role(&self) -> Role {
        Role::MenuItem
    }

    fn actions(&self) -> Actions {
        let mut actions = Actions::ACTIVATE | Actions::FOCUS;

        if self.has_submenu {
            actions |= Actions::TOGGLE_EXPANDED;
        }

        actions
    }

    fn min_size(&self, density: Density) -> Size {
        let height = match density {
            Density::Compact => 48.0,
            Density::Comfortable => 52.0,
            Density::Spacious => 56.0,
        };

        // Wide enough for a label plus a submenu affordance.
        Size::new(160.0, height)
    }

    fn supported_states(&self) -> StateSupport {
        StateSupport::INTERACTIVE | StateSupport::SELECTED
    }

    fn input_methods(&self) -> InputMethods {
        InputMethods::ALL
    }

    fn radius(&self) -> Radius {
        Radius::Small
    }
}

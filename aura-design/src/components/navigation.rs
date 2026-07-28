#![allow(missing_docs)]

use super::{ComponentSpec, Density, InputMethods, StateSupport, ViewportClass, touch_size};
use crate::tokens::Radius;

use aura_core::Size;
use aura_core::semantics::{Actions, Role};

/// How a navigation destination presents itself, which is a function of
/// the viewport rather than an author's choice: the same destinations
/// become a bottom bar on a phone, a rail on a tablet, and a sidebar on a
/// desktop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationMode {
    Bar,
    Rail,
    Sidebar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Navigation {
    pub mode: NavigationMode,
    pub selected: bool,
}

impl Navigation {
    /// Returns the destination presentation for `viewport`. This mapping
    /// is the whole of Aura's adaptive navigation policy.
    pub const fn for_viewport(viewport: ViewportClass) -> Self {
        Self {
            mode: match viewport {
                ViewportClass::Compact => NavigationMode::Bar,
                ViewportClass::Medium => NavigationMode::Rail,
                ViewportClass::Expanded => NavigationMode::Sidebar,
            },
            selected: false,
        }
    }

    /// Whether this mode shows a text label beside its icon. A bar and a
    /// rail are too narrow for one; a sidebar always shows it.
    pub const fn shows_label(&self) -> bool {
        matches!(self.mode, NavigationMode::Sidebar)
    }
}

impl ComponentSpec for Navigation {
    fn role(&self) -> Role {
        Role::Tab
    }

    fn actions(&self) -> Actions {
        Actions::ACTIVATE | Actions::FOCUS
    }

    fn min_size(&self, density: Density) -> Size {
        let touch = touch_size(density);

        match self.mode {
            // A sidebar destination is a full-width row, so it needs room
            // for its label; a bar/rail destination is a square target.
            NavigationMode::Sidebar => Size::new(200.0, touch.height),
            NavigationMode::Bar | NavigationMode::Rail => touch,
        }
    }

    fn supported_states(&self) -> StateSupport {
        StateSupport::INTERACTIVE | StateSupport::SELECTED
    }

    fn input_methods(&self) -> InputMethods {
        InputMethods::ALL
    }

    fn radius(&self) -> Radius {
        Radius::Full
    }
}

#![allow(missing_docs)]

use super::{ComponentSpec, Density, InputMethods, StateSupport, touch_size};
use crate::icon::Icon;
use crate::tokens::Radius;

use taim_core::Size;
use taim_core::semantics::{Actions, Role};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconButton {
    pub icon: Icon,
    /// An icon-only control carries no visible label, so a textual
    /// alternative is mandatory rather than decorative: it is the only
    /// name assistive technology and a tooltip can report.
    pub tooltip: Option<&'static str>,
    pub disabled: bool,
}

impl IconButton {
    /// Returns whether this control has the accessible name every
    /// icon-only button requires.
    pub fn has_accessible_name(&self) -> bool {
        self.tooltip.is_some_and(|label| !label.trim().is_empty())
    }
}

impl ComponentSpec for IconButton {
    fn role(&self) -> Role {
        Role::Button
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
        Radius::Full
    }
}

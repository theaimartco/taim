#![allow(missing_docs)]

use super::{ComponentSpec, Density, InputMethods, LayoutSpec, StateSupport, ViewportClass};
use crate::tokens::{Elevation, Radius, Spacing};

use aura_core::Size;
use aura_core::semantics::{Actions, Role};

/// A modal dialog.
///
/// A modal owns focus for as long as it is open: [`Self::focus_trapped`]
/// is not a styling preference but the property that stops a keyboard or
/// screen-reader user from wandering into the inert content behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dialog {
    pub dismissible: bool,
    pub focus_trapped: bool,
}

impl Dialog {
    /// Whether this dialog is a correctly-formed modal.
    ///
    /// A modal that does not trap focus is a bug, not a variant: the
    /// content behind it is visually inert but would still be reachable.
    pub const fn traps_focus_correctly(&self) -> bool {
        self.focus_trapped
    }

    /// The elevation a modal sits at, above every ordinary surface.
    pub const fn elevation(&self) -> Elevation {
        Elevation::Modal
    }
}

impl ComponentSpec for Dialog {
    fn role(&self) -> Role {
        Role::Window
    }

    fn actions(&self) -> Actions {
        if self.dismissible {
            Actions::DISMISS
        } else {
            Actions::empty()
        }
    }

    fn min_size(&self, _density: Density) -> Size {
        Size::new(280.0, 120.0)
    }

    fn supported_states(&self) -> StateSupport {
        StateSupport::empty()
    }

    fn input_methods(&self) -> InputMethods {
        if self.dismissible {
            // Dismissal must be reachable from every input: Escape,
            // a scrim tap, and a gamepad's cancel button.
            InputMethods::ALL
        } else {
            InputMethods::empty()
        }
    }

    fn layout(&self, viewport: ViewportClass, density: Density) -> LayoutSpec {
        LayoutSpec {
            min_size: self.min_size(density),
            // A dialog is roomier than an inline surface at every size,
            // and grows its gutter on larger viewports.
            padding: match viewport {
                ViewportClass::Compact => Spacing::LG,
                ViewportClass::Medium | ViewportClass::Expanded => Spacing::XL,
            },
            gap: Spacing::MD,
        }
    }

    fn radius(&self) -> Radius {
        Radius::Large
    }
}

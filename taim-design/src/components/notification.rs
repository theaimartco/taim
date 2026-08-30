#![allow(missing_docs)]

use super::{ComponentSpec, Density, InputMethods, StateSupport};
use crate::tokens::Radius;

use taim_core::Size;
use taim_core::semantics::{Actions, Role};

/// How much of a notification may be shown when the device is locked.
///
/// This is a security boundary, not a styling choice: a notification's
/// content can be visible to anyone looking at a locked screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Privacy {
    /// Title and body may be shown anywhere.
    #[default]
    Public,
    /// The title may be shown while locked; the body is withheld until
    /// the device is unlocked.
    Sensitive,
    /// Nothing beyond the app's own name may be shown while locked.
    Secret,
}

impl Privacy {
    /// Every level, for exhaustive iteration.
    pub const ALL: [Self; 3] = [Self::Public, Self::Sensitive, Self::Secret];

    /// Whether the title may be shown at `locked`.
    pub const fn shows_title(self, locked: bool) -> bool {
        match self {
            Self::Public | Self::Sensitive => true,
            Self::Secret => !locked,
        }
    }

    /// Whether the body may be shown at `locked`.
    pub const fn shows_body(self, locked: bool) -> bool {
        match self {
            Self::Public => true,
            Self::Sensitive | Self::Secret => !locked,
        }
    }
}

/// A persistent, grouped notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Notification {
    pub privacy: Privacy,
    pub actionable: bool,
    /// How many notifications this entry stands for when several from the
    /// same source are collapsed together. `1` is an ungrouped entry.
    pub group_count: u32,
}

impl Notification {
    /// Whether this entry represents a collapsed group.
    pub const fn is_grouped(&self) -> bool {
        self.group_count > 1
    }
}

impl ComponentSpec for Notification {
    fn role(&self) -> Role {
        Role::Group
    }

    fn actions(&self) -> Actions {
        if self.actionable {
            // Activatable implies focusable: an action nobody can reach
            // from a keyboard is not an action.
            Actions::ACTIVATE | Actions::FOCUS | Actions::DISMISS
        } else {
            Actions::DISMISS
        }
    }

    fn min_size(&self, _density: Density) -> Size {
        Size::new(280.0, 64.0)
    }

    fn supported_states(&self) -> StateSupport {
        if self.actionable {
            StateSupport::INTERACTIVE
        } else {
            StateSupport::empty()
        }
    }

    fn input_methods(&self) -> InputMethods {
        InputMethods::ALL
    }

    fn radius(&self) -> Radius {
        Radius::Medium
    }
}

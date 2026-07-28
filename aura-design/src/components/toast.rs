#![allow(missing_docs)]

use super::{ComponentSpec, Density, InputMethods, StateSupport};
use crate::tokens::{Elevation, Radius};

use aura_core::Size;
use aura_core::semantics::{Actions, Role};
use aura_core::time::Duration;

/// The shortest a toast may stay on screen. Anything briefer cannot be
/// read, let alone acted on.
pub const MIN_DURATION: Duration = Duration::from_secs(4);

/// The longest a toast stays on screen unaided.
pub const MAX_DURATION: Duration = Duration::from_secs(10);

/// A transient, timed message.
///
/// A toast announces itself to assistive technology as a live region --
/// it is never focused, because stealing focus from whatever the user was
/// doing to announce a transient message is hostile. That is exactly why
/// an *actionable* toast must also stay on screen long enough to reach:
/// see [`Self::visible_duration`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Toast {
    pub actionable: bool,
    pub dismissible: bool,
}

impl Toast {
    /// How long this toast stays visible without user interaction.
    ///
    /// An actionable toast gets the longer window, since the user has to
    /// notice it, read it, and travel to its action before it leaves.
    pub const fn visible_duration(&self) -> Duration {
        if self.actionable {
            MAX_DURATION
        } else {
            MIN_DURATION
        }
    }

    /// Whether this toast announces via a live region rather than taking
    /// focus. Always true: see this type's own documentation.
    pub const fn announces_as_live_region(&self) -> bool {
        true
    }

    /// The elevation a toast floats at.
    pub const fn elevation(&self) -> Elevation {
        Elevation::Floating
    }
}

impl ComponentSpec for Toast {
    fn role(&self) -> Role {
        Role::Group
    }

    fn actions(&self) -> Actions {
        let mut actions = Actions::empty();

        if self.actionable {
            actions |= Actions::ACTIVATE;
        }

        if self.dismissible {
            actions |= Actions::DISMISS;
        }

        actions
    }

    fn min_size(&self, _density: Density) -> Size {
        Size::new(280.0, 48.0)
    }

    fn supported_states(&self) -> StateSupport {
        StateSupport::empty()
    }

    fn input_methods(&self) -> InputMethods {
        if self.actionable || self.dismissible {
            InputMethods::ALL
        } else {
            InputMethods::empty()
        }
    }

    fn radius(&self) -> Radius {
        Radius::Small
    }
}

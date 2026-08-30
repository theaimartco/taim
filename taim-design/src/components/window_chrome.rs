#![allow(missing_docs)]

use super::{ComponentSpec, Density, InputMethods, StateSupport};
use crate::tokens::Radius;

use taim_core::Size;
use taim_core::semantics::{Actions, Role};

/// The inset, in logical pixels, along each screen edge that the system
/// reserves for its own gestures (back-swipe, home, notification shade).
///
/// Application content may render beneath this band, but must not place
/// a control whose only affordance is a drag starting inside it: the
/// system's own `edge_swipe` recognizer is registered at
/// `taim_gesture::arena::Priority::SYSTEM` and wins that competition by
/// policy (see `docs/architecture/input-gestures.md`).
pub const SYSTEM_GESTURE_INSET: f32 = 20.0;

/// The window's own frame: title bar, controls, and the band where system
/// gestures take precedence over application ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowChrome {
    /// Desktop chrome is taller and carries window controls; tablet/phone
    /// chrome is a thin status band.
    pub desktop: bool,
    /// Whether the platform reserves its edges for system gestures.
    pub system_gestures_reserved: bool,
}

impl WindowChrome {
    /// The inset an application must keep drag-only controls out of.
    /// Zero when the platform reserves nothing.
    pub const fn system_gesture_inset(&self) -> f32 {
        if self.system_gestures_reserved {
            SYSTEM_GESTURE_INSET
        } else {
            0.0
        }
    }

    /// Whether a drag-only control may own a gesture starting at
    /// `distance_from_edge`.
    ///
    /// This is the application-facing half of "system-edge gestures
    /// remain reserved by policy": a control that answers `false` here
    /// would be competing with the system and losing, so it must not be
    /// placed there in the first place.
    pub fn may_own_drag_at(&self, distance_from_edge: f32) -> bool {
        distance_from_edge >= self.system_gesture_inset()
    }

    /// The chrome's height in logical pixels.
    pub const fn height(&self) -> f32 {
        if self.desktop { 32.0 } else { 24.0 }
    }
}

impl ComponentSpec for WindowChrome {
    fn role(&self) -> Role {
        Role::Window
    }

    fn actions(&self) -> Actions {
        // The chrome is a frame; its window controls are `IconButton`s
        // with their own roles, focus, and touch targets.
        Actions::empty()
    }

    fn min_size(&self, _density: Density) -> Size {
        Size::new(0.0, self.height())
    }

    fn supported_states(&self) -> StateSupport {
        StateSupport::empty()
    }

    fn input_methods(&self) -> InputMethods {
        InputMethods::empty()
    }

    fn radius(&self) -> Radius {
        Radius::None
    }
}

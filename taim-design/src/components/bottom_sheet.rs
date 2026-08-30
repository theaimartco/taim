#![allow(missing_docs)]

use super::{ComponentSpec, Density, InputMethods, StateSupport};
use crate::tokens::{Elevation, Radius};

use taim_core::Size;
use taim_core::semantics::{Actions, Role};

/// A draggable sheet anchored to the bottom edge.
///
/// The sheet snaps between discrete [`Anchor`]s rather than resting
/// anywhere, so a released drag always lands somewhere meaningful. Its
/// drag participates in nested scrolling: content inside the sheet
/// scrolls first, and only a sheet-level drag past the content's own
/// extent moves the sheet (see `taim_gesture::nested_scroll`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BottomSheet {
    /// How far open the sheet is, `0.0` (closed) to `1.0` (fully open).
    pub expanded: f32,
    pub dismissible: bool,
}

/// A resting position a released sheet snaps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// Fully dismissed.
    Closed,
    /// The default resting height.
    Half,
    /// Fully open.
    Full,
}

impl Anchor {
    /// Every anchor, in ascending order of openness.
    pub const ALL: [Self; 3] = [Self::Closed, Self::Half, Self::Full];

    /// The normalized expansion this anchor rests at.
    pub const fn expansion(self) -> f32 {
        match self {
            Self::Closed => 0.0,
            Self::Half => 0.5,
            Self::Full => 1.0,
        }
    }
}

impl BottomSheet {
    /// Returns the anchor a release at the current expansion snaps to:
    /// whichever anchor is nearest.
    ///
    /// A non-dismissible sheet never snaps closed -- it settles at the
    /// lowest anchor it is allowed to occupy instead, so a drag can never
    /// dismiss something the caller said cannot be dismissed.
    pub fn snap_target(&self) -> Anchor {
        let expansion = self.expanded.clamp(0.0, 1.0);

        let candidates: &[Anchor] = if self.dismissible {
            &Anchor::ALL
        } else {
            &[Anchor::Half, Anchor::Full]
        };

        let mut best = candidates[0];
        let mut best_distance = f32::INFINITY;

        for anchor in candidates {
            let distance = (anchor.expansion() - expansion).abs();
            if distance < best_distance {
                best_distance = distance;
                best = *anchor;
            }
        }

        best
    }

    /// The elevation a sheet floats at.
    pub const fn elevation(&self) -> Elevation {
        Elevation::Floating
    }
}

impl ComponentSpec for BottomSheet {
    fn role(&self) -> Role {
        Role::Window
    }

    fn actions(&self) -> Actions {
        let mut actions = Actions::SET_VALUE;

        if self.dismissible {
            actions |= Actions::DISMISS;
        }

        actions
    }

    fn min_size(&self, _density: Density) -> Size {
        Size::new(280.0, 160.0)
    }

    fn supported_states(&self) -> StateSupport {
        // A sheet is dragged, so it has a pressed state, but it is not
        // itself hoverable or focusable -- its contents are.
        StateSupport::PRESSED
    }

    fn input_methods(&self) -> InputMethods {
        InputMethods::ALL
    }

    fn radius(&self) -> Radius {
        // Rounded at the top, meeting the screen edge at the bottom.
        Radius::Large
    }
}

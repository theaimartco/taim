use crate::core::Point;
use crate::pane_grid::{Axis, Pane, Split};

/// The current pane-grid interaction.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Action {
    /// No drag or resize is active.
    #[default]
    Idle,
    /// A pane is being dragged.
    Dragging {
        /// Dragged pane.
        pane: Pane,
        /// Pointer position where dragging began.
        origin: Point,
    },
    /// A split is being resized.
    Resizing {
        /// Resized split.
        split: Split,
        /// Split axis.
        axis: Axis,
    },
}

impl Action {
    /// Returns the dragged pane and its origin.
    pub fn picked_pane(&self) -> Option<(Pane, Point)> {
        match *self {
            Self::Dragging { pane, origin } => Some((pane, origin)),
            _ => None,
        }
    }

    /// Returns the resized split and its axis.
    pub fn picked_split(&self) -> Option<(Split, Axis)> {
        match *self {
            Self::Resizing { split, axis } => Some((split, axis)),
            _ => None,
        }
    }
}

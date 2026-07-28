//! Compositor-reserved edge policy.

use aura_core::{Point, Size};

/// Physical display edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    /// Left edge.
    Left,
    /// Right edge.
    Right,
    /// Top edge.
    Top,
    /// Bottom edge.
    Bottom,
}

/// Logical widths reserved for compositor gestures.
#[derive(Debug, Clone, Copy)]
pub struct ReservedEdges {
    /// Left reservation.
    pub left: f32,
    /// Right reservation.
    pub right: f32,
    /// Top reservation.
    pub top: f32,
    /// Bottom reservation.
    pub bottom: f32,
}

impl ReservedEdges {
    /// No compositor reservation.
    pub const NONE: Self = Self {
        left: 0.0,
        right: 0.0,
        top: 0.0,
        bottom: 0.0,
    };

    /// Returns the reserved edge containing a point.
    pub fn edge_at(self, point: Point, surface: Size) -> Option<Edge> {
        if point.x <= self.left {
            Some(Edge::Left)
        } else if point.x >= surface.width - self.right {
            Some(Edge::Right)
        } else if point.y <= self.top {
            Some(Edge::Top)
        } else if point.y >= surface.height - self.bottom {
            Some(Edge::Bottom)
        } else {
            None
        }
    }
}

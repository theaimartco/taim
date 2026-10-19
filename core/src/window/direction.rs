//! TAIM window-edge resize intents used by native compositor adapters.

/// Cardinal/corner edges relative to the center of a window.
#[derive(Debug, Clone, Copy)]
pub enum Direction {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

impl Direction {
    /// Logical outward direction in screen coordinates (+Y is downward).
    /// Use discrete coordinates rather than normalized floating-point
    /// vectors: a corner represents simultaneous horizontal+vertical resize.
    #[inline]
    pub const fn delta(self) -> (i8, i8) {
        match self {
            Self::North => (0, -1),
            Self::South => (0, 1),
            Self::East => (1, 0),
            Self::West => (-1, 0),
            Self::NorthEast => (1, -1),
            Self::NorthWest => (-1, -1),
            Self::SouthEast => (1, 1),
            Self::SouthWest => (-1, 1),
        }
    }

    /// Whether both axes of a native resize operation are active.
    #[inline]
    pub const fn is_corner(self) -> bool {
        let (x, y) = self.delta();
        x != 0 && y != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_resize_edges_point_outward_in_screen_space() {
        assert_eq!(Direction::North.delta(), (0, -1));
        assert_eq!(Direction::South.delta(), (0, 1));
        assert_eq!(Direction::NorthWest.delta(), (-1, -1));
        assert_eq!(Direction::SouthEast.delta(), (1, 1));
        assert!(Direction::NorthEast.is_corner());
        assert!(!Direction::East.is_corner());
    }
}

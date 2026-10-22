//! Stable physical key zones for TAIM keyboard and accessibility policy.

/// The physical key zone from the operating-system keyboard event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location {
    /// Not tied to a particular side or zone of the keyboard.
    Standard,
    /// The left copy of a key that appears twice, such as Left Shift.
    Left,
    /// The right copy of a key that appears twice, such as Right Shift.
    Right,
    /// A key on the numeric keypad.
    Numpad,
}

/// Whether a physical key is on a distinct side of the modifier cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The left-hand side.
    Left,
    /// The right-hand side.
    Right,
}

impl Location {
    /// Distinguishes left/right modifier keys for physical shortcuts.
    ///
    /// Numpad is not interpreted as a right-side modifier: shortcuts and
    /// accessibility focus must not conflate numeric keypad with Right Shift.
    #[inline]
    pub const fn modifier_side(self) -> Option<Side> {
        match self {
            Self::Left => Some(Side::Left),
            Self::Right => Some(Side::Right),
            Self::Standard | Self::Numpad => None,
        }
    }

    /// Numpad is a separate physical typing zone with its own shortcuts.
    #[inline]
    pub const fn is_numpad(self) -> bool {
        matches!(self, Self::Numpad)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keypad_does_not_impersonate_right_modifier() {
        assert_eq!(Location::Left.modifier_side(), Some(Side::Left));
        assert_eq!(Location::Right.modifier_side(), Some(Side::Right));
        assert_eq!(Location::Standard.modifier_side(), None);
        assert_eq!(Location::Numpad.modifier_side(), None);
        assert!(Location::Numpad.is_numpad());
        assert!(!Location::Right.is_numpad());
    }
}

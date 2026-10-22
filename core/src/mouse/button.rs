//! TAIM pointer-button roles shared by mouse, trackpad and pen adapters.
//!
//! The public button variants remain compatible with existing applications,
//! but navigation is no longer inferred from platform-specific numeric IDs.

/// Physical mouse button translated by the platform adapter.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum Button {
    /// The primary button, usually the left one.
    Left,
    /// The secondary button, usually the right one.
    Right,
    /// The middle button or wheel click.
    Middle,
    /// The side button that navigates back.
    Back,
    /// The side button that navigates forward.
    Forward,
    /// A vendor-specific button identified by its platform number.
    Other(u16),
}

/// Navigation intent carried by a dedicated mouse side button.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum Navigation {
    /// Go back in history.
    Back,
    /// Go forward in history.
    Forward,
}

impl Button {
    /// Primary activation (click) rather than a context-menu action.
    #[inline]
    pub const fn is_primary(self) -> bool {
        matches!(self, Self::Left)
    }

    /// Secondary context-menu activation, never a primary click.
    #[inline]
    pub const fn is_secondary(self) -> bool {
        matches!(self, Self::Right)
    }

    /// Dedicated history/navigation intent, if any.
    #[inline]
    pub const fn navigation(self) -> Option<Navigation> {
        match self {
            Self::Back => Some(Navigation::Back),
            Self::Forward => Some(Navigation::Forward),
            _ => None,
        }
    }

    /// Whether the device button is known to TAIM without a vendor code.
    #[inline]
    pub const fn is_standard(self) -> bool {
        !matches!(self, Self::Other(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_does_not_confuse_primary_button_or_vendor_buttons() {
        assert_eq!(Button::Back.navigation(), Some(Navigation::Back));
        assert_eq!(Button::Forward.navigation(), Some(Navigation::Forward));
        assert_eq!(Button::Left.navigation(), None);
        assert_eq!(Button::Other(8).navigation(), None);
        assert!(Button::Left.is_primary());
        assert!(!Button::Right.is_primary());
        assert!(Button::Right.is_secondary());
        assert!(!Button::Other(2).is_standard());
    }
}

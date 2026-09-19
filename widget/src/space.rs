//! Add some explicit spacing between elements.
use crate::core::Length;

mod implementation;

/// Creates a new [`Space`] widget that fills the available
/// horizontal space.
///
/// This can be useful to separate widgets in a [`Row`](crate::Row).
pub const fn horizontal() -> Space {
    Space {
        width: Length::Fill,
        height: Length::Shrink,
    }
}

/// Creates a new [`Space`] widget that fills the available
/// vertical space.
///
/// This can be useful to separate widgets in a [`Column`](crate::Column).
pub const fn vertical() -> Space {
    Space {
        width: Length::Shrink,
        height: Length::Fill,
    }
}

/// An amount of empty space.
///
/// It can be useful if you want to fill some space with nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Space {
    pub(super) width: Length,
    pub(super) height: Length,
}

impl Space {
    /// Creates some empty [`Space`] with no size.
    pub const fn new() -> Self {
        Self {
            width: Length::Shrink,
            height: Length::Shrink,
        }
    }

    /// Sets the width of the [`Space`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`Space`].
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }
}

impl Default for Space {
    fn default() -> Self {
        Space::new()
    }
}

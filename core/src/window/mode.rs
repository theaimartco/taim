//! TAIM window presentation state.
//!
//! A hidden window must not consume presentation frames. The visibility
//! contract is owned here and reused by all shell backends.

/// Runtime presentation mode of a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// A regular, decorated desktop window.
    Windowed,
    /// A borderless window covering the whole monitor.
    Fullscreen,
    /// The window exists but is not mapped on screen.
    Hidden,
}

impl Mode {
    /// Request a borderless compositor fullscreen surface.
    #[inline]
    pub const fn requests_fullscreen(self) -> bool {
        matches!(self, Self::Fullscreen)
    }

    /// Whether the native window should be mapped as visible.
    #[inline]
    pub const fn is_visible(self) -> bool {
        !matches!(self, Self::Hidden)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presentation_modes_have_coherent_compositor_intent() {
        assert!(Mode::Windowed.is_visible());
        assert!(!Mode::Windowed.requests_fullscreen());
        assert!(Mode::Fullscreen.requests_fullscreen());
        assert!(Mode::Fullscreen.is_visible());
        assert!(!Mode::Hidden.requests_fullscreen());
        assert!(!Mode::Hidden.is_visible());
    }
}

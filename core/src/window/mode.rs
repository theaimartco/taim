//! TAIM window presentation state.
//!
//! A hidden window must not consume presentation frames. The visibility
//! contract is owned here and reused by all shell backends.

/// Runtime presentation mode of a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Present as an ordinary window managed by the compositor.
    Windowed,
    /// Request fullscreen presentation on the active display.
    Fullscreen,
    /// Keep the surface unmapped and skip visible presentation.
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

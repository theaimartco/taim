/// A window level groups windows with respect to their z-position.
///
/// The relative ordering between windows in different window levels is fixed.
/// The z-order of a window within the same window level may change dynamically
/// on user interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Level {
    /// The default behavior.
    #[default]
    Normal,

    /// The window will always be below normal windows.
    ///
    /// This is useful for a widget-based app.
    AlwaysOnBottom,

    /// The window will always be on top of normal windows.
    AlwaysOnTop,
}

impl Level {
    /// Whether this window is deliberately kept above ordinary application
    /// surfaces (e.g. a floating inspector or always-visible utility).
    #[inline]
    pub const fn is_floating(self) -> bool {
        matches!(self, Self::AlwaysOnTop)
    }

    /// Whether the surface is explicitly intended behind ordinary windows.
    #[inline]
    pub const fn is_background(self) -> bool {
        matches!(self, Self::AlwaysOnBottom)
    }
}

#[cfg(test)]
mod taim_level_tests {
    use super::*;

    #[test]
    fn distinguishes_foreground_and_background_window_intent() {
        assert!(!Level::Normal.is_floating());
        assert!(!Level::Normal.is_background());
        assert!(Level::AlwaysOnTop.is_floating());
        assert!(Level::AlwaysOnBottom.is_background());
    }
}

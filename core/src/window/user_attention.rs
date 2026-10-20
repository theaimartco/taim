/// The type of user attention to request.
///
/// ## Platform-specific
///
/// - **X11:** Sets the WM's `XUrgencyHint`. No distinction between [`Critical`] and [`Informational`].
///
/// [`Critical`]: Self::Critical
/// [`Informational`]: Self::Informational
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserAttention {
    /// ## Platform-specific
    ///
    /// - **macOS:** Bounces the dock icon until the application is in focus.
    /// - **Windows:** Flashes both the window and the taskbar button until the application is in focus.
    Critical,
    /// ## Platform-specific
    ///
    /// - **macOS:** Bounces the dock icon once.
    /// - **Windows:** Flashes the taskbar button until the application is in focus.
    Informational,
}

impl UserAttention {
    /// A critical request should keep drawing attention until the window
    /// receives focus; an informational one is a single-notification hint.
    #[inline]
    pub const fn is_critical(self) -> bool {
        matches!(self, Self::Critical)
    }
}

#[cfg(test)]
mod taim_attention_tests {
    use super::*;

    #[test]
    fn urgent_attention_can_be_distinguished_from_informational() {
        assert!(UserAttention::Critical.is_critical());
        assert!(!UserAttention::Informational.is_critical());
    }
}

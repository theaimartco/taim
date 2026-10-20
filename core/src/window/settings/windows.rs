//! Platform specific settings for Windows.

/// The platform specific window settings of an application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformSpecific {
    /// Drag and drop support
    pub drag_and_drop: bool,

    /// Whether show or hide the window icon in the taskbar.
    pub skip_taskbar: bool,

    /// Shows or hides the background drop shadow for undecorated windows.
    ///
    /// The shadow is hidden by default.
    /// Enabling the shadow causes a thin 1px line to appear on the top of the window.
    pub undecorated_shadow: bool,

    /// Sets the preferred style of the window corners.
    ///
    /// Supported starting with Windows 11 Build 22000.
    pub corner_preference: CornerPreference,
}

impl PlatformSpecific {
    /// Windows should apply this shadow override only for an undecorated
    /// window; decorated frames already receive their OS-managed shadows.
    ///
    /// This avoids asking DWM to add the known thin undecorated-shadow
    /// artifact to otherwise standard titlebar windows.
    #[inline]
    pub const fn effective_undecorated_shadow(self, decorations: bool) -> bool {
        self.undecorated_shadow && !decorations
    }
}

impl Default for PlatformSpecific {
    fn default() -> Self {
        Self {
            drag_and_drop: true,
            skip_taskbar: false,
            undecorated_shadow: false,
            corner_preference: Default::default(),
        }
    }
}

/// Describes how the corners of a window should look like.
#[repr(i32)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CornerPreference {
    /// Corresponds to `DWMWCP_DEFAULT`.
    ///
    /// Let the system decide when to round window corners.
    #[default]
    Default = 0,

    /// Corresponds to `DWMWCP_DONOTROUND`.
    ///
    /// Never round window corners.
    DoNotRound = 1,

    /// Corresponds to `DWMWCP_ROUND`.
    ///
    /// Round the corners, if appropriate.
    Round = 2,

    /// Corresponds to `DWMWCP_ROUNDSMALL`.
    ///
    /// Round the corners if appropriate, with a small radius.
    RoundSmall = 3,
}

#[cfg(test)]
mod taim_windows_shadow_tests {
    use super::*;

    #[test]
    fn custom_shadow_only_applies_to_undecorated_windows() {
        let mut settings = PlatformSpecific::default();
        assert!(!settings.effective_undecorated_shadow(false));
        settings.undecorated_shadow = true;
        assert!(settings.effective_undecorated_shadow(false));
        assert!(!settings.effective_undecorated_shadow(true));
    }
}

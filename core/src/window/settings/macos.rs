//! Platform specific settings for macOS.

/// The platform specific window settings of an application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlatformSpecific {
    /// Hides the window title.
    pub title_hidden: bool,
    /// Makes the titlebar transparent and allows the content to appear behind it.
    pub titlebar_transparent: bool,
    /// Makes the window content appear behind the titlebar.
    pub fullsize_content_view: bool,
}

impl PlatformSpecific {
    /// Canonical macOS titlebar flags for the actual window attributes.
    ///
    /// A transparent titlebar needs full-size content to make its underlying
    /// scene visible. Decorless windows do not have a native titlebar.
    #[inline]
    pub const fn normalized(mut self, decorations: bool) -> Self {
        if !decorations {
            self.title_hidden = false;
            self.titlebar_transparent = false;
            self.fullsize_content_view = false;
        } else if self.titlebar_transparent {
            self.fullsize_content_view = true;
        }
        self
    }
}

#[cfg(test)]
mod taim_macos_titlebar_tests {
    use super::*;

    #[test]
    fn transparent_titlebar_requires_fullsize_content() {
        let cfg = PlatformSpecific {
            titlebar_transparent: true,
            ..Default::default()
        }
        .normalized(true);
        assert!(cfg.fullsize_content_view);
        assert!(cfg.titlebar_transparent);
    }

    #[test]
    fn undecorated_window_ignores_inapplicable_titlebar_flags() {
        let cfg = PlatformSpecific {
            title_hidden: true,
            titlebar_transparent: true,
            fullsize_content_view: true,
        }
        .normalized(false);
        assert_eq!(cfg, PlatformSpecific::default());
    }
}

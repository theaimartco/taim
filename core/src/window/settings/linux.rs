//! Platform specific settings for Linux.

/// The platform specific window settings of an application.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlatformSpecific {
    /// Sets the application id of the window.
    ///
    /// As a best practice, it is suggested to select an application id that match
    /// the basename of the application’s .desktop file.
    pub application_id: String,

    /// Whether bypass the window manager mapping for x11 windows
    ///
    /// This flag is particularly useful for creating UI elements that need precise
    /// positioning and immediate display without window manager interference.
    pub override_redirect: bool,
}

impl PlatformSpecific {
    /// Return a compositor-safe window application identity.
    ///
    /// Invalid IDs are excluded rather than interpolated into Wayland app-id
    /// and X11 WM_CLASS requests. An unset ID leaves identity to the shell.
    /// The maximum prevents pathological client-provided strings from
    /// reaching window-system protocols.
    pub fn valid_application_id(&self) -> Option<&str> {
        let id = self.application_id.as_str();
        if id.is_empty()
            || id.len() > 255
            || id.starts_with('.')
            || id.ends_with('.')
            || id.as_bytes().iter().any(|byte| {
                !matches!(
                    *byte,
                    b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'-' | b'_'
                )
            })
        {
            return None;
        }
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverse_dns_ids_are_accepted_and_control_bytes_rejected() {
        let mut settings = PlatformSpecific::default();
        assert_eq!(settings.valid_application_id(), None);
        settings.application_id = "co.theaimart.VYOM_Desktop".into();
        assert_eq!(settings.valid_application_id(), Some("co.theaimart.VYOM_Desktop"));
        for invalid in [".leading", "trailing.", "spaced app", "evil\nvalue"] {
            settings.application_id = invalid.into();
            assert_eq!(settings.valid_application_id(), None);
        }
        settings.application_id = "x".repeat(256);
        assert_eq!(settings.valid_application_id(), None);
    }
}

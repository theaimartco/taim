//! Backend creation settings.

use super::{Backend, PowerPreference};

/// Graphics backend configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Backend preference.
    pub backend: Backend,
    /// Adapter power policy.
    pub power_preference: PowerPreference,
    /// Enables antialiasing for supported primitives.
    pub antialiasing: bool,
    /// Synchronizes presentation with the display.
    pub vsync: bool,
}

impl Settings {
    /// Selects a backend.
    #[inline]
    pub fn with_backend(mut self, backend: impl Into<Backend>) -> Self {
        self.backend = backend.into();
        self
    }

    /// Selects an adapter power policy.
    #[inline]
    pub const fn with_power_preference(mut self, preference: PowerPreference) -> Self {
        self.power_preference = preference;
        self
    }

    /// Enables or disables vertical synchronization.
    #[inline]
    pub const fn with_vsync(mut self, enabled: bool) -> Self {
        self.vsync = enabled;
        self
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            backend: Backend::Best,
            power_preference: PowerPreference::None,
            antialiasing: true,
            vsync: true,
        }
    }
}

impl From<&crate::Settings> for Settings {
    fn from(settings: &crate::Settings) -> Self {
        Self {
            backend: settings.backend.clone(),
            power_preference: settings.power_preference,
            antialiasing: settings.antialiasing,
            vsync: settings.vsync,
        }
    }
}

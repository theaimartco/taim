//! Runtime protocol negotiation.

use std::collections::BTreeMap;

/// One compositor global and its highest advertised version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Global {
    /// Registry interface name.
    pub interface: String,
    /// Highest version advertised for this instance.
    pub version: u32,
}

/// Capabilities understood by the native backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Capability {
    /// Core surface compositor.
    Compositor,
    /// Stable desktop shell.
    XdgShell,
    /// Keyboard, pointer, and touch seat.
    Seat,
    /// Shared-memory buffers.
    SharedMemory,
    /// Clipboard and drag-and-drop data device.
    DataDevice,
    /// Primary selection protocol.
    PrimarySelection,
    /// Text-input-v3 protocol.
    TextInput,
    /// Per-frame presentation timestamps.
    PresentationFeedback,
    /// Fractional output scaling.
    FractionalScale,
    /// Viewport cropping and scaling.
    Viewporter,
    /// Compositor pointer gesture events.
    PointerGestures,
    /// WLR layer-shell surfaces.
    LayerShell,
    /// Standard secure session lock.
    SessionLock,
    /// Idle notification.
    IdleNotify,
    /// Idle inhibition.
    IdleInhibit,
    /// Per-output colour description: primaries, transfer, luminance range.
    ///
    /// Without it the backend cannot know what panel it is presenting to, and
    /// the renderer tone maps into an assumed standard-range display — giving
    /// up an HDR panel's entire headroom.
    ColorManagement,
    /// Authenticated VYOM secure-shell extension.
    VyomSecureShell,
    /// VYOM power and thermal extension.
    VyomPower,
    /// VYOM accessibility transport.
    VyomAccessibility,
}

impl Capability {
    /// Protocol interface and minimum supported version.
    pub const fn requirement(self) -> (&'static str, u32) {
        match self {
            Self::Compositor => ("wl_compositor", 4),
            Self::XdgShell => ("xdg_wm_base", 1),
            Self::Seat => ("wl_seat", 5),
            Self::SharedMemory => ("wl_shm", 1),
            Self::DataDevice => ("wl_data_device_manager", 3),
            Self::PrimarySelection => ("zwp_primary_selection_device_manager_v1", 1),
            Self::TextInput => ("zwp_text_input_manager_v3", 1),
            Self::PresentationFeedback => ("wp_presentation", 1),
            Self::FractionalScale => ("wp_fractional_scale_manager_v1", 1),
            Self::Viewporter => ("wp_viewporter", 1),
            Self::PointerGestures => ("zwp_pointer_gestures_v1", 3),
            Self::LayerShell => ("zwlr_layer_shell_v1", 4),
            Self::SessionLock => ("ext_session_lock_manager_v1", 1),
            Self::IdleNotify => ("ext_idle_notifier_v1", 1),
            Self::IdleInhibit => ("zwp_idle_inhibit_manager_v1", 1),
            Self::ColorManagement => ("wp_color_manager_v1", 1),
            Self::VyomSecureShell => ("vyom_secure_shell_v1", 1),
            Self::VyomPower => ("vyom_power_manager_v1", 1),
            Self::VyomAccessibility => ("vyom_accessibility_manager_v1", 1),
        }
    }
}

/// Immutable, deduplicated compositor capability snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Capabilities {
    globals: BTreeMap<String, u32>,
}

impl Capabilities {
    /// Builds a snapshot, retaining the highest version of duplicate globals.
    pub fn new(globals: impl IntoIterator<Item = Global>) -> Self {
        let mut versions = BTreeMap::<String, u32>::new();

        for global in globals {
            let _ = versions
                .entry(global.interface)
                .and_modify(|version| *version = (*version).max(global.version))
                .or_insert(global.version);
        }

        Self { globals: versions }
    }

    /// Returns whether the required protocol and minimum version exist.
    pub fn supports(&self, capability: Capability) -> bool {
        let (interface, minimum) = capability.requirement();
        self.version(interface)
            .is_some_and(|version| version >= minimum)
    }

    /// Returns the highest advertised version of an interface.
    pub fn version(&self, interface: &str) -> Option<u32> {
        self.globals.get(interface).copied()
    }

    /// Iterates over the deduplicated protocol registry.
    pub fn globals(&self) -> impl ExactSizeIterator<Item = (&str, u32)> {
        self.globals
            .iter()
            .map(|(interface, version)| (interface.as_str(), *version))
    }

    /// Iterates over core capabilities absent from the compositor.
    pub fn missing_core(&self) -> impl Iterator<Item = Capability> + '_ {
        [
            Capability::Compositor,
            Capability::XdgShell,
            Capability::Seat,
        ]
        .into_iter()
        .filter(|capability| !self.supports(*capability))
    }
}

#[cfg(unix)]
impl From<Vec<wayland_client::globals::Global>> for Capabilities {
    fn from(globals: Vec<wayland_client::globals::Global>) -> Self {
        Self::new(globals.into_iter().map(|global| Global {
            interface: global.interface,
            version: global.version,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_globals_keep_the_highest_version() {
        let capabilities = Capabilities::new([
            Global {
                interface: "wl_seat".into(),
                version: 2,
            },
            Global {
                interface: "wl_seat".into(),
                version: 8,
            },
        ]);

        assert_eq!(capabilities.version("wl_seat"), Some(8));
        assert!(capabilities.supports(Capability::Seat));
    }

    #[test]
    fn versions_below_the_backend_minimum_are_not_supported() {
        let capabilities = Capabilities::new([Global {
            interface: "wl_compositor".into(),
            version: 3,
        }]);

        assert!(!capabilities.supports(Capability::Compositor));
    }
}

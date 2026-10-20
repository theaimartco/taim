//! Access the native system.
use crate::core::theme;
use crate::futures::futures::channel::oneshot;
use crate::futures::subscription::{self, Subscription};
use crate::task::{self, Task};

/// An operation to be performed on the system.
#[derive(Debug)]
pub enum Action {
    /// Send available system information.
    GetInformation(oneshot::Sender<Information>),

    /// Send the current system theme mode.
    GetTheme(oneshot::Sender<theme::Mode>),

    /// Notify to the runtime that the system theme has changed.
    NotifyTheme(theme::Mode),
}

/// Contains information about the system (e.g. system name, processor, memory, graphics adapter).
#[derive(Clone, Debug)]
pub struct Information {
    /// The operating system name
    pub system_name: Option<String>,
    /// Operating system kernel version
    pub system_kernel: Option<String>,
    /// Long operating system version
    ///
    /// Examples:
    /// - MacOS 10.15 Catalina
    /// - Windows 10 Pro
    /// - Ubuntu 20.04 LTS (Focal Fossa)
    pub system_version: Option<String>,
    /// Short operating system version number
    pub system_short_version: Option<String>,
    /// Detailed processor model information
    pub cpu_brand: String,
    /// The number of physical cores on the processor
    pub cpu_cores: Option<usize>,
    /// Total RAM size, in bytes
    pub memory_total: u64,
    /// Memory used by this process, in bytes
    pub memory_used: Option<u64>,
    /// Underlying graphics backend for rendering
    pub graphics_backend: String,
    /// Model information for the active graphics adapter
    pub graphics_adapter: String,
}

impl Information {
    /// Never report fewer than one CPU for worker-pool sizing.
    pub fn effective_cpu_cores(&self) -> usize {
        self.cpu_cores.unwrap_or(1).max(1)
    }

    /// Best-effort process-memory pressure, clipped to the range [0,1].
    /// Missing measurements or zero total memory return None.
    pub fn process_memory_fraction(&self) -> Option<f64> {
        if self.memory_total == 0 {
            return None;
        }
        let used = self.memory_used?;
        Some((used as f64 / self.memory_total as f64).min(1.0))
    }
}

/// Returns available system information.
pub fn information() -> Task<Information> {
    task::oneshot(|channel| crate::Action::System(Action::GetInformation(channel)))
}

/// Returns the current system theme.
pub fn theme() -> Task<theme::Mode> {
    task::oneshot(|sender| crate::Action::System(Action::GetTheme(sender)))
}

/// Subscribes to system theme changes.
pub fn theme_changes() -> Subscription<theme::Mode> {
    #[derive(Hash)]
    struct ThemeChanges;

    subscription::filter_map(ThemeChanges, |event| {
        let subscription::Event::SystemThemeChanged(mode) = event else {
            return None;
        };

        Some(mode)
    })
}

#[cfg(test)]
mod taim_system_tests {
    use super::*;

    #[test]
    fn handles_unavailable_topology_without_zero_workers() {
        let mut info = Information {
            system_name: None,
            system_kernel: None,
            system_version: None,
            system_short_version: None,
            cpu_brand: String::new(),
            cpu_cores: Some(0),
            memory_total: 0,
            memory_used: Some(100),
            graphics_backend: String::new(),
            graphics_adapter: String::new(),
        };
        assert_eq!(info.effective_cpu_cores(), 1);
        assert_eq!(info.process_memory_fraction(), None);
        info.memory_total = 80;
        assert_eq!(info.process_memory_fraction(), Some(1.0));
        info.cpu_cores = Some(12);
        assert_eq!(info.effective_cpu_cores(), 12);
    }
}

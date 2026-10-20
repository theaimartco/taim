//! Configure a [`Backend`](crate::core::Backend) at runtime.
use crate::Task;
use crate::core::backend;
use crate::futures::futures::channel::oneshot;
use crate::task;

/// An backend operation.
#[derive(Debug)]
pub enum Action {
    /// Switches the [`backend::Settings`] of the current application.
    Configure(
        backend::Settings,
        oneshot::Sender<Result<(), backend::Error>>,
    ),
}

/// Returns a [`Task`] that switches the [`backend::Settings`] of the current application.
///
/// This can be leveraged to switch renderers at runtime.
pub fn configure(settings: backend::Settings) -> Task<Result<(), backend::Error>> {
    task::oneshot(|sender| crate::Action::Backend(Action::Configure(settings, sender)))
}

/// Avoid redundant GPU reconfiguration when the requested settings are
/// identical to the active renderer's configuration.
pub fn configure_if_changed(
    current: &backend::Settings,
    requested: backend::Settings,
) -> Option<Task<Result<(), backend::Error>>> {
    (current != &requested).then(|| configure(requested))
}

#[cfg(test)]
mod taim_backend_tests {
    use super::*;

    #[test]
    fn identical_renderer_settings_do_not_issue_a_gpu_reset() {
        let original = backend::Settings::default();
        assert!(configure_if_changed(&original, original.clone()).is_none());
        let changed = original.clone().with_vsync(!original.vsync);
        assert!(configure_if_changed(&original, changed).is_some());
    }
}

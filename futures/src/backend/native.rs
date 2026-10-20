//! Backends that are only available in native platforms: Windows, macOS, or Linux.
#[cfg(feature = "tokio")]
pub mod tokio;

#[cfg(feature = "smol")]
pub mod smol;

#[cfg(feature = "thread-pool")]
pub mod thread_pool;

/// The executor selected by TAIM's default-backend feature precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedBackend {
    /// TAIM runs futures on the Tokio backend.
    Tokio,
    /// TAIM runs futures on the smol backend.
    Smol,
    /// TAIM runs futures on its instrumented native worker pool.
    ThreadPool,
    /// No usable native async executor feature was enabled.
    Unavailable,
}

/// Report the effective native async engine without launching worker threads.
///
/// This order must match `backend::default`; it prevents diagnostics from
/// reporting an enabled but lower-priority executor.
pub const fn selected_backend() -> SelectedBackend {
    if cfg!(feature = "tokio") {
        SelectedBackend::Tokio
    } else if cfg!(feature = "smol") {
        SelectedBackend::Smol
    } else if cfg!(feature = "thread-pool") {
        SelectedBackend::ThreadPool
    } else {
        SelectedBackend::Unavailable
    }
}

#[cfg(test)]
mod taim_backend_selection_tests {
    use super::*;

    #[test]
    fn selection_follows_the_same_feature_precedence_as_runtime() {
        let expected = if cfg!(feature = "tokio") {
            SelectedBackend::Tokio
        } else if cfg!(feature = "smol") {
            SelectedBackend::Smol
        } else if cfg!(feature = "thread-pool") {
            SelectedBackend::ThreadPool
        } else {
            SelectedBackend::Unavailable
        };
        assert_eq!(selected_backend(), expected);
    }
}

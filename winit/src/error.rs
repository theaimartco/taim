use crate::core::backend;
use crate::futures::futures;

/// An error that occurred while running an application.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The futures executor could not be created.
    #[error("TAIM window executor failed: {0}")]
    ExecutorCreationFailed(#[source] futures::io::Error),

    /// The application window could not be created.
    #[error("TAIM window creation failed: {0}")]
    WindowCreationFailed(#[source] winit::error::OsError),

    /// The application graphics context could not be created.
    #[error("TAIM window graphics initialization failed: {0}")]
    GraphicsCreationFailed(#[source] backend::Error),
}

impl From<backend::Error> for Error {
    fn from(error: backend::Error) -> Error {
        Error::GraphicsCreationFailed(error)
    }
}

#[cfg(test)]
mod taim_window_error_chain_tests {
    use super::*;

    #[test]
    fn async_initialization_error_has_a_root_cause() {
        use std::error::Error as _;
        let err =
            Error::ExecutorCreationFailed(futures::io::Error::other("worker backend unavailable"));
        assert!(err.to_string().contains("worker backend unavailable"));
        assert!(err.source().is_some());
    }
}

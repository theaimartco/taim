use crate::core::backend;

/// An error that occurred while running an application.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The platform event loop could not start or failed while running.
    #[error("TAIM event loop failed: {0}")]
    EventLoop(#[source] winit::error::EventLoopError),

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
        let err = Error::GraphicsCreationFailed(backend::Error::BackendError(
            "worker backend unavailable".to_owned(),
        ));
        assert!(err.to_string().contains("graphics"));
        assert!(err.source().is_some());
    }
}

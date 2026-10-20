use crate::backend;
use crate::futures;
use crate::shell;

/// An error that occurred while running an application.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The futures executor could not be created.
    #[error("TAIM executor startup failed: {0}")]
    ExecutorCreationFailed(#[source] futures::io::Error),

    /// The application window could not be created.
    #[error("TAIM native window creation failed: {0}")]
    WindowCreationFailed(#[source] Box<dyn std::error::Error + Send + Sync>),

    /// The application graphics context could not be created.
    #[error("TAIM graphics initialization failed: {0}")]
    GraphicsCreationFailed(#[source] backend::Error),
}

impl From<shell::Error> for Error {
    fn from(error: shell::Error) -> Error {
        match error {
            shell::Error::ExecutorCreationFailed(error) => Error::ExecutorCreationFailed(error),
            shell::Error::WindowCreationFailed(error) => {
                Error::WindowCreationFailed(Box::new(error))
            }
            shell::Error::GraphicsCreationFailed(error) => Error::GraphicsCreationFailed(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assert_send_sync() {
        fn _assert<T: Send + Sync>() {}
        _assert::<Error>();
    }
}

#[cfg(test)]
mod taim_error_chain_tests {
    use super::*;

    #[test]
    fn executor_failure_preserves_original_operating_system_reason() {
        use std::error::Error as _;
        let err = Error::ExecutorCreationFailed(futures::io::Error::other("no worker threads"));
        assert!(err.to_string().contains("no worker threads"));
        assert!(err.source().unwrap().to_string().contains("no worker threads"));
    }
}

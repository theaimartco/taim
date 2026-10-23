use crate::backend;

/// Why an application could not run.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The platform's event loop could not start or failed while running.
    #[error("TAIM event loop failed: {0}")]
    EventLoop(#[source] Box<dyn std::error::Error + Send + Sync>),

    /// The application window could not be created.
    #[error("TAIM native window creation failed: {0}")]
    WindowCreationFailed(#[source] Box<dyn std::error::Error + Send + Sync>),

    /// The application graphics context could not be created.
    #[error("TAIM graphics initialization failed: {0}")]
    GraphicsCreationFailed(#[source] backend::Error),
}

impl From<taim_winit::Error> for Error {
    fn from(error: taim_winit::Error) -> Error {
        match error {
            taim_winit::Error::EventLoop(error) => Error::EventLoop(Box::new(error)),
            taim_winit::Error::WindowCreationFailed(error) => {
                Error::WindowCreationFailed(Box::new(error))
            }
            taim_winit::Error::GraphicsCreationFailed(error) => {
                Error::GraphicsCreationFailed(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_cross_threads() {
        fn _assert<T: Send + Sync>() {}
        _assert::<Error>();
    }

    #[test]
    fn window_failures_keep_their_cause() {
        use std::error::Error as _;
        let err = Error::WindowCreationFailed("no display".into());
        assert!(err.to_string().contains("no display"));
        assert!(err.source().is_some());
    }
}

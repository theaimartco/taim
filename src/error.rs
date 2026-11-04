use crate::backend;

/// Why an application could not run.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Android requires the activity passed to the native entry point.
    #[cfg(target_os = "android")]
    #[error("Android requires app(...).run_android(activity) from android_main")]
    AndroidActivityRequired,

    /// The platform's event loop could not start or failed while running.
    #[error("TAIM event loop failed: {0}")]
    EventLoop(#[source] Box<dyn std::error::Error + Send + Sync>),

    /// The application window could not be created.
    #[error("TAIM native window creation failed: {0}")]
    WindowCreationFailed(#[source] Box<dyn std::error::Error + Send + Sync>),

    /// The application graphics context could not be created.
    #[error("TAIM graphics initialization failed: {0}")]
    GraphicsCreationFailed(#[source] backend::Error),

    /// A fatal presentation failure stopped the application.
    #[error("TAIM frame presentation failed: {0}")]
    PresentationFailed(#[source] taim_renderer::graphics::compositor::SurfaceError),
}

impl From<taim_winit::Error> for Error {
    fn from(error: taim_winit::Error) -> Error {
        match error {
            #[cfg(target_os = "android")]
            taim_winit::Error::AndroidActivityRequired => Error::AndroidActivityRequired,
            taim_winit::Error::EventLoop(error) => Error::EventLoop(Box::new(error)),
            taim_winit::Error::WindowCreationFailed(error) => {
                Error::WindowCreationFailed(Box::new(error))
            }
            taim_winit::Error::GraphicsCreationFailed(error) => {
                Error::GraphicsCreationFailed(error)
            }
            taim_winit::Error::PresentationFailed(error) => Error::PresentationFailed(error),
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

    #[test]
    fn fatal_presentation_errors_reach_application_callers_with_their_cause() {
        use std::error::Error as _;
        use taim_renderer::graphics::compositor::SurfaceError;
        let error: Error = taim_winit::Error::PresentationFailed(SurfaceError::OutOfMemory).into();
        assert!(error.to_string().contains("ran out of memory"));
        assert!(error.source().unwrap().is::<SurfaceError>());
    }
}

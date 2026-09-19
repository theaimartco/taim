//! Backend initialization failures.

use super::Backend;

use std::fmt;

/// An error raised while creating a graphics context.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Requested backend version is unsupported.
    #[error("the requested backend version is not supported")]
    VersionNotSupported,
    /// No compatible surface pixel format exists.
    #[error("failed to find any pixel format that matches the criteria")]
    NoAvailablePixelFormat,
    /// No suitable adapter or device was found.
    #[error("a suitable graphics adapter could not be found: {reason}")]
    GraphicsAdapterNotFound {
        /// Backend that failed.
        backend: &'static str,
        /// Failure reason.
        reason: Reason,
    },
    /// Backend-specific failure.
    #[error("an error occurred in the context's internal backend")]
    BackendError(String),
    /// Several fallback backends failed.
    #[error("multiple errors occurred:\n{}", error_list(.0))]
    List(Vec<Self>),
}

/// Why adapter selection failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// Candidate did not match the requested backend.
    DidNotMatch {
        /// Requested backend.
        preferred_backend: Backend,
    },
    /// Device or adapter request failed.
    RequestFailed(String),
}

impl fmt::Display for Reason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DidNotMatch { preferred_backend } => write!(
                formatter,
                "the backend did not match the preference: {preferred_backend}"
            ),
            Self::RequestFailed(error) => formatter.write_str(error),
        }
    }
}

fn error_list(errors: &[Error]) -> String {
    use fmt::Write as _;

    let mut list = String::with_capacity(errors.len().saturating_mul(48));
    for error in errors {
        let _ = writeln!(list, "- {error}");
    }
    list
}

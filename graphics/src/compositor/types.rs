use crate::futures::{MaybeSend, MaybeSync};

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use thiserror::Error;

use std::fmt::Debug;

/// A window accepted by a graphics compositor.
pub trait Window: HasWindowHandle + Debug + MaybeSend + MaybeSync + 'static {}

impl<T> Window for T where T: HasWindowHandle + Debug + MaybeSend + MaybeSync + 'static {}

/// An owned display handle accepted by a graphics compositor.
pub trait Display: HasDisplayHandle + Debug + Send + Sync + 'static {}

impl<T> Display for T where T: HasDisplayHandle + Debug + Send + Sync + 'static {}

/// A surface acquisition or presentation failure.
#[derive(Clone, PartialEq, Eq, Debug, Error)]
pub enum SurfaceError {
    /// The surface did not produce a frame before its deadline.
    #[error("surface acquisition timed out")]
    Timeout,
    /// The surface configuration is no longer current.
    #[error("surface configuration is outdated")]
    Outdated,
    /// The swap chain was lost and must be recreated.
    #[error("surface was lost")]
    Lost,
    /// The graphics backend ran out of memory.
    #[error("graphics backend ran out of memory")]
    OutOfMemory,
    /// The platform reports that the surface is occluded.
    #[error("surface is occluded")]
    Occluded,
    /// A backend-specific presentation failure occurred.
    #[error("surface presentation failed")]
    Other,
}

/// Human-readable graphics adapter and backend information.
#[derive(Debug)]
pub struct Information {
    /// Selected graphics adapter.
    pub adapter: String,
    /// Selected graphics backend.
    pub backend: String,
}

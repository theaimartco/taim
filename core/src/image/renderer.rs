use crate::{Rectangle, Size};

use std::io;
use std::sync::Arc;

use super::{Allocation, Image};

/// A renderer capable of loading and drawing raster images.
pub trait Renderer: crate::Renderer {
    /// Renderer-specific image handle.
    type Handle: Clone;

    /// Loads an image synchronously and retains its allocation.
    fn load_image(&self, handle: &Self::Handle) -> Result<Allocation, Error>;

    /// Returns cached dimensions, or `None` while an asynchronous load is pending.
    fn measure_image(&self, handle: &Self::Handle) -> Option<Size<u32>>;

    /// Draws an image within the supplied bounds and clip rectangle.
    fn draw_image(&mut self, image: Image<Self::Handle>, bounds: Rectangle, clip_bounds: Rectangle);
}

/// An image loading failure.
#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    /// Image data could not be decoded.
    #[error("the image data was invalid or could not be decoded: {0}")]
    Invalid(Arc<dyn std::error::Error + Send + Sync>),
    /// The image file could not be accessed.
    #[error("the image file could not be opened: {0}")]
    Inaccessible(Arc<io::Error>),
    /// Loading images is unsupported.
    #[error("loading images is unsupported")]
    Unsupported,
    /// The image contains no pixels.
    #[error("the image is empty")]
    Empty,
    /// Renderer memory could not be allocated.
    #[error("not enough memory to allocate the image")]
    OutOfMemory,
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Inaccessible(Arc::new(error))
    }
}

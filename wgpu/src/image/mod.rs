//! TAIM image rendering: ordered batching, bounded caching, and GPU atlases.

pub(crate) mod cache;
pub(crate) use cache::Cache;

mod atlas;
mod batch;
mod pipeline;

#[cfg(feature = "image")]
mod raster;

#[cfg(feature = "image")]
#[cfg(feature = "svg")]
mod vector;

pub use crate::graphics::Image;
pub use batch::State;
pub use pipeline::Pipeline;

/// Images submitted together for one ordered render layer.
pub type Batch = Vec<Image>;

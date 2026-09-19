//! TAIM-native vector geometry recording and tessellation.

mod buffers;
mod cache;
mod dash;
mod frame;
mod transform;

pub use cache::{Cache, Geometry};
pub use frame::Frame;

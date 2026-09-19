//! TAIM's streamed and retained triangle renderer.

mod batch;
mod draw;
mod msaa;
mod pipeline;
mod storage;
mod uniforms;

pub use batch::{Batch, Item, State};
pub use pipeline::Pipeline;
pub use storage::Storage;
pub use uniforms::Uniforms;

mod batch;
mod cache;
mod pipeline;
mod prepare;
mod state;
mod storage;

pub use batch::{Batch, Item};
pub use cache::Cache;
pub use pipeline::{Pipeline, Viewport};
pub use state::State;
pub use storage::Storage;

pub use crate::graphics::Text;

use crate::graphics::color;
use taim_text_gpu as gpu;

pub(super) const COLOR_MODE: gpu::ColorMode = if color::GAMMA_CORRECTION {
    gpu::ColorMode::Accurate
} else {
    gpu::ColorMode::Web
};

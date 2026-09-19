//! TAIM text storage, shaping, and backend conversion.

pub mod cache;
pub mod editor;
pub mod paragraph;

mod conversion;
mod font_system;
mod layout;
mod primitive;

pub use cosmic_text;

pub use cache::Cache;
pub use conversion::{hint_factor, to_attributes, to_color, to_ellipsize, to_shaping, to_wrap};
pub use editor::Editor;
pub(crate) use font_system::font_version;
pub use font_system::{FIRA_SANS_REGULAR, FontSystem, Version, font_system};
pub use layout::{align, measure};
pub use paragraph::Paragraph;
pub use primitive::{Raw, Text};

use crate::core::text::{Alignment, Ellipsis, Shaping, Wrapping};

/// Optional shaping-cache telemetry exposed by a renderer.
pub trait CacheStatistics {
    /// Returns statistics when the renderer owns a compatible text cache.
    fn text_cache_statistics(&self) -> Option<cache::Statistics> {
        None
    }
}

/// Draws a raw, externally retained text buffer.
pub trait Renderer {
    /// Records a raw text primitive.
    fn fill_raw(&mut self, raw: Raw);
}

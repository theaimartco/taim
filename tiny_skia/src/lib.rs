#![allow(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod window;

mod backend;
mod engine;

/// The colour-space conversions this backend composites through.
///
/// Exposed so `tests/color/contract.rs` can pin them: they are the pair that
/// decides whether authored bytes reach the working space correctly.
pub use engine::{decode_channel, decode_premultiplied, encode_channel};

mod layer;
mod primitive;
mod text;

#[cfg(feature = "image")]
mod raster;

#[cfg(feature = "svg")]
mod vector;

#[cfg(feature = "geometry")]
pub mod geometry;

pub use taim_graphics as graphics;
pub use taim_graphics::core;

pub use layer::Layer;
pub use primitive::Primitive;

#[cfg(feature = "geometry")]
pub use geometry::Geometry;

use backend::{ActiveRetainedFrame, RecordingRetainedFrame, RetainedFrame};
use core::renderer;
use engine::Engine;

/// A [`tiny-skia`] graphics renderer for [`taim`].
///
/// [`tiny-skia`]: https://github.com/RazrFalcon/tiny-skia
/// [`taim`]: https://github.com/theaimartco/taim
#[derive(Debug)]
pub struct Renderer {
    settings: renderer::Settings,
    layers: layer::Stack,
    retained_frames: graphics::retained_layer::RetainedStore<RetainedFrame>,
    active_retained_frames: Vec<ActiveRetainedFrame>,
    recording_retained_frames: Vec<RecordingRetainedFrame>,
    retained_layer_statistics: core::renderer::RetainedLayerStatistics,
    engine: Engine,
}

impl Renderer {
    pub fn new(settings: renderer::Settings) -> Self {
        Self {
            settings,
            layers: layer::Stack::new(),
            retained_frames: graphics::retained_layer::RetainedStore::with_budget(
                graphics::retained_layer::RetainedLayerCache::DEFAULT_BUDGET,
            ),
            active_retained_frames: Vec::new(),
            recording_retained_frames: Vec::new(),
            retained_layer_statistics: core::renderer::RetainedLayerStatistics::default(),
            engine: Engine::new(),
        }
    }

    pub fn layers(&mut self) -> &[Layer] {
        self.layers.flush();
        self.layers.as_slice()
    }
}

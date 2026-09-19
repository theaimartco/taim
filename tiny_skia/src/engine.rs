mod clip;
mod image;
mod path;
mod primitive;
mod quad;
mod shadow;
mod text;

mod color;
pub use color::{decode_channel, decode_premultiplied, encode_channel};
pub(crate) use color::{into_color, working_components};

#[derive(Debug)]
pub struct Engine {
    text_pipeline: crate::text::Pipeline,
    shadow_cache: shadow::Cache,
    clip: clip::Tracker,

    #[cfg(feature = "image")]
    pub(crate) raster_pipeline: crate::raster::Pipeline,
    #[cfg(feature = "svg")]
    pub(crate) vector_pipeline: crate::vector::Pipeline,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            text_pipeline: crate::text::Pipeline::new(),
            shadow_cache: shadow::Cache::new(),
            clip: clip::Tracker::new(),
            #[cfg(feature = "image")]
            raster_pipeline: crate::raster::Pipeline::new(),
            #[cfg(feature = "svg")]
            vector_pipeline: crate::vector::Pipeline::new(),
        }
    }

    pub fn begin_frame(&mut self) {
        self.clip.begin_frame();
    }

    pub fn adjust_clip_mask(&mut self, mask: &mut tiny_skia::Mask, bounds: crate::core::Rectangle) {
        self.clip.set(mask, bounds);
    }

    pub fn trim(&mut self) {
        self.text_pipeline.trim_cache();
        self.shadow_cache.trim();

        #[cfg(feature = "image")]
        self.raster_pipeline.trim_cache();
        #[cfg(feature = "svg")]
        self.vector_pipeline.trim_cache();
    }

    pub fn text_cache_statistics(&self) -> crate::graphics::text::cache::Statistics {
        self.text_pipeline.cache_statistics()
    }
}

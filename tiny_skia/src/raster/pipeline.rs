use super::cache;
use crate::core::image as raster;
use crate::core::{Rectangle, Size};

use std::cell::RefCell;

#[derive(Debug)]
pub struct Pipeline {
    cache: RefCell<cache::Cache>,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            cache: RefCell::new(cache::Cache::default()),
        }
    }

    pub fn load(&self, handle: &raster::Handle) -> Result<raster::Allocation, raster::Error> {
        let mut cache = self.cache.borrow_mut();
        let image = cache.allocate(handle)?;

        #[allow(unsafe_code)]
        Ok(unsafe { raster::allocate(handle, Size::new(image.width(), image.height())) })
    }

    pub fn dimensions(&self, handle: &raster::Handle) -> Option<Size<u32>> {
        let mut cache = self.cache.borrow_mut();
        let image = cache.allocate(handle).ok()?;
        Some(Size::new(image.width(), image.height()))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        handle: &raster::Handle,
        filter_method: raster::FilterMethod,
        bounds: Rectangle,
        opacity: f32,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        transform: tiny_skia::Transform,
        clip_mask: Option<&tiny_skia::Mask>,
    ) {
        if opacity <= 0.0
            || !opacity.is_finite()
            || bounds.width <= 0.0
            || bounds.height <= 0.0
            || !bounds.width.is_finite()
            || !bounds.height.is_finite()
        {
            return;
        }

        let mut cache = self.cache.borrow_mut();
        let Ok(image) = cache.allocate(handle) else {
            return;
        };
        let scale_x = bounds.width / image.width() as f32;
        let scale_y = bounds.height / image.height() as f32;
        if !scale_x.is_finite() || !scale_y.is_finite() {
            return;
        }

        let quality = match filter_method {
            raster::FilterMethod::Linear => tiny_skia::FilterQuality::Bilinear,
            raster::FilterMethod::Nearest => tiny_skia::FilterQuality::Nearest,
        };
        pixels.draw_pixmap(
            (bounds.x / scale_x).round() as i32,
            (bounds.y / scale_y).round() as i32,
            image,
            &tiny_skia::PixmapPaint {
                quality,
                opacity: opacity.clamp(0.0, 1.0),
                ..Default::default()
            },
            transform.pre_scale(scale_x, scale_y),
            clip_mask,
        );
    }

    pub fn trim_cache(&mut self) {
        self.cache.borrow_mut().trim();
    }
}

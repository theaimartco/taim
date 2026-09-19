use super::cache;
use crate::core::svg::Handle;
use crate::core::{Color, Rectangle, Size};

use std::cell::RefCell;

const EXACT_ICON_EDGE: u32 = 128;
const MAX_RASTER_EDGE: u32 = 8192;
const MAX_RASTER_PIXELS: u64 = 8 * 1024 * 1024;

#[derive(Debug)]
pub struct Pipeline {
    cache: RefCell<cache::Cache>,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            cache: RefCell::new(cache::Cache::new()),
        }
    }

    pub fn viewport_dimensions(&self, handle: &Handle) -> Size<u32> {
        self.cache
            .borrow_mut()
            .viewport_dimensions(handle)
            .unwrap_or(Size::new(0, 0))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        handle: &Handle,
        color: Option<Color>,
        bounds: Rectangle,
        opacity: f32,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        transform: tiny_skia::Transform,
        clip_mask: Option<&tiny_skia::Mask>,
    ) {
        if opacity <= 0.0 || !opacity.is_finite() {
            return;
        }
        let Some(target) = raster_size(bounds, transform) else {
            return;
        };

        let mut cache = self.cache.borrow_mut();
        let Some(image) = cache.raster(handle, color, target) else {
            return;
        };
        let scale_x = bounds.width / image.width() as f32;
        let scale_y = bounds.height / image.height() as f32;
        if !scale_x.is_finite() || !scale_y.is_finite() || scale_x <= 0.0 || scale_y <= 0.0 {
            return;
        }

        pixels.draw_pixmap(
            (bounds.x / scale_x).round() as i32,
            (bounds.y / scale_y).round() as i32,
            image,
            &tiny_skia::PixmapPaint {
                opacity: opacity.clamp(0.0, 1.0),
                quality: tiny_skia::FilterQuality::Bilinear,
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

fn raster_size(bounds: Rectangle, transform: tiny_skia::Transform) -> Option<Size<u32>> {
    if bounds.width <= 0.0
        || bounds.height <= 0.0
        || !bounds.width.is_finite()
        || !bounds.height.is_finite()
    {
        return None;
    }

    let (scale_x, scale_y) = transform.get_scale();
    let width = quantize((bounds.width * scale_x.abs()).ceil())?;
    let height = quantize((bounds.height * scale_y.abs()).ceil())?;
    Some(fit_budget(width, height))
}

fn quantize(value: f32) -> Option<u32> {
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    let value = value.min(MAX_RASTER_EDGE as f32) as u32;
    let step = if value <= EXACT_ICON_EDGE {
        1
    } else if value <= 1024 {
        4
    } else {
        8
    };
    Some(value.max(1).div_ceil(step) * step)
}

fn fit_budget(width: u32, height: u32) -> Size<u32> {
    let pixels = u64::from(width) * u64::from(height);
    if pixels <= MAX_RASTER_PIXELS {
        return Size::new(width, height);
    }

    let ratio = (MAX_RASTER_PIXELS as f64 / pixels as f64).sqrt();
    Size::new(
        ((f64::from(width) * ratio).floor() as u32).max(1),
        ((f64::from(height) * ratio).floor() as u32).max(1),
    )
}

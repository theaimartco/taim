use crate::core::{Color, Size, renderer};
use crate::graphics::{Viewport, compositor};
use crate::{Renderer, window};

impl compositor::Default for Renderer {
    type Compositor = window::Compositor;
}

impl crate::graphics::text::CacheStatistics for Renderer {
    fn text_cache_statistics(&self) -> Option<crate::graphics::text::cache::Statistics> {
        Some(self.engine.text_cache_statistics())
    }
}

impl renderer::Headless for Renderer {
    async fn new(settings: renderer::Settings, backend: Option<&str>) -> Option<Self> {
        if backend.is_some_and(|backend| !["tiny-skia", "tiny_skia", "software"].contains(&backend))
        {
            return None;
        }

        Some(Self::new(settings))
    }

    fn name(&self) -> String {
        "tiny-skia".to_owned()
    }

    fn screenshot(
        &mut self,
        size: Size<u32>,
        scale_factor: f32,
        background_color: Color,
    ) -> Vec<u8> {
        let viewport = Viewport::with_physical_size(size, scale_factor);
        window::compositor::screenshot(self, &viewport, background_color)
    }
}

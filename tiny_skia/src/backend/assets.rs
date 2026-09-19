use crate::Renderer;
#[cfg(any(feature = "image", feature = "svg"))]
use crate::core::{self, Rectangle};
use crate::graphics;

impl graphics::mesh::Renderer for Renderer {
    fn draw_mesh(&mut self, _mesh: graphics::Mesh) {
        log::warn!("taim_tiny_skia does not support drawing meshes");
    }

    fn draw_mesh_cache(&mut self, _cache: graphics::mesh::Cache) {
        log::warn!("taim_tiny_skia does not support drawing meshes");
    }
}

#[cfg(feature = "image")]
impl core::image::Renderer for Renderer {
    type Handle = core::image::Handle;

    fn load_image(
        &self,
        handle: &Self::Handle,
    ) -> Result<core::image::Allocation, core::image::Error> {
        self.engine.raster_pipeline.load(handle)
    }

    fn measure_image(&self, handle: &Self::Handle) -> Option<crate::core::Size<u32>> {
        self.engine.raster_pipeline.dimensions(handle)
    }

    fn draw_image(&mut self, mut image: core::Image, bounds: Rectangle, clip_bounds: Rectangle) {
        image.opacity *= self.layers.opacity();
        if image.opacity <= 0.0 {
            return;
        }

        let (layer, transformation) = self.layers.current_mut();
        layer.draw_raster(image, bounds, clip_bounds, transformation);
    }
}

#[cfg(feature = "svg")]
impl core::svg::Renderer for Renderer {
    fn measure_svg(&self, handle: &core::svg::Handle) -> crate::core::Size<u32> {
        self.engine.vector_pipeline.viewport_dimensions(handle)
    }

    fn draw_svg(&mut self, mut svg: core::Svg, bounds: Rectangle, clip_bounds: Rectangle) {
        svg.opacity *= self.layers.opacity();
        if svg.opacity <= 0.0 {
            return;
        }

        let (layer, transformation) = self.layers.current_mut();
        layer.draw_svg(svg, bounds, clip_bounds, transformation);
    }
}

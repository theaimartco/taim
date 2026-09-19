use crate::core::Rectangle;
use crate::graphics;
use crate::{Geometry, Renderer, geometry};

impl graphics::geometry::Renderer for Renderer {
    type Geometry = Geometry;
    type Frame = geometry::Frame;

    fn new_frame(&self, bounds: Rectangle) -> Self::Frame {
        geometry::Frame::new(bounds)
    }

    fn draw_geometry(&mut self, geometry: Self::Geometry) {
        let opacity = self.layers.opacity();
        if opacity <= 0.0 {
            return;
        }

        let (layer, transformation) = self.layers.current_mut();

        match geometry {
            Geometry::Live {
                primitives,
                images,
                text,
                clip_bounds,
            } => {
                let primitives = if opacity == 1.0 {
                    primitives
                } else {
                    primitives
                        .into_iter()
                        .map(|primitive| primitive.scale_alpha(opacity))
                        .collect()
                };
                layer.draw_primitive_group(primitives, clip_bounds, transformation);

                let images = if opacity == 1.0 {
                    images
                } else {
                    images
                        .into_iter()
                        .map(|image| image.scale_alpha(opacity))
                        .collect()
                };
                layer.draw_image_group(images, clip_bounds, transformation);

                let text = if opacity == 1.0 {
                    text
                } else {
                    text.into_iter()
                        .map(|text| text.scale_alpha(opacity))
                        .collect()
                };
                layer.draw_text_group(text, clip_bounds, transformation);
            }
            Geometry::Cache(cache) => {
                if opacity == 1.0 {
                    layer.draw_primitive_cache(
                        std::sync::Arc::clone(&cache.primitives),
                        cache.clip_bounds,
                        transformation,
                    );
                } else {
                    let primitives = cache
                        .primitives
                        .iter()
                        .cloned()
                        .map(|primitive| primitive.scale_alpha(opacity))
                        .collect();
                    layer.draw_primitive_group(primitives, cache.clip_bounds, transformation);
                }

                if opacity == 1.0 {
                    layer.draw_image_cache(
                        std::sync::Arc::clone(&cache.images),
                        cache.clip_bounds,
                        transformation,
                    );
                } else {
                    let images = cache
                        .images
                        .iter()
                        .cloned()
                        .map(|image| image.scale_alpha(opacity))
                        .collect();
                    layer.draw_image_group(images, cache.clip_bounds, transformation);
                }

                if opacity == 1.0 {
                    layer.draw_text_cache(
                        std::sync::Arc::clone(&cache.text),
                        cache.clip_bounds,
                        transformation,
                    );
                } else {
                    let text = cache
                        .text
                        .iter()
                        .cloned()
                        .map(|text| text.scale_alpha(opacity))
                        .collect();
                    layer.draw_text_group(text, cache.clip_bounds, transformation);
                }
            }
        }
    }
}

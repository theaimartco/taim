use super::Engine;
#[cfg(any(feature = "image", feature = "svg"))]
use super::path;
use crate::core::{Rectangle, Transformation};
use crate::graphics::Image;

impl Engine {
    #[allow(clippy::too_many_arguments)]
    #[cfg_attr(not(any(feature = "image", feature = "svg")), allow(unused_variables))]
    pub fn draw_image(
        &mut self,
        image: &Image,
        transformation: Transformation,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: &mut tiny_skia::Mask,
        outer_clip: Rectangle,
    ) {
        match image {
            #[cfg(feature = "image")]
            Image::Raster {
                image,
                bounds,
                clip_bounds,
            } => {
                if image.opacity <= 0.0 {
                    return;
                }
                let Some(clip) = outer_clip.intersection(&(*clip_bounds * transformation)) else {
                    return;
                };
                let visible = bounds.rotate(image.rotation) * transformation;
                if !clip.intersects(&visible) {
                    return;
                }
                let mask = self.image_mask(clip_mask, clip, visible);
                let center = bounds.center();
                let transform = path::transform(transformation).pre_rotate_at(
                    f32::from(image.rotation).to_degrees(),
                    center.x,
                    center.y,
                );
                self.raster_pipeline.draw(
                    &image.handle,
                    image.filter_method,
                    *bounds,
                    image.opacity,
                    pixels,
                    transform,
                    mask,
                );
            }
            #[cfg(feature = "svg")]
            Image::Vector {
                svg,
                bounds,
                clip_bounds,
            } => {
                if svg.opacity <= 0.0 {
                    return;
                }
                let Some(clip) = outer_clip.intersection(&(*clip_bounds * transformation)) else {
                    return;
                };
                let visible = bounds.rotate(svg.rotation) * transformation;
                if !clip.intersects(&visible) {
                    return;
                }
                let mask = self.image_mask(clip_mask, clip, visible);
                let center = bounds.center();
                let transform = path::transform(transformation).pre_rotate_at(
                    f32::from(svg.rotation).to_degrees(),
                    center.x,
                    center.y,
                );
                self.vector_pipeline.draw(
                    &svg.handle,
                    svg.color,
                    *bounds,
                    svg.opacity,
                    pixels,
                    transform,
                    mask,
                );
            }
            #[cfg(not(feature = "image"))]
            Image::Raster { .. } => {
                log::warn!("TAIM software renderer received raster data without image support");
            }
            #[cfg(not(feature = "svg"))]
            Image::Vector { .. } => {
                log::warn!("TAIM software renderer received vector data without SVG support");
            }
        }
    }

    #[cfg(any(feature = "image", feature = "svg"))]
    fn image_mask<'a>(
        &mut self,
        mask: &'a mut tiny_skia::Mask,
        clip: Rectangle,
        visible: Rectangle,
    ) -> Option<&'a tiny_skia::Mask> {
        if visible.is_within(&clip) {
            None
        } else {
            self.adjust_clip_mask(mask, clip);
            Some(mask)
        }
    }
}

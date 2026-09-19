use super::{Engine, path};
use crate::Primitive;
use crate::core::{Rectangle, Transformation};

impl Engine {
    #[allow(clippy::too_many_arguments)]
    pub fn draw_primitive(
        &mut self,
        primitive: &Primitive,
        transformation: Transformation,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: &mut tiny_skia::Mask,
        clip_bounds: Rectangle,
    ) {
        match primitive {
            Primitive::Fill {
                path: shape,
                paint,
                rule,
                bounds,
            } => {
                let visible = *bounds * transformation;
                if !clip_bounds.intersects(&visible) {
                    return;
                }
                pixels.fill_path(
                    shape,
                    paint,
                    *rule,
                    path::transform(transformation),
                    (!visible.is_within(&clip_bounds)).then_some(clip_mask as &_),
                );
            }
            Primitive::Stroke {
                path: shape,
                paint,
                stroke,
                bounds,
            } => {
                let visible = *bounds * transformation;
                if !clip_bounds.intersects(&visible) {
                    return;
                }
                pixels.stroke_path(
                    shape,
                    paint,
                    stroke,
                    path::transform(transformation),
                    (!visible.is_within(&clip_bounds)).then_some(clip_mask as &_),
                );
            }
        }
    }
}

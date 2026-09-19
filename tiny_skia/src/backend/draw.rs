use crate::core::{Color, Rectangle, Transformation};
use crate::graphics::Viewport;
use crate::{Renderer, engine};
use taim_debug as debug;

impl Renderer {
    pub fn draw(
        &mut self,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: &mut tiny_skia::Mask,
        viewport: &Viewport,
        damage: &[Rectangle],
        background_color: Color,
    ) {
        let scale_factor = viewport.scale_factor();
        let scale_transform = Transformation::scale(scale_factor);
        let background = tiny_skia::Paint {
            shader: tiny_skia::Shader::SolidColor(engine::into_color(background_color)),
            anti_alias: false,
            blend_mode: tiny_skia::BlendMode::Source,
            ..Default::default()
        };
        self.layers.flush();
        self.engine.begin_frame();

        for &damage_bounds in damage {
            let damage_bounds = damage_bounds * scale_factor;

            if !damage_bounds.x.is_finite()
                || !damage_bounds.y.is_finite()
                || !damage_bounds.width.is_finite()
                || !damage_bounds.height.is_finite()
                || damage_bounds.width <= 0.0
                || damage_bounds.height <= 0.0
            {
                continue;
            }

            let Some(rect) = tiny_skia::Rect::from_xywh(
                damage_bounds.x,
                damage_bounds.y,
                damage_bounds.width,
                damage_bounds.height,
            ) else {
                continue;
            };
            let path = tiny_skia::PathBuilder::from_rect(rect);

            pixels.fill_path(
                &path,
                &background,
                tiny_skia::FillRule::default(),
                tiny_skia::Transform::identity(),
                None,
            );

            for layer in self.layers.iter() {
                let Some(layer_bounds) = damage_bounds.intersection(&(layer.bounds * scale_factor))
                else {
                    continue;
                };

                self.engine.adjust_clip_mask(clip_mask, layer_bounds);

                if !layer.quads.is_empty() {
                    let render_span = debug::render(debug::Primitive::Quad);
                    for quad in &layer.quads {
                        self.engine.draw_quad(
                            &quad.primitive,
                            &quad.background,
                            scale_transform * quad.transformation,
                            pixels,
                            clip_mask,
                            layer_bounds,
                        );
                    }
                    render_span.finish();
                }

                if !layer.primitives.is_empty() {
                    let render_span = debug::render(debug::Primitive::Triangle);

                    for group in &layer.primitives {
                        let Some(group_bounds) =
                            (group.clip_bounds() * scale_factor).intersection(&layer_bounds)
                        else {
                            continue;
                        };

                        self.engine.adjust_clip_mask(clip_mask, group_bounds);

                        for primitive in group.as_slice() {
                            self.engine.draw_primitive(
                                primitive,
                                scale_transform * group.transformation(),
                                pixels,
                                clip_mask,
                                group_bounds,
                            );
                        }

                        self.engine.adjust_clip_mask(clip_mask, layer_bounds);
                    }

                    render_span.finish();
                }

                if !layer.images.is_empty() {
                    let render_span = debug::render(debug::Primitive::Image);

                    for group in &layer.images {
                        let Some(group_bounds) =
                            (group.clip_bounds() * scale_factor).intersection(&layer_bounds)
                        else {
                            continue;
                        };

                        for image in group.as_slice() {
                            self.engine.draw_image(
                                image,
                                scale_transform * group.transformation(),
                                pixels,
                                clip_mask,
                                group_bounds,
                            );
                        }
                    }

                    render_span.finish();
                }

                if !layer.text.is_empty() {
                    let render_span = debug::render(debug::Primitive::Text);

                    for group in &layer.text {
                        let Some(group_bounds) =
                            (group.clip_bounds() * scale_factor).intersection(&layer_bounds)
                        else {
                            continue;
                        };

                        for text in group.as_slice() {
                            self.engine.draw_text(
                                text,
                                scale_transform * group.transformation(),
                                pixels,
                                clip_mask,
                                group_bounds,
                            );
                        }
                    }

                    render_span.finish();
                }
            }
        }

        self.engine.trim();
    }
}

use super::Engine;
use crate::core::{Rectangle, Size, Transformation};
use crate::graphics::Text;

impl Engine {
    #[allow(clippy::too_many_arguments)]
    pub fn draw_text(
        &mut self,
        text: &Text,
        transformation: Transformation,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: &mut tiny_skia::Mask,
        outer_clip: Rectangle,
    ) {
        match text {
            Text::Paragraph {
                paragraph,
                position,
                color,
                clip_bounds,
                transformation: local,
            } => {
                if color.a <= 0.0 {
                    return;
                }
                let transformation = transformation * *local;
                let Some(clip) = outer_clip.intersection(&(*clip_bounds * transformation)) else {
                    return;
                };
                let visible = Rectangle::new(*position, paragraph.min_bounds) * transformation;
                if !clip.intersects(&visible) {
                    return;
                }
                let mask = self.mask_for(clip_mask, clip, visible);
                self.text_pipeline.draw_paragraph(
                    paragraph,
                    *position,
                    *color,
                    pixels,
                    mask,
                    transformation,
                );
            }
            Text::Editor {
                editor,
                position,
                color,
                clip_bounds,
                transformation: local,
            } => {
                if color.a <= 0.0 {
                    return;
                }
                let transformation = transformation * *local;
                let Some(clip) = outer_clip.intersection(&(*clip_bounds * transformation)) else {
                    return;
                };
                let visible = Rectangle::new(*position, editor.bounds) * transformation;
                if !clip.intersects(&visible) {
                    return;
                }
                let mask = self.mask_for(clip_mask, clip, visible);
                self.text_pipeline.draw_editor(
                    editor,
                    *position,
                    *color,
                    pixels,
                    mask,
                    transformation,
                );
            }
            Text::Cached {
                content,
                bounds,
                color,
                size,
                line_height,
                font,
                align_x,
                align_y,
                shaping,
                wrapping,
                ellipsis,
                clip_bounds,
            } => {
                if color.a <= 0.0 {
                    return;
                }
                let Some(clip) = outer_clip.intersection(&(*clip_bounds * transformation)) else {
                    return;
                };
                let visible = *bounds * transformation;
                if !clip.intersects(&visible) {
                    return;
                }
                let mask = self.mask_for(clip_mask, clip, visible);
                self.text_pipeline.draw_cached(
                    content,
                    *bounds,
                    *color,
                    *size,
                    *line_height,
                    *font,
                    *align_x,
                    *align_y,
                    *shaping,
                    *wrapping,
                    *ellipsis,
                    pixels,
                    mask,
                    transformation,
                );
            }
            Text::Raw {
                raw,
                transformation: local,
            } => {
                if raw.color.a <= 0.0 {
                    return;
                }
                let Some(buffer) = raw.buffer.upgrade() else {
                    return;
                };
                let transformation = transformation * *local;
                let Some(clip) = outer_clip.intersection(&(raw.clip_bounds * transformation))
                else {
                    return;
                };
                let (width, height) = buffer.size();
                let visible = Rectangle::new(
                    raw.position,
                    Size::new(
                        width.unwrap_or(raw.clip_bounds.width),
                        height.unwrap_or(raw.clip_bounds.height),
                    ),
                ) * transformation;
                if !clip.intersects(&visible) {
                    return;
                }
                let mask = self.mask_for(clip_mask, clip, visible);
                self.text_pipeline.draw_raw(
                    &buffer,
                    raw.position,
                    raw.color,
                    pixels,
                    mask,
                    transformation,
                );
            }
        }
    }

    fn mask_for<'a>(
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

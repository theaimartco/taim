use super::glyph_cache;
use crate::core::{Color, Point, Transformation};

#[allow(clippy::too_many_arguments)]
pub(super) fn buffer(
    font_system: &mut cosmic_text::FontSystem,
    glyphs: &mut glyph_cache::Cache,
    swash: &mut cosmic_text::SwashCache,
    buffer: &cosmic_text::Buffer,
    position: Point,
    color: Color,
    pixels: &mut tiny_skia::PixmapMut<'_>,
    clip_mask: Option<&tiny_skia::Mask>,
    transformation: Transformation,
) {
    let position = position * transformation;
    let scale = transformation.scale_factor();
    if !scale.is_finite() || scale <= 0.0 {
        return;
    }

    for run in buffer.layout_runs() {
        let baseline = (run.line_y * scale).round() as i32;

        for glyph in run.glyphs {
            let physical = glyph.physical((position.x, position.y), scale);
            let glyph_color = glyph.color_opt.map(from_color).unwrap_or(color);
            let Some((buffer, placement)) =
                glyphs.allocate(physical.cache_key, glyph_color, font_system, swash)
            else {
                continue;
            };
            let Some(pixmap) =
                tiny_skia::PixmapRef::from_bytes(buffer, placement.width, placement.height)
            else {
                log::warn!("TAIM software text rejected malformed glyph pixels");
                continue;
            };
            let explicit_alpha = glyph
                .color_opt
                .map(|color| f32::from(color.a()) / 255.0)
                .unwrap_or(1.0);

            pixels.draw_pixmap(
                physical.x.saturating_add(placement.left),
                physical
                    .y
                    .saturating_sub(placement.top)
                    .saturating_add(baseline),
                pixmap,
                &tiny_skia::PixmapPaint {
                    opacity: color.a * explicit_alpha,
                    ..Default::default()
                },
                tiny_skia::Transform::identity(),
                clip_mask,
            );
        }
    }
}

fn from_color(color: cosmic_text::Color) -> Color {
    let [red, green, blue, alpha] = color.as_rgba();
    Color::from_rgba8(red, green, blue, f32::from(alpha) / 255.0)
}

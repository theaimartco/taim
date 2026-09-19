use super::Text;
use crate::core;
use crate::core::alignment;
use crate::core::text::{Alignment, Paragraph as _};
use crate::core::{Color, Point, Size, Vector};
use crate::geometry::Path;
use crate::text;

pub(super) fn draw(source: &Text, mut emit: impl FnMut(Path, Color)) {
    if source.content.is_empty()
        || source.size.0 <= 0.0
        || !source.size.0.is_finite()
        || source.color.a <= 0.0
        || !source.color.a.is_finite()
    {
        return;
    }

    let max_width = if source.max_width.is_nan() {
        f32::INFINITY
    } else {
        source.max_width.max(0.0)
    };
    let paragraph = text::Paragraph::with_text(core::text::Text {
        content: &source.content,
        bounds: Size::new(max_width, f32::INFINITY),
        size: source.size,
        line_height: source.line_height,
        font: source.font,
        align_x: source.align_x,
        align_y: source.align_y,
        shaping: source.shaping,
        wrapping: source.wrapping,
        ellipsis: source.ellipsis,
        hint_factor: None,
    });

    let origin_x = match source.align_x {
        Alignment::Default | Alignment::Left | Alignment::Justified => source.position.x,
        Alignment::Center => source.position.x - paragraph.min_width() * 0.5,
        Alignment::Right => source.position.x - paragraph.min_width(),
    };
    let origin_y = match source.align_y {
        alignment::Vertical::Top => source.position.y,
        alignment::Vertical::Center => source.position.y - paragraph.min_height() * 0.5,
        alignment::Vertical::Bottom => source.position.y - paragraph.min_height(),
    };

    let mut swash = cosmic_text::SwashCache::new();
    let mut fonts = text::font_system()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    for run in paragraph.buffer().layout_runs() {
        for glyph in run.glyphs {
            let physical = glyph.physical((0.0, 0.0), 1.0);
            let offset = Vector::new(
                origin_x + glyph.x + glyph.x_offset,
                origin_y + glyph.y_offset + run.line_y,
            );

            if let Some(commands) = swash.get_outline_commands(fonts.raw(), physical.cache_key) {
                let path = Path::new(|builder| append_outline(builder, commands, offset));
                emit(path, source.color);
                continue;
            }

            let [red, green, blue, alpha] = source.color.into_rgba8();
            swash.with_pixels(
                fonts.raw(),
                physical.cache_key,
                cosmic_text::Color::rgba(red, green, blue, alpha),
                |x, y, color| {
                    emit(
                        Path::rectangle(
                            Point::new(x as f32, y as f32) + offset,
                            Size::new(1.0, 1.0),
                        ),
                        Color::from_rgba8(
                            color.r(),
                            color.g(),
                            color.b(),
                            f32::from(color.a()) / 255.0,
                        ),
                    );
                },
            );
        }
    }
}

fn append_outline(
    path: &mut crate::geometry::path::Builder,
    commands: &[cosmic_text::Command],
    offset: Vector,
) {
    for command in commands {
        match command {
            cosmic_text::Command::MoveTo(point) => {
                path.move_to(Point::new(point.x, -point.y) + offset);
            }
            cosmic_text::Command::LineTo(point) => {
                path.line_to(Point::new(point.x, -point.y) + offset);
            }
            cosmic_text::Command::CurveTo(a, b, to) => path.bezier_curve_to(
                Point::new(a.x, -a.y) + offset,
                Point::new(b.x, -b.y) + offset,
                Point::new(to.x, -to.y) + offset,
            ),
            cosmic_text::Command::QuadTo(control, to) => path.quadratic_curve_to(
                Point::new(control.x, -control.y) + offset,
                Point::new(to.x, -to.y) + offset,
            ),
            cosmic_text::Command::Close => path.close(),
        }
    }
}

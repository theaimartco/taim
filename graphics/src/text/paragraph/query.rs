//! Read-only paragraph hit testing and geometry extraction.

use super::Internal;
use crate::core::{Point, Rectangle, Size};

use unicode_segmentation::UnicodeSegmentation;

pub(super) fn hit_span(paragraph: &Internal, point: Point) -> Option<usize> {
    let scale = paragraph.hint_factor;
    let cursor = paragraph.buffer.hit(point.x * scale, point.y * scale)?;
    let line = paragraph.buffer.lines.get(cursor.line)?;

    if cursor.index >= line.text().len() {
        return None;
    }

    let byte = match cursor.affinity {
        cosmic_text::Affinity::Before => cursor.index.saturating_sub(1),
        cosmic_text::Affinity::After => cursor.index,
    };

    line.layout_opt()?
        .iter()
        .flat_map(|visual| &visual.glyphs)
        .find(|glyph| glyph.start <= byte && byte < glyph.end)
        .map(|glyph| glyph.metadata)
}

pub(super) fn span_bounds(paragraph: &Internal, span: usize) -> Vec<Rectangle> {
    let scale = paragraph.hint_factor.recip();
    let mut output = Vec::with_capacity(1);

    for run in paragraph.buffer.layout_runs() {
        let mut line_bounds: Option<Rectangle> = None;

        for glyph in run.glyphs.iter().filter(|glyph| glyph.metadata == span) {
            let glyph_bounds = Rectangle::new(
                Point::new(glyph.x * scale, (run.line_top + glyph.y) * scale),
                Size::new(
                    glyph.w * scale,
                    glyph.line_height_opt.unwrap_or(run.line_height) * scale,
                ),
            );

            line_bounds = Some(match line_bounds {
                Some(bounds) => bounds.union(&glyph_bounds),
                None => glyph_bounds,
            });
        }

        output.extend(line_bounds);
    }

    output
}

pub(super) fn grapheme_position(paragraph: &Internal, line: usize, index: usize) -> Option<Point> {
    let run = paragraph.buffer.layout_runs().nth(line)?;
    let mut previous_start = None;
    let mut cluster_len = 0;
    let mut graphemes_seen = 0;

    let glyph = run
        .glyphs
        .iter()
        .find(|glyph| {
            if previous_start != Some(glyph.start) {
                cluster_len = run.text[glyph.start..glyph.end].graphemes(false).count();
                previous_start = Some(glyph.start);
                graphemes_seen += cluster_len;
            }

            graphemes_seen >= index
        })
        .or_else(|| run.glyphs.last())?;

    let advance = if index == 0 {
        0.0
    } else {
        let remaining = graphemes_seen.saturating_sub(index) as f32;
        glyph.w * (1.0 - remaining / cluster_len.max(1) as f32)
    };
    let scale = paragraph.hint_factor.recip();

    Some(Point::new(
        (glyph.x + glyph.x_offset * glyph.font_size + advance) * scale,
        (glyph.y - glyph.y_offset * glyph.font_size) * scale,
    ))
}

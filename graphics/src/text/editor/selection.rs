//! Cached caret and selection geometry for the text editor.

use super::{Internal, buffer_from_editor};
use crate::core::text::editor::Selection;
use crate::core::{Point, Rectangle};

use cosmic_text::Edit as _;

use std::sync::Arc;

pub(super) fn resolve(state: &Internal) -> Selection {
    if let Some(cached) = state
        .selection
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
    {
        return cached.clone();
    }

    let buffer = buffer_from_editor(&state.editor);
    let geometry = match state.editor.selection_bounds() {
        Some((start, end)) => range(state, buffer, start, end),
        None => caret(state, buffer),
    };

    *state
        .selection
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(geometry.clone());
    geometry
}

fn range(
    state: &Internal,
    buffer: &cosmic_text::Buffer,
    start: cosmic_text::Cursor,
    end: cosmic_text::Cursor,
) -> Selection {
    let line_height = buffer.metrics().line_height;
    let selected_lines = end.line.saturating_sub(start.line) + 1;
    let y_offset = visual_lines_offset(start.line, buffer);
    let scale = state.hint_factor.recip();

    let regions: Arc<[Rectangle]> = buffer
        .lines
        .iter()
        .skip(start.line)
        .take(selected_lines)
        .enumerate()
        .flat_map(|(line_index, line)| {
            selected_segments(
                line,
                if line_index == 0 { start.index } else { 0 },
                if line_index + 1 == selected_lines {
                    end.index
                } else {
                    line.text().len()
                },
            )
        })
        .enumerate()
        .filter_map(|(visual_line, (x, width))| {
            (width > 0.0).then(|| {
                Rectangle {
                    x,
                    width,
                    y: (visual_line as i32 + y_offset) as f32 * line_height
                        - buffer.scroll().vertical,
                    height: line_height,
                } * scale
            })
        })
        .collect::<Vec<_>>()
        .into();

    Selection::Range(regions)
}

fn caret(state: &Internal, buffer: &cosmic_text::Buffer) -> Selection {
    let cursor = state.editor.cursor();
    let y_offset = visual_lines_offset(cursor.line, buffer);
    let layout = buffer
        .lines
        .get(cursor.line)
        .and_then(cosmic_text::BufferLine::layout_opt);

    let Some(layout) = layout else {
        return Selection::Caret(Point::ORIGIN);
    };

    let position = layout.iter().enumerate().find_map(|(line_index, line)| {
        let start = line.glyphs.first().map_or(0, |glyph| glyph.start);
        let end = line.glyphs.last().map_or(0, |glyph| glyph.end);
        let before_end = match cursor.affinity {
            cosmic_text::Affinity::Before => cursor.index <= end,
            cosmic_text::Affinity::After => cursor.index < end,
        };

        if start > cursor.index {
            let previous = line_index.saturating_sub(1);
            Some((previous, layout.get(previous).map_or(0.0, |line| line.w)))
        } else if before_end {
            let x = line
                .glyphs
                .iter()
                .take_while(|glyph| cursor.index > glyph.start)
                .map(|glyph| glyph.w)
                .sum();
            Some((line_index, x))
        } else {
            None
        }
    });

    let (visual_line, x) = position.unwrap_or_else(|| {
        (
            layout.len().saturating_sub(1),
            layout.last().map_or(0.0, |line| line.w),
        )
    });
    let scale = state.hint_factor.recip();

    Selection::Caret(Point::new(
        x * scale,
        ((y_offset + visual_line as i32) as f32 * buffer.metrics().line_height
            - buffer.scroll().vertical)
            * scale,
    ))
}

fn selected_segments(
    line: &cosmic_text::BufferLine,
    from: usize,
    to: usize,
) -> impl Iterator<Item = (f32, f32)> + '_ {
    line.layout_opt()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(move |visual| {
            let start = visual.glyphs.first().map_or(0, |glyph| glyph.start);
            let end = visual.glyphs.last().map_or(0, |glyph| glyph.end);
            let selected = start.max(from)..end.min(to);

            if selected.is_empty() {
                return (0.0, 0.0);
            }
            if selected.start == start && selected.end == end {
                return (0.0, visual.w);
            }

            let first = visual
                .glyphs
                .iter()
                .position(|glyph| selected.start <= glyph.start)
                .unwrap_or(0);
            let mut glyphs = visual.glyphs.iter();
            let x = glyphs.by_ref().take(first).map(|glyph| glyph.w).sum();
            let width = glyphs
                .take_while(|glyph| selected.end > glyph.start)
                .map(|glyph| glyph.w)
                .sum();
            (x, width)
        })
}

fn visual_lines_offset(line: usize, buffer: &cosmic_text::Buffer) -> i32 {
    let scroll = buffer.scroll();
    let start = scroll.line.min(line);
    let end = scroll.line.max(line);
    let count: usize = buffer.lines[start..]
        .iter()
        .take(end - start)
        .map(|line| line.layout_opt().map_or(0, Vec::len))
        .sum();

    count as i32 * if scroll.line < line { 1 } else { -1 }
}

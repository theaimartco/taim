//! Paragraph measurement and alignment.

use super::conversion;
use crate::core::Size;
use crate::core::text::Alignment;

/// Measures shaped visual runs and reports whether any run is right-to-left.
pub fn measure(buffer: &cosmic_text::Buffer) -> (Size, bool) {
    buffer
        .layout_runs()
        .fold((Size::ZERO, false), |(mut size, has_rtl), run| {
            if run.line_w.is_finite() {
                size.width = size.width.max(run.line_w.max(0.0));
            }
            if run.line_height.is_finite() {
                size.height += run.line_height.max(0.0);
            }
            (size, has_rtl || run.rtl)
        })
}

/// Applies horizontal alignment and returns the minimum logical bounds.
pub fn align(
    buffer: &mut cosmic_text::Buffer,
    fonts: &mut cosmic_text::FontSystem,
    alignment: Alignment,
) -> Size {
    let (minimum, has_rtl) = measure(buffer);
    let mut changed = false;

    if let Some(target) = conversion::alignment(alignment) {
        let multiple_lines = buffer.lines.len() > 1
            || buffer
                .lines
                .first()
                .and_then(cosmic_text::BufferLine::layout_opt)
                .is_some_and(|layout| layout.len() > 1);

        if multiple_lines {
            for line in &mut buffer.lines {
                changed |= line.set_align(Some(target));
            }
        } else if let Some(line) = buffer.lines.first_mut() {
            changed = line.set_align(None);
        }
    }

    if changed || has_rtl {
        buffer.set_size(Some(minimum.width), Some(minimum.height));
        buffer.shape_until_scroll(fonts, false);
    }

    minimum
}

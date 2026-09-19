//! Incremental editor configuration and shaping.

use super::{Editor, buffer_from_editor, buffer_mut_from_editor};
use crate::core::text::highlighter::Highlighter;
use crate::core::text::{LineHeight, Wrapping};
use crate::core::{Font, Pixels, Size};
use crate::text;

use cosmic_text::Edit as _;

pub(super) fn apply(
    editor: &mut Editor,
    bounds: Size,
    font: Font,
    size: Pixels,
    line_height: LineHeight,
    wrapping: Wrapping,
    hint_factor: Option<f32>,
    highlighter: &mut impl Highlighter,
) {
    let hint = text::hint_factor(size, hint_factor);
    let scale = hint.unwrap_or(1.0);
    let line_height = line_height.to_absolute(size);
    let wrapping = text::to_wrap(wrapping);

    {
        let state = editor.internal();
        let buffer = buffer_from_editor(&state.editor);
        let metrics = buffer.metrics();

        if state.topmost_line_changed.is_none()
            && state.version == text::font_version()
            && state.font == font
            && state.bounds == bounds
            && state.hint.then_some(state.hint_factor) == hint
            && metrics.font_size == size.0 * scale
            && metrics.line_height == line_height.0 * scale
            && buffer.wrap() == wrapping
        {
            return;
        }
    }

    editor.with_internal_mut(|state| {
        let mut fonts = text::font_system()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let buffer = buffer_mut_from_editor(&mut state.editor);

        if fonts.version() != state.version {
            for line in &mut buffer.lines {
                line.reset();
            }
            state.version = fonts.version();
            state.topmost_line_changed = Some(0);
        }

        if state.font != font {
            let attributes = cosmic_text::AttrsList::new(&text::to_attributes(font));
            for line in &mut buffer.lines {
                let _ = line.set_attrs_list(attributes.clone());
            }
            state.font = font;
            state.topmost_line_changed = Some(0);
        }

        let old_metrics = buffer.metrics();
        let hint_changed = hint != state.hint.then_some(state.hint_factor);

        if hint_changed {
            state.hint = hint.is_some();
            state.hint_factor = scale;
            buffer.set_hinting(if state.hint {
                cosmic_text::Hinting::Enabled
            } else {
                cosmic_text::Hinting::Disabled
            });
        }

        let font_size = size.0 * state.hint_factor;
        let line_height = line_height.0 * state.hint_factor;
        if old_metrics.font_size != font_size
            || old_metrics.line_height != line_height
            || hint_changed
        {
            buffer.set_metrics(cosmic_text::Metrics::new(font_size, line_height));
        }

        if buffer.wrap() != wrapping {
            buffer.set_wrap(wrapping);
        }

        if state.bounds != bounds || hint_changed {
            buffer.set_size(
                Some(bounds.width * state.hint_factor),
                Some(bounds.height * state.hint_factor),
            );
            state.bounds = bounds;
        }

        if let Some(first_changed_line) = state.topmost_line_changed.take() {
            highlighter.change_line(first_changed_line);
        }

        state.editor.shape_as_needed(fonts.raw(), false);
    });
}

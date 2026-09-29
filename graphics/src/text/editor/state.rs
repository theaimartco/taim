//! Editor storage shared by mutation and rendering views.

use super::{Editor, buffer_from_editor};
use crate::core::text::editor::Selection;
use crate::core::{Font, Size};
use crate::text;

use std::fmt;
use std::sync::{self, RwLock};

pub(super) struct Internal {
    pub(super) editor: cosmic_text::Editor<'static>,
    pub(super) selection: RwLock<Option<Selection>>,
    pub(super) font: Font,
    pub(super) bounds: Size,
    pub(super) topmost_line_changed: Option<usize>,
    pub(super) hint: bool,
    pub(super) hint_factor: f32,
    pub(super) version: text::Version,
}

impl Default for Internal {
    fn default() -> Self {
        Self {
            editor: cosmic_text::Editor::new(cosmic_text::Buffer::new_empty(
                cosmic_text::Metrics::new(1.0, 1.0),
            )),
            selection: RwLock::new(None),
            font: Font::default(),
            bounds: Size::ZERO,
            topmost_line_changed: None,
            hint: false,
            hint_factor: 1.0,
            version: text::Version::default(),
        }
    }
}

impl Clone for Internal {
    fn clone(&self) -> Self {
        Self {
            editor: self.editor.clone(),
            selection: RwLock::new(None),
            font: self.font,
            bounds: self.bounds,
            topmost_line_changed: self.topmost_line_changed,
            hint: self.hint,
            hint_factor: self.hint_factor,
            version: self.version,
        }
    }
}

impl PartialEq for Internal {
    fn eq(&self, other: &Self) -> bool {
        let left = buffer_from_editor(&self.editor);
        let right = buffer_from_editor(&other.editor);

        self.version == other.version
            && self.font == other.font
            && self.bounds == other.bounds
            && self.hint == other.hint
            && self.hint_factor == other.hint_factor
            && left.metrics() == right.metrics()
            && left.wrap() == right.wrap()
            && left.lines.len() == right.lines.len()
            && left
                .lines
                .iter()
                .zip(&right.lines)
                .all(|(a, b)| a.text() == b.text() && a.ending() == b.ending())
    }
}

impl fmt::Debug for Internal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EditorState")
            .field("font", &self.font)
            .field("bounds", &self.bounds)
            .field("font_generation", &self.version)
            .finish_non_exhaustive()
    }
}

/// A snapshot of editor geometry retained until the renderer consumes it.
#[derive(Debug, Clone)]
pub struct Weak {
    pub(super) raw: sync::Arc<Internal>,
    /// Logical bounds captured without upgrading the editor.
    pub bounds: Size,
}

impl Weak {
    /// Returns the retained editor snapshot.
    pub fn upgrade(&self) -> Option<Editor> {
        Some(Editor(Some(self.raw.clone())))
    }
}

impl PartialEq for Weak {
    fn eq(&self, other: &Self) -> bool {
        if sync::Arc::ptr_eq(&self.raw, &other.raw) {
            return true;
        }

        self.raw.as_ref() == other.raw.as_ref()
    }
}

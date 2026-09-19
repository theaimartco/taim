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

/// A non-owning editor reference used by retained rendering trees.
#[derive(Debug, Clone)]
pub struct Weak {
    pub(super) raw: sync::Weak<Internal>,
    /// Logical bounds captured without upgrading the editor.
    pub bounds: Size,
}

impl Weak {
    /// Upgrades the reference while the editor is still alive.
    pub fn upgrade(&self) -> Option<Editor> {
        self.raw.upgrade().map(Some).map(Editor)
    }
}

impl PartialEq for Weak {
    fn eq(&self, other: &Self) -> bool {
        if sync::Weak::ptr_eq(&self.raw, &other.raw) {
            return self.raw.strong_count() > 0;
        }

        match (self.upgrade(), other.upgrade()) {
            (Some(left), Some(right)) => left == right,
            _ => false,
        }
    }
}

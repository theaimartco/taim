//! Shared paragraph state and cheap rendering-tree references.

use super::Paragraph;
use crate::core::alignment;
use crate::core::text::{Alignment, Ellipsis, Shaping, Wrapping};
use crate::core::{Font, Size};
use crate::text;

use std::sync;

#[derive(Clone)]
pub(super) struct Internal {
    pub(super) buffer: cosmic_text::Buffer,
    pub(super) font: Font,
    pub(super) shaping: Shaping,
    pub(super) wrapping: Wrapping,
    pub(super) ellipsis: Ellipsis,
    pub(super) align_x: Alignment,
    pub(super) align_y: alignment::Vertical,
    pub(super) bounds: Size,
    pub(super) min_bounds: Size,
    pub(super) version: text::Version,
    pub(super) hint: bool,
    pub(super) hint_factor: f32,
}

impl Default for Internal {
    fn default() -> Self {
        Self {
            buffer: cosmic_text::Buffer::new_empty(cosmic_text::Metrics::new(1.0, 1.0)),
            font: Font::default(),
            shaping: Shaping::default(),
            wrapping: Wrapping::default(),
            ellipsis: Ellipsis::default(),
            align_x: Alignment::Default,
            align_y: alignment::Vertical::Top,
            bounds: Size::ZERO,
            min_bounds: Size::ZERO,
            version: text::Version::default(),
            hint: false,
            hint_factor: 1.0,
        }
    }
}

impl PartialEq for Internal {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version
            && self.font == other.font
            && self.shaping == other.shaping
            && self.wrapping == other.wrapping
            && self.ellipsis == other.ellipsis
            && self.align_x == other.align_x
            && self.align_y == other.align_y
            && self.bounds == other.bounds
            && self.min_bounds == other.min_bounds
            && self.hint == other.hint
            && self.hint_factor == other.hint_factor
            && self.buffer.metrics() == other.buffer.metrics()
            && self.buffer.lines.len() == other.buffer.lines.len()
            && self
                .buffer
                .lines
                .iter()
                .zip(&other.buffer.lines)
                .all(|(left, right)| left.text() == right.text() && left.ending() == right.ending())
    }
}

/// A non-owning paragraph reference used by retained rendering trees.
#[derive(Debug, Clone)]
pub struct Weak {
    pub(super) raw: sync::Weak<Internal>,
    /// Minimum logical bounds captured without upgrading the weak reference.
    pub min_bounds: Size,
    /// Horizontal alignment captured for positioning.
    pub align_x: Alignment,
    /// Vertical alignment captured for positioning.
    pub align_y: alignment::Vertical,
}

impl Weak {
    /// Upgrades this reference while the shaped paragraph is still alive.
    pub fn upgrade(&self) -> Option<Paragraph> {
        self.raw.upgrade().map(Paragraph)
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

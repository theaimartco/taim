//! Retained text primitives shared by rendering backends.

use super::{editor, paragraph};
use crate::core::alignment;
use crate::core::text::{Alignment, Ellipsis, Shaping, Wrapping};
use crate::core::{Color, Font, Pixels, Point, Rectangle, Transformation};

use std::sync::Weak;

/// A backend-neutral text primitive.
#[derive(Debug, Clone, PartialEq)]
pub enum Text {
    /// A shaped paragraph.
    Paragraph {
        /// Weak paragraph storage reference.
        paragraph: paragraph::Weak,
        /// Logical origin.
        position: Point,
        /// Text color.
        color: Color,
        /// Logical clip rectangle.
        clip_bounds: Rectangle,
        /// Full affine display transform.
        transformation: Transformation,
    },
    /// An editable text buffer.
    Editor {
        /// Weak editor storage reference.
        editor: editor::Weak,
        /// Logical origin.
        position: Point,
        /// Text color.
        color: Color,
        /// Logical clip rectangle.
        clip_bounds: Rectangle,
        /// Full affine display transform.
        transformation: Transformation,
    },
    /// Owned text awaiting cache lookup or shaping.
    Cached {
        /// UTF-8 content.
        content: String,
        /// Transformed bounds.
        bounds: Rectangle,
        /// Text color.
        color: Color,
        /// Logical font size.
        size: Pixels,
        /// Absolute line height.
        line_height: Pixels,
        /// Font descriptor.
        font: Font,
        /// Horizontal alignment.
        align_x: Alignment,
        /// Vertical alignment.
        align_y: alignment::Vertical,
        /// Shaping policy.
        shaping: Shaping,
        /// Wrapping policy.
        wrapping: Wrapping,
        /// Ellipsis policy.
        ellipsis: Ellipsis,
        /// Transformed clip rectangle.
        clip_bounds: Rectangle,
    },
    /// An externally retained shaping buffer.
    Raw {
        /// Raw buffer and paint data.
        raw: Raw,
        /// Full affine display transform.
        transformation: Transformation,
    },
}

impl Text {
    /// Multiplies the primitive opacity.
    pub fn scale_alpha(mut self, factor: f32) -> Self {
        match &mut self {
            Self::Paragraph { color, .. }
            | Self::Editor { color, .. }
            | Self::Cached { color, .. } => *color = color.scale_alpha(factor),
            Self::Raw { raw, .. } => raw.color = raw.color.scale_alpha(factor),
        }
        self
    }

    /// Returns the clipped display bounds when they are known cheaply.
    pub fn visible_bounds(&self) -> Option<Rectangle> {
        match self {
            Self::Paragraph {
                paragraph,
                position,
                clip_bounds,
                transformation,
                ..
            } => Rectangle::new(*position, paragraph.min_bounds)
                .intersection(clip_bounds)
                .map(|bounds| bounds * *transformation),
            Self::Editor {
                editor,
                position,
                clip_bounds,
                transformation,
                ..
            } => Rectangle::new(*position, editor.bounds)
                .intersection(clip_bounds)
                .map(|bounds| bounds * *transformation),
            Self::Cached {
                bounds,
                clip_bounds,
                ..
            } => bounds.intersection(clip_bounds),
            Self::Raw { raw, .. } => Some(raw.clip_bounds),
        }
    }
}

/// Weak externally managed shaping buffer and its paint data.
#[derive(Debug, Clone)]
pub struct Raw {
    /// Shaping buffer reference.
    pub buffer: Weak<cosmic_text::Buffer>,
    /// Logical origin.
    pub position: Point,
    /// Text color.
    pub color: Color,
    /// Logical clip rectangle.
    pub clip_bounds: Rectangle,
}

impl PartialEq for Raw {
    fn eq(&self, _other: &Self) -> bool {
        // The external buffer can mutate without changing its allocation.
        // Conservatively force retained-layer damage until it carries a
        // generation supplied by its owner.
        false
    }
}

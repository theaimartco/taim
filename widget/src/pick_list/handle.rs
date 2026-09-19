//! Pick-list indicator icons.

use crate::core::Pixels;
use crate::core::text;

/// The indicator displayed on the right side of a pick list.
#[derive(Debug, Clone, PartialEq)]
pub enum Handle<Font> {
    /// Built-in downward arrow.
    Arrow {
        /// Optional arrow size.
        size: Option<Pixels>,
    },
    /// One icon for both states.
    Static(Icon<Font>),
    /// Separate closed and open icons.
    Dynamic {
        /// Closed-state icon.
        closed: Icon<Font>,
        /// Open-state icon.
        open: Icon<Font>,
    },
    /// No indicator.
    None,
}

impl<Font> Default for Handle<Font> {
    fn default() -> Self {
        Self::Arrow { size: None }
    }
}

/// A font-backed indicator icon.
#[derive(Debug, Clone, PartialEq)]
pub struct Icon<Font> {
    /// Icon font.
    pub font: Font,
    /// Unicode glyph.
    pub code_point: char,
    /// Optional glyph size.
    pub size: Option<Pixels>,
    /// Glyph line height.
    pub line_height: text::LineHeight,
    /// Glyph shaping strategy.
    pub shaping: text::Shaping,
}

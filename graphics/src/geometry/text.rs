use crate::core::alignment;
use crate::core::text::{Alignment, Ellipsis, LineHeight, Shaping, Wrapping};
use crate::core::{Color, Font, Pixels, Point};
use crate::geometry::Path;

mod render;

/// A bunch of text that can be drawn to a canvas
#[derive(Debug, Clone)]
pub struct Text {
    /// The contents of the text
    pub content: String,
    /// The position of the text relative to the alignment properties.
    ///
    /// By default, this position will be relative to the top-left corner coordinate meaning that
    /// if the horizontal and vertical alignments are unchanged, this property will tell where the
    /// top-left corner of the text should be placed.
    ///
    /// By changing the horizontal_alignment and vertical_alignment properties, you are are able to
    /// change what part of text is placed at this positions.
    ///
    /// For example, when the horizontal_alignment and vertical_alignment are set to Center, the
    /// center of the text will be placed at the given position NOT the top-left coordinate.
    pub position: Point,
    /// The maximum horizontal space available for this [`Text`].
    ///
    /// Text will break into new lines when the width is reached.
    pub max_width: f32,
    /// The color of the text
    pub color: Color,
    /// The size of the text
    pub size: Pixels,
    /// The line height of the text.
    pub line_height: LineHeight,
    /// The font of the text
    pub font: Font,
    /// The horizontal alignment of the text
    pub align_x: Alignment,
    /// The vertical alignment of the text
    pub align_y: alignment::Vertical,
    /// The shaping strategy of the text.
    pub shaping: Shaping,
    /// The wrapping strategy of the text.
    pub wrapping: Wrapping,
    /// The ellipsis strategy of the text.
    pub ellipsis: Ellipsis,
}

impl Text {
    /// Computes the [`Path`]s of the [`Text`] and draws them using
    /// the given closure.
    pub fn draw_with(&self, f: impl FnMut(Path, Color)) {
        render::draw(self, f);
    }
}

impl Default for Text {
    fn default() -> Text {
        Text {
            content: String::new(),
            position: Point::ORIGIN,
            max_width: f32::INFINITY,
            color: Color::BLACK,
            size: Pixels(16.0),
            line_height: LineHeight::Relative(1.2),
            font: Font::default(),
            align_x: Alignment::Default,
            align_y: alignment::Vertical::Top,
            shaping: Shaping::default(),
            wrapping: Wrapping::default(),
            ellipsis: Ellipsis::default(),
        }
    }
}

impl From<String> for Text {
    fn from(content: String) -> Text {
        Text {
            content,
            ..Default::default()
        }
    }
}

impl From<&str> for Text {
    fn from(content: &str) -> Text {
        String::from(content).into()
    }
}

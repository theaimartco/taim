use crate::alignment;
use crate::text::{self, Text};
use crate::{Color, Font, Pixels, Point, Rectangle, Size};

impl text::Renderer for () {
    type Font = Font;
    type Paragraph = ();
    type Editor = ();

    const ICON_FONT: Font = Font::DEFAULT;
    const CHECKMARK_ICON: char = '0';
    const ARROW_DOWN_ICON: char = '0';
    const SCROLL_UP_ICON: char = '0';
    const SCROLL_DOWN_ICON: char = '0';
    const SCROLL_LEFT_ICON: char = '0';
    const SCROLL_RIGHT_ICON: char = '0';
    const TAIM_LOGO: char = '0';

    fn default_font(&self) -> Font {
        Font::DEFAULT
    }

    fn default_size(&self) -> Pixels {
        Pixels(16.0)
    }

    fn fill_paragraph(&mut self, _: &(), _: Point, _: Color, _: Rectangle) {}
    fn fill_editor(&mut self, _: &(), _: Point, _: Color, _: Rectangle) {}
    fn fill_text(&mut self, _: Text, _: Point, _: Color, _: Rectangle) {}
}

impl text::Paragraph for () {
    type Font = Font;

    fn with_text(_: Text<&str>) -> Self {}
    fn with_spans<Link>(_: Text<&[text::Span<'_, Link, Font>], Font>) -> Self {}
    fn resize(&mut self, _: Size) {}

    fn compare(&self, _: Text<()>) -> text::Difference {
        text::Difference::None
    }

    fn hint_factor(&self) -> Option<f32> {
        None
    }

    fn size(&self) -> Pixels {
        Pixels(16.0)
    }

    fn font(&self) -> Font {
        Font::DEFAULT
    }

    fn line_height(&self) -> text::LineHeight {
        text::LineHeight::default()
    }

    fn align_x(&self) -> text::Alignment {
        text::Alignment::Default
    }

    fn align_y(&self) -> alignment::Vertical {
        alignment::Vertical::Top
    }

    fn wrapping(&self) -> text::Wrapping {
        text::Wrapping::default()
    }

    fn ellipsis(&self) -> text::Ellipsis {
        text::Ellipsis::default()
    }

    fn shaping(&self) -> text::Shaping {
        text::Shaping::default()
    }

    fn grapheme_position(&self, _: usize, _: usize) -> Option<Point> {
        None
    }

    fn bounds(&self) -> Size {
        Size::ZERO
    }

    fn min_bounds(&self) -> Size {
        Size::ZERO
    }

    fn hit_test(&self, _: Point) -> Option<text::Hit> {
        None
    }

    fn hit_span(&self, _: Point) -> Option<usize> {
        None
    }

    fn span_bounds(&self, _: usize) -> Vec<Rectangle> {
        Vec::new()
    }
}

impl text::Editor for () {
    type Font = Font;

    fn with_text(_: &str) -> Self {}

    fn is_empty(&self) -> bool {
        true
    }

    fn cursor(&self) -> text::editor::Cursor {
        text::editor::Cursor::new(text::editor::Position::new(0, 0))
    }

    fn selection(&self) -> text::editor::Selection {
        text::editor::Selection::Caret(Point::ORIGIN)
    }

    fn copy(&self) -> Option<String> {
        None
    }

    fn line(&self, _: usize) -> Option<text::editor::Line<'_>> {
        None
    }

    fn line_count(&self) -> usize {
        0
    }

    fn perform(&mut self, _: text::editor::Action) {}
    fn move_to(&mut self, _: text::editor::Cursor) {}

    fn bounds(&self) -> Size {
        Size::ZERO
    }

    fn hint_factor(&self) -> Option<f32> {
        None
    }

    fn min_bounds(&self) -> Size {
        Size::ZERO
    }

    fn update(
        &mut self,
        _: Size,
        _: Font,
        _: Pixels,
        _: text::LineHeight,
        _: text::Wrapping,
        _: Option<f32>,
        _: &mut impl text::Highlighter,
    ) {
    }

    fn highlight<H: text::Highlighter>(
        &mut self,
        _: Font,
        _: &mut H,
        _: impl Fn(&H::Highlight) -> text::highlighter::Format<Font>,
    ) {
    }
}

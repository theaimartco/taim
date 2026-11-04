//! Editor gutter and scrollbar tied to the text editor's real viewport.

use taim::{Color, Point, Rectangle, Size};
use taim_core::text::Alignment;
use taim_widget::Canvas;
use taim_widget::canvas::{self, Program};

use crate::Message;

/// Shared top inset for gutter rows and the text editor's content bounds.
pub const EDITOR_CONTENT_INSET: f32 = 8.0;

#[derive(Clone, Copy)]
pub struct EditorViewport {
    pub doc_index: usize,
    pub first_line: usize,
    pub vertical_offset: f32,
    pub line_count: usize,
    pub cursor_line: usize,
    pub extra_cursor_lines: [usize; 4],
    pub extra_cursor_count: u8,
    pub font_size: f32,
    pub light: bool,
    pub foldable_lines: [usize; 16],
    pub foldable_count: u8,
    pub folded_lines: [usize; 16],
    pub folded_count: u8,
}

impl EditorViewport {
    pub fn has_extra_cursor(self, line: usize) -> bool {
        (0..self.extra_cursor_count as usize).any(|i| self.extra_cursor_lines[i] == line)
    }

    pub fn is_foldable(self, line: usize) -> bool {
        (0..self.foldable_count as usize).any(|i| self.foldable_lines[i] == line)
    }

    pub fn is_folded(self, line: usize) -> bool {
        (0..self.folded_count as usize).any(|i| self.folded_lines[i] == line)
    }

    fn line_height(self) -> f32 {
        self.font_size * 1.2
    }

    fn visible_lines(self, height: f32) -> usize {
        (height / self.line_height()).ceil().max(1.0) as usize
    }

    fn max_first(self, height: f32) -> usize {
        self.line_count.saturating_sub(self.visible_lines(height))
    }

    fn gray(self, dark: u8, light: u8) -> Color {
        let value = if self.light { light } else { dark };
        Color::from_rgb8(value, value, value)
    }
}

pub fn gutter(viewport: EditorViewport) -> Canvas<Gutter, Message> {
    Canvas::new(Gutter(viewport))
        .width(taim_core::Length::Fixed(48.0))
        .height(taim_core::Length::Fill)
}

pub fn scrollbar(viewport: EditorViewport) -> Canvas<Scrollbar, Message> {
    Canvas::new(Scrollbar(viewport))
        .width(taim_core::Length::Fixed(16.0))
        .height(taim_core::Length::Fill)
}

pub fn minimap(
    viewport: EditorViewport,
    density: std::sync::Arc<[u8]>,
) -> Canvas<Minimap, Message> {
    Canvas::new(Minimap { viewport, density })
        .width(taim_core::Length::Fixed(52.0))
        .height(taim_core::Length::Fill)
}

fn line_string(line_1_based: usize) -> String {
    static NUMBERS: std::sync::LazyLock<Vec<String>> =
        std::sync::LazyLock::new(|| (0..=2048).map(|n| n.to_string()).collect());
    if line_1_based <= 2048 {
        NUMBERS[line_1_based].clone()
    } else {
        line_1_based.to_string()
    }
}

pub struct Gutter(EditorViewport);

impl Program<Message> for Gutter {
    type State = ();

    fn update(
        &self,
        _state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: taim::mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if matches!(
            event,
            canvas::Event::Mouse(taim::mouse::Event::ButtonPressed(taim::mouse::Button::Left))
        ) {
            let position = cursor.position_over(bounds)?;
            let line = self.0.first_line
                + ((position.y - bounds.y - EDITOR_CONTENT_INSET + self.0.vertical_offset)
                    / self.0.line_height())
                .floor()
                .max(0.0) as usize;
            if line < self.0.line_count {
                if position.x < bounds.x + 14.0 && self.0.is_foldable(line) {
                    return Some(
                        canvas::Action::publish(Message::ToggleFold(self.0.doc_index, line))
                            .and_capture(),
                    );
                }
                return Some(
                    canvas::Action::publish(Message::EditorGoToLine(self.0.doc_index, line))
                        .and_capture(),
                );
            }
        }
        None
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &taim::Renderer,
        _theme: &taim::Theme,
        bounds: Rectangle,
        _cursor: taim::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), self.0.gray(22, 239));
        frame.fill_rectangle(
            Point::new(bounds.width - 1.0, 0.0),
            Size::new(1.0, bounds.height),
            self.0.gray(43, 204),
        );
        let line_height = self.0.line_height();
        let count = self.0.visible_lines(bounds.height).saturating_add(2);
        for row in 0..count {
            let line = self.0.first_line + row;
            if line >= self.0.line_count {
                break;
            }
            let y = EDITOR_CONTENT_INSET + row as f32 * line_height - self.0.vertical_offset;
            if y + line_height <= 0.0 || y >= bounds.height {
                continue;
            }
            let is_extra_cursor = self.0.has_extra_cursor(line);
            if is_extra_cursor {
                frame.fill_rectangle(
                    Point::new(1.0, y + 2.0),
                    Size::new(2.5, (line_height - 4.0).max(4.0)),
                    Color::from_rgb(0.35, 0.65, 0.95),
                );
            }
            let is_foldable = self.0.is_foldable(line);
            let is_folded = self.0.is_folded(line);
            if is_foldable {
                let marker = if is_folded { "▸" } else { "▾" };
                frame.fill_text(canvas::Text {
                    content: marker.to_string(),
                    position: Point::new(3.0, y),
                    max_width: 12.0,
                    color: if is_folded {
                        Color::from_rgb(0.95, 0.75, 0.25)
                    } else {
                        self.0.gray(150, 110)
                    },
                    size: taim::Pixels(self.0.font_size * 0.9),
                    font: taim::Font::MONOSPACE,
                    align_x: Alignment::Left,
                    ..canvas::Text::default()
                });
            }
            frame.fill_text(canvas::Text {
                content: line_string(line + 1),
                position: Point::new(bounds.width - 9.0, y),
                max_width: bounds.width - 9.0,
                color: if line == self.0.cursor_line {
                    self.0.gray(210, 36)
                } else if is_extra_cursor {
                    Color::from_rgb(0.35, 0.65, 0.95)
                } else {
                    self.0.gray(118, 113)
                },
                size: taim::Pixels(self.0.font_size),
                font: taim::Font::MONOSPACE,
                align_x: Alignment::Right,
                ..canvas::Text::default()
            });
        }
        vec![frame.into_geometry()]
    }
}

#[derive(Default)]
pub struct ScrollbarState {
    dragging: bool,
}

pub struct Scrollbar(EditorViewport);

impl Scrollbar {
    fn thumb(&self, height: f32) -> (f32, f32) {
        if height <= 0.0 {
            return (0.0, 0.0);
        }
        let visible = self.0.visible_lines(height);
        if self.0.line_count <= visible {
            return (0.0, height);
        }
        let thumb_height = (height * visible as f32 / self.0.line_count as f32)
            .clamp(24.0_f32.min(height), height);
        let max_first = self.0.max_first(height).max(1);
        let y =
            (self.0.first_line.min(max_first) as f32 / max_first as f32) * (height - thumb_height);
        (y, thumb_height)
    }

    fn target_line(&self, y: f32, height: f32) -> usize {
        if height <= 0.0 {
            return 0;
        }
        let (_, thumb_height) = self.thumb(height);
        let travel = (height - thumb_height).max(1.0);
        (((y - thumb_height / 2.0) / travel).clamp(0.0, 1.0) * self.0.max_first(height) as f32)
            .round() as usize
    }
}

pub struct Minimap {
    viewport: EditorViewport,
    density: std::sync::Arc<[u8]>,
}

impl Program<Message> for Minimap {
    type State = ScrollbarState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: taim::mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            canvas::Event::Mouse(taim::mouse::Event::ButtonPressed(taim::mouse::Button::Left))
                if cursor.is_over(bounds) =>
            {
                state.dragging = true
            }
            canvas::Event::Mouse(taim::mouse::Event::ButtonReleased(taim::mouse::Button::Left)) => {
                state.dragging = false;
                return None;
            }
            canvas::Event::Mouse(taim::mouse::Event::CursorMoved { .. }) if state.dragging => {}
            _ => return None,
        }
        let position = cursor.position()?;
        if bounds.height <= 0.0 {
            return None;
        }
        let target = ((position.y - bounds.y) / bounds.height).clamp(0.0, 1.0)
            * self.viewport.max_first(bounds.height) as f32;
        Some(
            canvas::Action::publish(Message::EditorScrollToLine(
                self.viewport.doc_index,
                target.round() as usize,
            ))
            .and_capture(),
        )
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &taim::Renderer,
        _theme: &taim::Theme,
        bounds: Rectangle,
        _cursor: taim::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), self.viewport.gray(24, 239));
        let total = self.viewport.line_count.max(1);
        let visible = self.viewport.visible_lines(bounds.height);
        let first = self.viewport.first_line.min(total);
        let viewport_y = bounds.height * first as f32 / total as f32;
        let viewport_height = (bounds.height * visible as f32 / total as f32)
            .clamp(14.0_f32.min(bounds.height), bounds.height);
        frame.fill_rectangle(
            Point::new(0.0, viewport_y.min(bounds.height - viewport_height)),
            Size::new(bounds.width, viewport_height),
            self.viewport.gray(48, 210),
        );
        let count = self.density.len().max(1);
        for (index, density) in self.density.iter().copied().enumerate() {
            if density == 0 {
                continue;
            }
            let y = bounds.height * index as f32 / count as f32;
            frame.fill_rectangle(
                Point::new(4.0, y),
                Size::new((density as f32 * 0.55).clamp(2.0, bounds.width - 8.0), 1.0),
                self.viewport.gray(103, 145),
            );
        }
        vec![frame.into_geometry()]
    }
}

impl Program<Message> for Scrollbar {
    type State = ScrollbarState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: taim::mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            canvas::Event::Mouse(taim::mouse::Event::ButtonPressed(taim::mouse::Button::Left))
                if cursor.is_over(bounds) =>
            {
                state.dragging = true;
            }
            canvas::Event::Mouse(taim::mouse::Event::ButtonReleased(taim::mouse::Button::Left)) => {
                state.dragging = false;
                return None;
            }
            canvas::Event::Mouse(taim::mouse::Event::CursorMoved { .. }) if state.dragging => {}
            _ => return None,
        }
        let position = cursor.position()?;
        let target = self.target_line(position.y - bounds.y, bounds.height);
        Some(
            canvas::Action::publish(Message::EditorScrollToLine(self.0.doc_index, target))
                .and_capture(),
        )
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &taim::Renderer,
        _theme: &taim::Theme,
        bounds: Rectangle,
        _cursor: taim::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), self.0.gray(22, 239));
        let (y, thumb_height) = self.thumb(bounds.height);
        frame.fill_rectangle(
            Point::new(5.0, y),
            Size::new(6.0, thumb_height),
            self.0.gray(102, 140),
        );
        vec![frame.into_geometry()]
    }
}

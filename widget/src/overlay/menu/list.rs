use super::Catalog;

use crate::core::alignment;
use crate::core::border;
use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::renderer;
use crate::core::text::{self, Text};
use crate::core::touch;
use crate::core::widget::tree::{self, Tree};
use crate::core::window;
use crate::core::{Element, Event, Length, Padding, Pixels, Point, Rectangle, Shell, Size, Widget};

use std::cell::RefCell;

pub(super) struct List<'a, 'b, T, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    pub(super) options: &'a [T],
    pub(super) hovered_option: &'a mut Option<usize>,
    pub(super) to_string: &'a dyn Fn(&T) -> String,
    pub(super) on_selected: Box<dyn FnMut(T) -> Message + 'a>,
    pub(super) on_option_hovered: Option<&'a dyn Fn(T) -> Message>,
    pub(super) padding: Padding,
    pub(super) text_size: Option<Pixels>,
    pub(super) line_height: text::LineHeight,
    pub(super) shaping: text::Shaping,
    pub(super) ellipsis: text::Ellipsis,
    pub(super) font: Option<Renderer::Font>,
    pub(super) class: &'a <Theme as Catalog>::Class<'b>,
}

struct ListState<P: text::Paragraph> {
    is_hovered: Option<bool>,
    option_height: f32,
    generation: u64,
    paragraphs: RefCell<ParagraphWindow<P>>,
}

struct ParagraphWindow<P: text::Paragraph> {
    generation: u64,
    bounds_key: (u32, u32),
    start: usize,
    entries: Vec<text::paragraph::Plain<P>>,
}

impl<P: text::Paragraph> ParagraphWindow<P> {
    fn empty() -> Self {
        Self {
            generation: u64::MAX,
            bounds_key: (0, 0),
            start: 0,
            entries: Vec::new(),
        }
    }

    fn contains(&self, generation: u64, bounds_key: (u32, u32), start: usize, end: usize) -> bool {
        self.generation == generation
            && self.bounds_key == bounds_key
            && self.start <= start
            && end <= self.start.saturating_add(self.entries.len())
    }
}

impl<T, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for List<'_, '_, T, Message, Theme, Renderer>
where
    T: Clone,
    Theme: Catalog,
    Renderer: text::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<ListState<Renderer::Paragraph>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(ListState::<Renderer::Paragraph> {
            is_hovered: None,
            option_height: 0.0,
            generation: 0,
            paragraphs: RefCell::new(ParagraphWindow::empty()),
        })
    }

    fn diff(&mut self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<ListState<Renderer::Paragraph>>();
        state.generation = state.generation.wrapping_add(1);

        if state.generation == u64::MAX {
            state.paragraphs.get_mut().entries.clear();
        }

        tree.mark_dirty(tree::DirtyFlags::all());
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Shrink,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let text_size = self.text_size.unwrap_or_else(|| renderer.default_size());
        let option_height = option_height(self.line_height, text_size, self.padding);
        let intrinsic = Size::new(0.0, option_height * self.options.len() as f32);
        let size = limits.resolve(Length::Fill, Length::Shrink, intrinsic);

        tree.state
            .downcast_mut::<ListState<Renderer::Paragraph>>()
            .option_height = option_height;

        layout::Node::new(size)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<ListState<Renderer::Paragraph>>();
        let option_height = if state.option_height > 0.0 {
            state.option_height
        } else {
            option_height(
                self.line_height,
                self.text_size.unwrap_or_else(|| renderer.default_size()),
                self.padding,
            )
        };
        let bounds = layout.bounds();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if cursor.is_over(bounds)
                    && let Some(index) = *self.hovered_option
                    && let Some(option) = self.options.get(index)
                {
                    shell.publish((self.on_selected)(option.clone()));
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(cursor_position) = cursor.position_in(bounds) {
                    let index = (cursor_position.y / option_height) as usize;
                    let next = self.options.get(index).map(|_| index);

                    if *self.hovered_option != next {
                        if let Some(option) = next.and_then(|index| self.options.get(index))
                            && let Some(on_option_hovered) = self.on_option_hovered
                        {
                            shell.publish(on_option_hovered(option.clone()));
                        }

                        *self.hovered_option = next;
                        shell.request_redraw();
                    }
                } else if self.hovered_option.take().is_some() {
                    shell.request_redraw();
                }
            }
            Event::Touch(touch::Event::FingerPressed { .. }) => {
                if let Some(cursor_position) = cursor.position_in(bounds) {
                    let index = (cursor_position.y / option_height) as usize;
                    *self.hovered_option = self.options.get(index).map(|_| index);

                    if let Some(option) = self.options.get(index) {
                        shell.publish((self.on_selected)(option.clone()));
                        shell.capture_event();
                    }
                }
            }
            _ => {}
        }

        let is_hovered = cursor.is_over(bounds);

        if matches!(event, Event::Window(window::Event::RedrawRequested(_))) {
            state.is_hovered = Some(is_hovered);
        } else if state
            .is_hovered
            .is_some_and(|was_hovered| was_hovered != is_hovered)
        {
            shell.request_redraw();
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let style = Catalog::style(theme, self.class);
        let bounds = layout.bounds();
        let state = tree.state.downcast_ref::<ListState<Renderer::Paragraph>>();
        let option_height = state.option_height.max(f32::EPSILON);
        let offset = (viewport.y - bounds.y).max(0.0);
        let start = ((offset / option_height) as usize).min(self.options.len());
        let end = (((offset + viewport.height.max(0.0)) / option_height).ceil() as usize)
            .min(self.options.len());

        if start >= end {
            return;
        }

        let text_bounds = Size::new((bounds.width - self.padding.x()).max(0.0), option_height);
        let bounds_key = (text_bounds.width.to_bits(), text_bounds.height.to_bits());
        let mut paragraphs = state.paragraphs.borrow_mut();

        if !paragraphs.contains(state.generation, bounds_key, start, end) {
            let visible_count = end - start;
            let cache_start = start.saturating_sub(visible_count);
            let cache_end = end.saturating_add(visible_count).min(self.options.len());
            let font = self.font.unwrap_or_else(|| renderer.default_font());
            let text_size = self.text_size.unwrap_or_else(|| renderer.default_size());
            let hint_factor = renderer.scale_factor();

            paragraphs.entries.clear();
            paragraphs
                .entries
                .reserve(cache_end.saturating_sub(cache_start));

            for option in &self.options[cache_start..cache_end] {
                paragraphs.entries.push(text::paragraph::Plain::new(Text {
                    content: (self.to_string)(option),
                    bounds: text_bounds,
                    size: text_size,
                    line_height: self.line_height,
                    font,
                    align_x: text::Alignment::Default,
                    align_y: alignment::Vertical::Center,
                    shaping: self.shaping,
                    wrapping: text::Wrapping::None,
                    ellipsis: self.ellipsis,
                    hint_factor,
                }));
            }

            paragraphs.generation = state.generation;
            paragraphs.bounds_key = bounds_key;
            paragraphs.start = cache_start;
        }

        let local_start = start - paragraphs.start;
        let local_end = end - paragraphs.start;

        for (offset, paragraph) in paragraphs.entries[local_start..local_end]
            .iter()
            .enumerate()
        {
            let index = start + offset;
            let is_selected = *self.hovered_option == Some(index);
            let option_bounds = Rectangle {
                x: bounds.x,
                y: bounds.y + option_height * index as f32,
                width: bounds.width,
                height: option_height,
            };

            if is_selected {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: option_bounds.x + style.border.width,
                            width: (option_bounds.width - style.border.width * 2.0).max(0.0),
                            ..option_bounds
                        },
                        border: border::rounded(style.border.radius),
                        ..renderer::Quad::default()
                    },
                    style.selected_background,
                );
            }

            renderer.fill_paragraph(
                paragraph.raw(),
                Point::new(option_bounds.x + self.padding.left, option_bounds.y),
                if is_selected {
                    style.selected_text_color
                } else {
                    style.text_color
                },
                *viewport,
            );
        }
    }
}

impl<'a, 'b, T, Message, Theme, Renderer> From<List<'a, 'b, T, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    T: Clone,
    Message: 'a,
    Theme: 'a + Catalog,
    Renderer: 'a + text::Renderer,
    'b: 'a,
{
    fn from(list: List<'a, 'b, T, Message, Theme, Renderer>) -> Self {
        Element::new(list)
    }
}

#[inline]
fn option_height(line_height: text::LineHeight, text_size: Pixels, padding: Padding) -> f32 {
    (f32::from(line_height.to_absolute(text_size)) + padding.y()).max(f32::EPSILON)
}

use crate::core;
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::time::Instant;
use crate::core::widget;
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::visit;
use crate::core::window;
use crate::core::{Element, Event, Layout, Length, Rectangle, Shell, Size, Vector, Widget};

use super::Sensor;

#[derive(Debug, Clone)]
struct State<Key> {
    is_visible: bool,
    pending: Option<Pending>,
    last_size: Option<Size>,
    last_key: Key,
}

#[derive(Debug, Clone, Copy)]
struct Pending {
    visible: bool,
    at: Instant,
}

impl<Key, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Sensor<'_, Key, Message, Theme, Renderer>
where
    Key: super::Key,
    Renderer: core::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<Key::Owned>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State {
            is_visible: false,
            pending: None,
            last_size: None,
            last_key: self.key.to_owned(),
        })
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let state = tree.state.downcast_mut::<State<Key::Owned>>();

            if !self.key.eq(&state.last_key) {
                state.is_visible = false;
                state.pending = None;
                state.last_size = None;
                state.last_key = self.key.to_owned();
            }

            let bounds = layout.bounds();
            let visible = rectangle_distance(bounds, *viewport) <= self.anticipate.0;
            let size = bounds.size();

            match (state.is_visible, visible) {
                (false, true) => {
                    state.is_visible = true;
                    state.last_size = Some(size);
                    state.pending = self.on_show.as_ref().map(|_| Pending {
                        visible: true,
                        at: *now + self.delay,
                    });
                }
                (true, false) => {
                    state.is_visible = false;
                    state.pending = self.on_hide.as_ref().map(|_| Pending {
                        visible: false,
                        at: *now + self.delay,
                    });
                }
                (true, true) if state.last_size != Some(size) => {
                    state.last_size = Some(size);
                    if let Some(on_resize) = &self.on_resize {
                        shell.publish(on_resize(size));
                    }
                }
                _ => {}
            }

            match state.pending {
                Some(pending) if pending.at <= *now => {
                    if pending.visible {
                        if let Some(on_show) = &self.on_show {
                            shell.publish(on_show(size));
                        }
                    } else if let Some(on_hide) = self.on_hide.take() {
                        shell.publish(on_hide);
                    }
                    state.pending = None;
                }
                Some(pending) => shell.request_redraw_at(pending.at),
                None => {}
            }
        }

        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            shell,
            viewport,
        );
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn visit_children(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        visit(&mut visit::Widgets::new(
            self.content.as_widget_mut(),
            &mut tree.children[0],
            layout,
        ));
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

fn rectangle_distance(a: Rectangle, b: Rectangle) -> f32 {
    let a_right = a.x + a.width;
    let b_right = b.x + b.width;
    let a_bottom = a.y + a.height;
    let b_bottom = b.y + b.height;
    let dx = if a_right < b.x {
        b.x - a_right
    } else if b_right < a.x {
        a.x - b_right
    } else {
        0.0
    };
    let dy = if a_bottom < b.y {
        b.y - a_bottom
    } else if b_bottom < a.y {
        a.y - b_bottom
    } else {
        0.0
    };

    dx.hypot(dy)
}

impl<'a, Key, Message, Theme, Renderer> From<Sensor<'a, Key, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Key: super::Key + 'a,
    Renderer: core::Renderer + 'a,
    Theme: 'a,
{
    fn from(sensor: Sensor<'a, Key, Message, Theme, Renderer>) -> Self {
        Element::new(sensor)
    }
}

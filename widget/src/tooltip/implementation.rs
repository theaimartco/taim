use crate::container;
use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::text;
use crate::core::time::Instant;
use crate::core::widget::visit;
use crate::core::widget::{self, Widget};
use crate::core::window;
use crate::core::{Element, Event, Length, Padding, Point, Rectangle, Shell, Size, Vector};

use super::{Position, Tooltip};

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Tooltip<'_, Message, Theme, Renderer>
where
    Theme: container::Catalog,
    Renderer: text::Renderer,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::default())
    }

    fn diff(&mut self, tree: &mut widget::Tree) {
        tree.diff_children(&mut [self.content.as_widget_mut(), self.tooltip.as_widget_mut()]);
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if matches!(
            event,
            Event::Mouse(_) | Event::Window(window::Event::RedrawRequested(_))
        ) {
            let state = tree.state.downcast_mut::<State>();
            let cursor_position = cursor.position_over(layout.bounds());

            match (*state, cursor_position) {
                (State::Idle, Some(cursor_position)) if self.delay.is_zero() => {
                    *state = State::Open { cursor_position };
                    shell.invalidate_layout();
                }
                (State::Idle, Some(_)) => {
                    let opens_at = Instant::now() + self.delay;
                    *state = State::Hovered { opens_at };
                    shell.request_redraw_at(opens_at);
                }
                (State::Hovered { .. }, None) => *state = State::Idle,
                (State::Hovered { opens_at }, Some(_)) if Instant::now() < opens_at => {
                    shell.request_redraw_at(opens_at);
                }
                (State::Hovered { .. }, Some(cursor_position)) => {
                    *state = State::Open { cursor_position };
                    shell.invalidate_layout();
                }
                (
                    State::Open {
                        cursor_position: previous,
                    },
                    Some(cursor_position),
                ) if self.position == Position::FollowCursor && previous != cursor_position => {
                    *state = State::Open { cursor_position };
                    shell.request_redraw();
                }
                (State::Open { .. }, None) => {
                    *state = State::Idle;
                    shell.invalidate_layout();

                    if !matches!(event, Event::Window(window::Event::RedrawRequested(_))) {
                        shell.request_redraw();
                    }
                }
                (State::Open { .. }, Some(_)) | (State::Idle, None) => {}
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

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
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

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        inherited_style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            inherited_style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut widget::Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        let state = *tree.state.downcast_ref::<State>();
        let mut children = tree.children.iter_mut();
        let content = self.content.as_widget_mut().overlay(
            children.next().expect("Tooltip content state"),
            layout,
            renderer,
            viewport,
            translation,
        );
        let tooltip = match state {
            State::Open { cursor_position } => {
                Some(overlay::Element::new(Box::new(TooltipOverlay {
                    position: layout.position() + translation,
                    tooltip: &mut self.tooltip,
                    tree: children.next().expect("Tooltip overlay state"),
                    cursor_position,
                    content_bounds: layout.bounds(),
                    snap_within_viewport: self.snap_within_viewport,
                    positioning: self.position,
                    gap: self.gap,
                    padding: self.padding,
                    class: &self.class,
                })))
            }
            State::Idle | State::Hovered { .. } => None,
        };

        match (content, tooltip) {
            (Some(content), Some(tooltip)) => {
                Some(overlay::Group::with_children(vec![content, tooltip]).overlay())
            }
            (Some(content), None) => Some(content),
            (None, Some(tooltip)) => Some(tooltip),
            (None, None) => None,
        }
    }

    fn visit_children(
        &mut self,
        tree: &mut widget::Tree,
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
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                layout,
                renderer,
                operation,
            );
        });
    }
}

impl<'a, Message, Theme, Renderer> From<Tooltip<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: container::Catalog + 'a,
    Renderer: text::Renderer + 'a,
{
    fn from(tooltip: Tooltip<'a, Message, Theme, Renderer>) -> Self {
        Element::new(tooltip)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum State {
    #[default]
    Idle,
    Hovered {
        opens_at: Instant,
    },
    Open {
        cursor_position: Point,
    },
}

struct TooltipOverlay<'a, 'b, Message, Theme, Renderer>
where
    Theme: container::Catalog,
    Renderer: text::Renderer,
{
    position: Point,
    tooltip: &'b mut Element<'a, Message, Theme, Renderer>,
    tree: &'b mut widget::Tree,
    cursor_position: Point,
    content_bounds: Rectangle,
    snap_within_viewport: bool,
    positioning: Position,
    gap: f32,
    padding: f32,
    class: &'b Theme::Class<'a>,
}

impl<Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for TooltipOverlay<'_, '_, Message, Theme, Renderer>
where
    Theme: container::Catalog,
    Renderer: text::Renderer,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        let viewport = Rectangle::with_size(bounds);
        let tooltip_layout = self.tooltip.as_widget_mut().layout(
            self.tree,
            renderer,
            &layout::Limits::new(
                Size::ZERO,
                if self.snap_within_viewport {
                    viewport.size()
                } else {
                    Size::INFINITE
                },
            )
            .shrink(Padding::new(self.padding)),
        );
        let text_bounds = tooltip_layout.bounds();
        let center_x = self.position.x + (self.content_bounds.width - text_bounds.width) / 2.0;
        let center_y = self.position.y + (self.content_bounds.height - text_bounds.height) / 2.0;
        let offset = match self.positioning {
            Position::Top => Vector::new(
                center_x,
                self.position.y - text_bounds.height - self.gap - self.padding,
            ),
            Position::Bottom => Vector::new(
                center_x,
                self.position.y + self.content_bounds.height + self.gap + self.padding,
            ),
            Position::Left => Vector::new(
                self.position.x - text_bounds.width - self.gap - self.padding,
                center_y,
            ),
            Position::Right => Vector::new(
                self.position.x + self.content_bounds.width + self.gap + self.padding,
                center_y,
            ),
            Position::FollowCursor => {
                let translation = self.position - self.content_bounds.position();
                Vector::new(
                    self.cursor_position.x,
                    self.cursor_position.y - text_bounds.height,
                ) + translation
            }
        };
        let mut tooltip_bounds = Rectangle {
            x: offset.x - self.padding,
            y: offset.y - self.padding,
            width: text_bounds.width + self.padding * 2.0,
            height: text_bounds.height + self.padding * 2.0,
        };

        if self.snap_within_viewport {
            let max_x = (viewport.x + viewport.width - tooltip_bounds.width).max(viewport.x);
            let max_y = (viewport.y + viewport.height - tooltip_bounds.height).max(viewport.y);
            tooltip_bounds.x = tooltip_bounds.x.clamp(viewport.x, max_x);
            tooltip_bounds.y = tooltip_bounds.y.clamp(viewport.y, max_y);
        }

        layout::Node::with_children(
            tooltip_bounds.size(),
            vec![tooltip_layout.translate(Vector::new(self.padding, self.padding))],
        )
        .translate(Vector::new(tooltip_bounds.x, tooltip_bounds.y))
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        inherited_style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let style = theme.style(self.class);
        container::draw_background(renderer, &style, layout.bounds());
        let defaults = renderer::Style {
            text_color: style.text_color.unwrap_or(inherited_style.text_color),
        };

        self.tooltip.as_widget().draw(
            self.tree,
            renderer,
            theme,
            &defaults,
            layout.children().next().expect("Tooltip content layout"),
            cursor,
            &Rectangle::with_size(Size::INFINITE),
        );
    }
}

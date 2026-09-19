use super::Content;

use crate::container;
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::widget::visit;
use crate::core::widget::{self, Tree};
use crate::core::{self, Event, Layout, Point, Rectangle, Shell, Size, Vector};

impl<Message, Theme, Renderer> Content<'_, Message, Theme, Renderer>
where
    Theme: container::Catalog,
    Renderer: core::Renderer,
{
    pub(in crate::pane_grid) fn state(&self) -> Tree {
        let children = if let Some(title_bar) = self.title_bar.as_ref() {
            vec![Tree::new(&self.body), title_bar.state()]
        } else {
            vec![Tree::new(&self.body), Tree::empty()]
        };

        Tree::with_children(children)
    }

    pub(in crate::pane_grid) fn diff(&mut self, tree: &mut Tree) {
        if tree.children.len() != 2 {
            *tree = self.state();
            return;
        }

        tree.children[0].diff(&mut self.body);

        if let Some(title_bar) = &mut self.title_bar {
            title_bar.diff(&mut tree.children[1]);
        } else if !matches!(tree.children[1].state, widget::tree::State::None)
            || !tree.children[1].children.is_empty()
        {
            // Release state, text caches, and renderer handles owned by a title
            // bar as soon as it is removed from this pane.
            tree.children[1] = Tree::empty();
        }
    }

    /// Draws the [`Content`] with the provided [`Renderer`] and [`Layout`].
    ///
    /// [`Renderer`]: core::Renderer
    pub fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let pane_style = theme.style(&self.class);

        if bounds.width > 0.0 && bounds.height > 0.0 && bounds.intersects(viewport) {
            container::draw_background(renderer, &pane_style, bounds);
        }

        if let Some(title_bar) = &self.title_bar {
            let mut children = layout.children();
            let title_bar_layout = children.next().unwrap();
            let body_layout = children.next().unwrap();
            let show_controls = cursor.is_over(bounds);
            let body = self.body.as_widget();

            if body
                .paint_bounds(&tree.children[0], body_layout)
                .intersects(viewport)
            {
                body.draw(
                    &tree.children[0],
                    renderer,
                    theme,
                    style,
                    body_layout,
                    cursor,
                    viewport,
                );
            }

            title_bar.draw(
                &tree.children[1],
                renderer,
                theme,
                style,
                title_bar_layout,
                cursor,
                viewport,
                show_controls,
            );
        } else {
            let body = self.body.as_widget();

            if body
                .paint_bounds(&tree.children[0], layout)
                .intersects(viewport)
            {
                body.draw(
                    &tree.children[0],
                    renderer,
                    theme,
                    style,
                    layout,
                    cursor,
                    viewport,
                );
            }
        }
    }

    pub(crate) fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        if let Some(title_bar) = &mut self.title_bar {
            let max_size = limits.max();
            let title_limits = layout::Limits::new(Size::ZERO, max_size);
            let title_bar_layout = tree.children[1].layout_with_cache(title_limits, |state| {
                title_bar.layout(state, renderer, &title_limits)
            });
            let title_bar_size = title_bar_layout.size();
            let body_limits = layout::Limits::new(
                Size::ZERO,
                Size::new(
                    max_size.width.max(0.0),
                    (max_size.height - title_bar_size.height).max(0.0),
                ),
            );
            let body_layout = tree.children[0].layout_with_cache(body_limits, |state| {
                self.body
                    .as_widget_mut()
                    .layout(state, renderer, &body_limits)
            });

            layout::Node::with_children(
                max_size,
                vec![
                    title_bar_layout,
                    body_layout.move_to(Point::new(0.0, title_bar_size.height)),
                ],
            )
        } else {
            tree.children[0].layout_with_cache(*limits, |state| {
                self.body.as_widget_mut().layout(state, renderer, limits)
            })
        }
    }

    pub(crate) fn visit_children(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        let body_layout = if let Some(title_bar) = &mut self.title_bar {
            let mut children = layout.children();

            title_bar.visit_children(&mut tree.children[1], children.next().unwrap(), visit);
            children.next().unwrap()
        } else {
            layout
        };

        visit(&mut visit::Widgets::new(
            self.body.as_widget_mut(),
            &mut tree.children[0],
            body_layout,
        ));
    }

    pub(crate) fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        let body_layout = if let Some(title_bar) = &mut self.title_bar {
            let mut children = layout.children();

            title_bar.operate(
                &mut tree.children[1],
                children.next().unwrap(),
                renderer,
                operation,
            );
            children.next().unwrap()
        } else {
            layout
        };

        self.body
            .as_widget_mut()
            .operate(&mut tree.children[0], body_layout, renderer, operation);
    }

    pub(crate) fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
        is_picked: bool,
    ) {
        let body_layout = if let Some(title_bar) = &mut self.title_bar {
            let mut children = layout.children();

            title_bar.update(
                &mut tree.children[1],
                event,
                children.next().unwrap(),
                cursor,
                renderer,
                shell,
                viewport,
            );
            children.next().unwrap()
        } else {
            layout
        };

        if !is_picked {
            self.body.as_widget_mut().update(
                &mut tree.children[0],
                event,
                body_layout,
                cursor,
                renderer,
                shell,
                viewport,
            );
        }

        if shell.is_layout_invalid().is_some() {
            tree.children[0].mark_dirty(widget::tree::DirtyFlags::LAYOUT);
            tree.children[1].mark_dirty(widget::tree::DirtyFlags::LAYOUT);
        }
    }

    pub(crate) fn grid_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        drag_enabled: bool,
    ) -> Option<mouse::Interaction> {
        let title_bar = self.title_bar.as_ref()?;
        let title_bar_layout = layout.children().next()?;
        let cursor_position = cursor.position()?;

        (drag_enabled && title_bar.is_over_pick_area(title_bar_layout, cursor_position))
            .then_some(mouse::Interaction::Grab)
    }

    pub(crate) fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
        drag_enabled: bool,
    ) -> mouse::Interaction {
        let (body_layout, title_bar_interaction) = if let Some(title_bar) = &self.title_bar {
            let mut children = layout.children();
            let title_bar_layout = children.next().unwrap();

            if drag_enabled
                && cursor
                    .position()
                    .is_some_and(|position| title_bar.is_over_pick_area(title_bar_layout, position))
            {
                return mouse::Interaction::Grab;
            }

            let interaction = title_bar.mouse_interaction(
                &tree.children[1],
                title_bar_layout,
                cursor,
                viewport,
                renderer,
            );

            (children.next().unwrap(), interaction)
        } else {
            (layout, mouse::Interaction::default())
        };

        self.body
            .as_widget()
            .mouse_interaction(&tree.children[0], body_layout, cursor, viewport, renderer)
            .max(title_bar_interaction)
    }

    pub(crate) fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        if let Some(title_bar) = self.title_bar.as_mut() {
            let mut children = layout.children();
            let title_bar_layout = children.next()?;
            let mut states = tree.children.iter_mut();
            let body_state = states.next()?;
            let title_bar_state = states.next()?;

            title_bar
                .overlay(
                    title_bar_state,
                    title_bar_layout,
                    renderer,
                    viewport,
                    translation,
                )
                .or_else(|| {
                    self.body.as_widget_mut().overlay(
                        body_state,
                        children.next()?,
                        renderer,
                        viewport,
                        translation,
                    )
                })
        } else {
            self.body.as_widget_mut().overlay(
                &mut tree.children[0],
                layout,
                renderer,
                viewport,
                translation,
            )
        }
    }
}

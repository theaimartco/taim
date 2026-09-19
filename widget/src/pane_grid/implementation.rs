use super::interaction::{
    click_pane, edge_bounds, hovered_split, in_edge, layout_region, layout_region_bounds,
};
use super::*;

#[derive(Default)]
struct Memory {
    action: state::Action,
    order: Vec<Pane>,
    last_mouse_interaction: Option<mouse::Interaction>,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for PaneGrid<'_, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: core::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Memory>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Memory::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        let Memory { order, .. } = tree.state.downcast_ref();

        // `Pane` always increments and is iterated by Ord so new
        // states are always added at the end. We can simply remove
        // states which no longer exist and `diff_children` will
        // diff the remaining values in the correct order and
        // add new states at the end

        let mut i = 0;
        let mut j = 0;
        tree.children.retain(|_| {
            let retain = self.panes.get(i) == order.get(j);

            if retain {
                i += 1;
            }
            j += 1;

            retain
        });

        tree.diff_children_custom(
            &mut self.contents,
            |state, content| content.diff(state),
            Content::state,
        );

        let Memory { order, .. } = tree.state.downcast_mut();
        order.clone_from(&self.panes);
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let bounds = limits.resolve(self.width, self.height, Size::ZERO);
        let regions = self
            .internal
            .layout()
            .pane_regions(self.spacing, self.min_size, bounds);

        let children = self
            .panes
            .iter_mut()
            .zip(&mut self.contents)
            .zip(tree.children.iter_mut())
            .filter_map(|((pane, content), tree)| {
                if self
                    .internal
                    .maximized()
                    .is_some_and(|maximized| maximized != *pane)
                {
                    return Some(layout::Node::new(Size::ZERO));
                }

                let region = regions.get(pane)?;
                let size = Size::new(region.width, region.height);

                let node = content.layout(tree, renderer, &layout::Limits::new(size, size));

                Some(node.move_to(Point::new(region.x, region.y)))
            })
            .collect();

        layout::Node::with_children(bounds, children)
    }

    fn visit_children(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        self.panes
            .iter_mut()
            .zip(&mut self.contents)
            .zip(&mut tree.children)
            .zip(layout.children())
            .filter(|(((pane, _), _), _)| {
                self.internal
                    .maximized()
                    .is_none_or(|maximized| **pane == maximized)
            })
            .for_each(|(((_, content), state), layout)| {
                content.visit_children(state, layout, visit);
            });
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            self.panes
                .iter_mut()
                .zip(&mut self.contents)
                .zip(&mut tree.children)
                .zip(layout.children())
                .filter(|(((pane, _), _), _)| {
                    self.internal
                        .maximized()
                        .is_none_or(|maximized| **pane == maximized)
                })
                .for_each(|(((_, content), state), layout)| {
                    content.operate(state, layout, renderer, operation);
                });
        });
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
        let Memory {
            action,
            last_mouse_interaction,
            ..
        } = tree.state.downcast_mut();
        let node = self.internal.layout();

        let on_drag = if self.drag_enabled() {
            &self.on_drag
        } else {
            &None
        };

        let picked_pane = action.picked_pane().map(|(pane, _)| pane);

        for (((pane, content), tree), layout) in self
            .panes
            .iter()
            .copied()
            .zip(&mut self.contents)
            .zip(&mut tree.children)
            .zip(layout.children())
            .filter(|(((pane, _), _), _)| {
                self.internal
                    .maximized()
                    .is_none_or(|maximized| *pane == maximized)
            })
        {
            let is_picked = picked_pane == Some(pane);

            content.update(
                tree, event, layout, cursor, renderer, shell, viewport, is_picked,
            );
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                let bounds = layout.bounds();

                if let Some(cursor_position) = cursor.position_over(bounds) {
                    shell.capture_event();

                    match &self.on_resize {
                        Some((leeway, _)) => {
                            let relative_cursor = Point::new(
                                cursor_position.x - bounds.x,
                                cursor_position.y - bounds.y,
                            );

                            let splits =
                                node.split_regions(self.spacing, self.min_size, bounds.size());

                            let clicked_split = hovered_split(
                                splits.iter(),
                                self.spacing + leeway,
                                relative_cursor,
                            );

                            if let Some((split, axis, _)) = clicked_split {
                                if action.picked_pane().is_none() {
                                    *action = state::Action::Resizing { split, axis };
                                }
                            } else {
                                click_pane(
                                    action,
                                    layout,
                                    cursor_position,
                                    shell,
                                    self.panes.iter().copied().zip(&self.contents),
                                    &self.on_click,
                                    on_drag,
                                );
                            }
                        }
                        None => {
                            click_pane(
                                action,
                                layout,
                                cursor_position,
                                shell,
                                self.panes.iter().copied().zip(&self.contents),
                                &self.on_click,
                                on_drag,
                            );
                        }
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. })
            | Event::Touch(touch::Event::FingerLost { .. }) => {
                if let Some((pane, origin)) = action.picked_pane()
                    && let Some(on_drag) = on_drag
                    && let Some(cursor_position) = cursor.position()
                {
                    if cursor_position.distance_squared(origin)
                        > DRAG_DEADBAND_DISTANCE * DRAG_DEADBAND_DISTANCE
                    {
                        let event = if let Some(edge) = in_edge(layout, cursor_position) {
                            DragEvent::Dropped {
                                pane,
                                target: Target::Edge(edge),
                            }
                        } else {
                            let dropped_region = self
                                .panes
                                .iter()
                                .copied()
                                .zip(&self.contents)
                                .zip(layout.children())
                                .find_map(|(target, layout)| {
                                    layout_region(layout, cursor_position)
                                        .map(|region| (target, region))
                                });

                            match dropped_region {
                                Some(((target, _), region)) if pane != target => {
                                    DragEvent::Dropped {
                                        pane,
                                        target: Target::Pane(target, region),
                                    }
                                }
                                _ => DragEvent::Canceled { pane },
                            }
                        };

                        shell.publish(on_drag(event));
                    } else {
                        shell.publish(on_drag(DragEvent::Canceled { pane }));
                    }
                }

                *action = state::Action::Idle;
            }
            Event::Mouse(mouse::Event::CursorMoved { .. })
            | Event::Touch(touch::Event::FingerMoved { .. }) => {
                if let Some((_, on_resize)) = &self.on_resize {
                    if let Some((split, _)) = action.picked_split() {
                        let bounds = layout.bounds();

                        let splits = node.split_regions(self.spacing, self.min_size, bounds.size());

                        if let Some((axis, rectangle, _)) = splits.get(&split)
                            && let Some(cursor_position) = cursor.position()
                        {
                            let ratio = match axis {
                                Axis::Horizontal => {
                                    let position = cursor_position.y - bounds.y - rectangle.y;

                                    (position / rectangle.height).clamp(0.0, 1.0)
                                }
                                Axis::Vertical => {
                                    let position = cursor_position.x - bounds.x - rectangle.x;

                                    (position / rectangle.width).clamp(0.0, 1.0)
                                }
                            };

                            shell.publish(on_resize(ResizeEvent { split, ratio }));

                            shell.capture_event();
                        }
                    } else if action.picked_pane().is_some() {
                        shell.request_redraw();
                    }
                }
            }
            _ => {}
        }

        if shell.redraw_request() != window::RedrawRequest::NextFrame {
            let interaction = self
                .grid_interaction(action, layout, cursor)
                .or_else(|| {
                    self.panes
                        .iter()
                        .zip(&self.contents)
                        .zip(layout.children())
                        .filter(|((pane, _content), _layout)| {
                            self.internal
                                .maximized()
                                .is_none_or(|maximized| **pane == maximized)
                        })
                        .find_map(|((_pane, content), layout)| {
                            content.grid_interaction(layout, cursor, on_drag.is_some())
                        })
                })
                .unwrap_or(mouse::Interaction::None);

            if let Event::Window(window::Event::RedrawRequested(_now)) = event {
                *last_mouse_interaction = Some(interaction);
            } else if last_mouse_interaction
                .is_some_and(|last_mouse_interaction| last_mouse_interaction != interaction)
            {
                shell.request_redraw();
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let Memory { action, .. } = tree.state.downcast_ref();

        if let Some(grid_interaction) = self.grid_interaction(action, layout, cursor) {
            return grid_interaction;
        }

        self.panes
            .iter()
            .copied()
            .zip(&self.contents)
            .zip(&tree.children)
            .zip(layout.children())
            .filter(|(((pane, _), _), _)| {
                self.internal
                    .maximized()
                    .is_none_or(|maximized| *pane == maximized)
            })
            .map(|(((_, content), tree), layout)| {
                content.mouse_interaction(
                    tree,
                    layout,
                    cursor,
                    viewport,
                    renderer,
                    self.drag_enabled(),
                )
            })
            .max()
            .unwrap_or_default()
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        defaults: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let Memory { action, .. } = tree.state.downcast_ref();
        let node = self.internal.layout();
        let resize_leeway = self.on_resize.as_ref().map(|(leeway, _)| *leeway);

        let picked_pane = action.picked_pane();
        let dragged_pane = picked_pane
            .filter(|(_, origin)| is_dragging(*origin, cursor.position().unwrap_or_default()));

        let picked_split = action
            .picked_split()
            .and_then(|(split, axis)| {
                let bounds = layout.bounds();

                let splits = node.split_regions(self.spacing, self.min_size, bounds.size());

                let (_axis, region, ratio) = splits.get(&split)?;

                let region = axis.split_line_bounds(*region, *ratio, self.spacing);

                Some((axis, region + Vector::new(bounds.x, bounds.y), true))
            })
            .or_else(|| match resize_leeway {
                Some(leeway) => {
                    let cursor_position = cursor.position()?;
                    let bounds = layout.bounds();

                    let relative_cursor =
                        Point::new(cursor_position.x - bounds.x, cursor_position.y - bounds.y);

                    let splits = node.split_regions(self.spacing, self.min_size, bounds.size());

                    let (_split, axis, region) =
                        hovered_split(splits.iter(), self.spacing + leeway, relative_cursor)?;

                    Some((axis, region + Vector::new(bounds.x, bounds.y), false))
                }
                None => None,
            });

        let pane_cursor = if picked_pane.is_some() {
            mouse::Cursor::Unavailable
        } else {
            cursor
        };

        let pane_in_edge = if picked_pane.is_some() {
            cursor
                .position()
                .and_then(|cursor_position| in_edge(layout, cursor_position))
        } else {
            None
        };

        let style = Catalog::style(theme, &self.class);

        for (((id, content), tree), pane_layout) in self
            .panes
            .iter()
            .copied()
            .zip(&self.contents)
            .zip(&tree.children)
            .zip(layout.children())
            .filter(|(((pane, _), _), _)| {
                self.internal
                    .maximized()
                    .is_none_or(|maximized| maximized == *pane)
            })
        {
            match picked_pane {
                Some((dragging, _)) if id == dragging => {}
                Some(_) => {
                    content.draw(
                        tree,
                        renderer,
                        theme,
                        defaults,
                        pane_layout,
                        pane_cursor,
                        viewport,
                    );

                    if let Some(cursor_position) = cursor.position()
                        && dragged_pane.is_some()
                        && pane_in_edge.is_none()
                        && let Some(region) = layout_region(pane_layout, cursor_position)
                    {
                        let bounds = layout_region_bounds(pane_layout, region);

                        renderer.fill_quad(
                            renderer::Quad {
                                bounds,
                                border: style.hovered_region.border,
                                ..renderer::Quad::default()
                            },
                            style.hovered_region.background,
                        );
                    }
                }
                _ => {
                    content.draw(
                        tree,
                        renderer,
                        theme,
                        defaults,
                        pane_layout,
                        pane_cursor,
                        viewport,
                    );
                }
            }
        }

        if dragged_pane.is_some()
            && let Some(edge) = pane_in_edge
        {
            let bounds = edge_bounds(layout, edge);

            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: style.hovered_region.border,
                    ..renderer::Quad::default()
                },
                style.hovered_region.background,
            );
        }

        if dragged_pane.is_none()
            && let Some((axis, split_region, is_picked)) = picked_split
        {
            let highlight = if is_picked {
                style.picked_split
            } else {
                style.hovered_split
            };

            renderer.fill_quad(
                renderer::Quad {
                    bounds: match axis {
                        Axis::Horizontal => Rectangle {
                            x: split_region.x,
                            y: (split_region.y + (split_region.height - highlight.width) / 2.0)
                                .round(),
                            width: split_region.width,
                            height: highlight.width,
                        },
                        Axis::Vertical => Rectangle {
                            x: (split_region.x + (split_region.width - highlight.width) / 2.0)
                                .round(),
                            y: split_region.y,
                            width: highlight.width,
                            height: split_region.height,
                        },
                    },
                    ..renderer::Quad::default()
                },
                highlight.color,
            );
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let state = tree.state.downcast_ref::<Memory>();
        let picked_pane = state.action.picked_pane();

        let children = self
            .panes
            .iter()
            .copied()
            .zip(&mut self.contents)
            .zip(&mut tree.children)
            .zip(layout.children())
            .filter_map(|(((pane, content), tree), layout)| {
                if self
                    .internal
                    .maximized()
                    .is_some_and(|maximized| maximized != pane)
                {
                    return None;
                }

                if let Some((picked_pane, origin)) = picked_pane
                    && picked_pane == pane
                {
                    return Some(overlay::Element::new(Box::new(PickedPane {
                        origin,
                        content,
                        tree,
                        layout,
                    })));
                }

                content.overlay(tree, layout, renderer, viewport, translation)
            })
            .collect::<Vec<_>>();

        (!children.is_empty()).then(|| Group::with_children(children).overlay())
    }
}

struct PickedPane<'a, 'b, Message, Theme, Renderer>
where
    Theme: container::Catalog,
    Renderer: core::Renderer,
{
    content: &'a Content<'b, Message, Theme, Renderer>,
    origin: Point,
    tree: &'a mut Tree,
    layout: Layout<'a>,
}

impl<'a, 'b, Message, Theme, Renderer> core::Overlay<Message, Theme, Renderer>
    for PickedPane<'a, 'b, Message, Theme, Renderer>
where
    Theme: container::Catalog,
    Renderer: core::Renderer,
{
    fn layout(&mut self, _renderer: &Renderer, _bounds: Size) -> layout::Node {
        // TODO: Mouse translation
        layout::Node::new(self.layout.bounds().size()).move_to(self.origin)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        _layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let cursor_position = cursor.position().unwrap_or_default();

        let translation = if is_dragging(self.origin, cursor_position) {
            cursor_position - self.origin
        } else {
            Vector::ZERO
        };

        renderer.with_translation(translation, |renderer| {
            self.content.draw(
                self.tree,
                renderer,
                theme,
                style,
                self.layout,
                mouse::Cursor::Unavailable,
                &Rectangle::INFINITE,
            );
        });
    }
}

fn is_dragging(origin: Point, cursor: Point) -> bool {
    cursor.distance_squared(origin) > DRAG_DEADBAND_DISTANCE * DRAG_DEADBAND_DISTANCE
}

impl<'a, Message, Theme, Renderer> From<PaneGrid<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: Catalog + 'a,
    Renderer: core::Renderer + 'a,
{
    fn from(
        pane_grid: PaneGrid<'a, Message, Theme, Renderer>,
    ) -> Element<'a, Message, Theme, Renderer> {
        Element::new(pane_grid)
    }
}

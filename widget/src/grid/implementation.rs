use super::engine::{State, Track};
use super::{Grid, Sizing};
use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::widget::visit;
use crate::core::widget::{self, Operation, Tree};
use crate::core::{Element, Event, Length, Rectangle, Shell, Size, Vector, Widget};

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Grid<'_, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(&mut self.children);
        tree.state.downcast_mut::<State>().invalidate();
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self
                .width
                .map(|pixels| Length::Fixed(pixels.0))
                .unwrap_or(Length::Fill),
            height: match self.height {
                Sizing::AspectRatio(_) => Length::Shrink,
                Sizing::EvenlyDistribute(length) => length,
            },
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let size = self.size();
        let limits = limits.width(size.width).height(size.height);
        let available = limits.max();

        if limits.compression().width && self.width.is_none() {
            return layout::Node::new(Size::ZERO);
        }

        let state = tree.state.downcast_mut::<State>();
        let Some(track) = self.track(state, available) else {
            return layout::Node::new(limits.resolve(size.width, size.height, Size::ZERO));
        };
        let Track {
            cells_per_row,
            cell_width,
            cell_height,
            ..
        } = track;
        let cell_limits = layout::Limits::new(
            Size::new(cell_width, cell_height.unwrap_or(0.0)),
            Size::new(cell_width, cell_height.unwrap_or(available.height)),
        );
        let mut nodes = Vec::with_capacity(self.children.len());
        let mut x = 0.0;
        let mut y = 0.0;
        let mut row_height = 0.0f32;
        let child_count = self.children.len();

        for (i, (child, tree)) in self.children.iter_mut().zip(&mut tree.children).enumerate() {
            let node = child
                .as_widget_mut()
                .layout(tree, renderer, &cell_limits)
                .move_to((x, y));
            let size = node.size();

            x += size.width + self.spacing;
            row_height = row_height.max(size.height);

            if (i + 1) % cells_per_row == 0 {
                y += cell_height.unwrap_or(row_height);
                if i + 1 < child_count {
                    y += self.spacing;
                }
                x = 0.0;
                row_height = 0.0;
            }

            nodes.push(node);
        }

        if x != 0.0 {
            y += cell_height.unwrap_or(row_height);
        }

        layout::Node::with_children(Size::new(available.width, y), nodes)
    }

    fn visit_children(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        self.children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .for_each(|((child, state), layout)| {
                visit(&mut visit::Widgets::new(
                    child.as_widget_mut(),
                    state,
                    layout,
                ));
            });
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            self.children
                .iter_mut()
                .zip(&mut tree.children)
                .zip(layout.children())
                .for_each(|((child, state), layout)| {
                    child
                        .as_widget_mut()
                        .operate(state, layout, renderer, operation);
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
        for ((child, tree), layout) in self
            .children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
        {
            child
                .as_widget_mut()
                .update(tree, event, layout, cursor, renderer, shell, viewport);
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
        self.children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((child, tree), layout)| {
                child
                    .as_widget()
                    .mouse_interaction(tree, layout, cursor, viewport, renderer)
            })
            .max()
            .unwrap_or_default()
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
        if let Some(viewport) = layout.bounds().intersection(viewport) {
            for ((child, tree), layout) in self
                .children
                .iter()
                .zip(&tree.children)
                .zip(layout.children())
            {
                let widget = child.as_widget();
                if widget.paint_bounds(tree, layout).intersects(&viewport) {
                    widget.draw(tree, renderer, theme, style, layout, cursor, &viewport);
                }
            }
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
        overlay::from_children(
            &mut self.children,
            tree,
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<Grid<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: crate::core::Renderer + 'a,
{
    fn from(grid: Grid<'a, Message, Theme, Renderer>) -> Self {
        Self::new(grid)
    }
}

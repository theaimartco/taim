use super::{Stack, State};
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::widget::visit;
use crate::core::widget::{self, Operation, Tree};
use crate::core::{Element, Event, Layout, Length, Rectangle, Shell, Size, Vector, Widget};

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Stack<'a, Message, Theme, Renderer>
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
        tree.state.downcast_mut::<State>().base.clear();

        if let Some(base) = self.children.get(self.base_layer) {
            let size = base.as_widget().size();
            self.width = self.width.cross(size.width);
            self.height = self.height.cross(size.height);
        }
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let limits = limits.width(self.width).height(self.height);

        if self.children.len() <= self.base_layer {
            return layout::Node::new(limits.resolve(self.width, self.height, Size::ZERO));
        }

        let fingerprint = layout::cache::Fingerprint::build(|hasher| {
            hasher.write_usize(self.children.len());
            self.width.hash_stable(hasher);
            self.height.hash_stable(hasher);
        });

        let base_layer = self.base_layer;
        let base_child = &mut self.children[base_layer];
        let base_tree = &mut tree.children[base_layer];
        let state = tree.state.downcast_mut::<State>();
        let base = state.base.get_or_insert_with(&limits, fingerprint, || {
            base_child
                .as_widget_mut()
                .layout(base_tree, renderer, &limits)
        });

        let size = limits.resolve(self.width, self.height, base.size());
        let child_limits = layout::Limits::new(Size::ZERO, size);
        let (under, above) = self.children.split_at_mut(base_layer);
        let (tree_under, tree_above) = tree.children.split_at_mut(base_layer);
        let with_clip = |node: layout::Node| {
            if self.clip {
                let bounds = node.bounds();
                node.with_clip(Rectangle::new(bounds.position(), bounds.size()))
            } else {
                node
            }
        };

        let nodes = under
            .iter_mut()
            .zip(tree_under)
            .map(|(layer, tree)| {
                with_clip(layer.as_widget_mut().layout(tree, renderer, &child_limits))
            })
            .chain(std::iter::once(with_clip(base)))
            .chain(
                above[1..]
                    .iter_mut()
                    .zip(&mut tree_above[1..])
                    .map(|(layer, tree)| {
                        with_clip(layer.as_widget_mut().layout(tree, renderer, &child_limits))
                    }),
            )
            .collect();

        layout::Node::with_children(size, nodes)
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
        mut cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if self.children.is_empty() {
            return;
        }

        let is_over = cursor.is_over(layout.bounds());
        let end = self.children.len() - 1;

        for (i, ((child, tree), layout)) in self
            .children
            .iter_mut()
            .rev()
            .zip(tree.children.iter_mut().rev())
            .zip(layout.children().rev())
            .enumerate()
        {
            child
                .as_widget_mut()
                .update(tree, event, layout, cursor, renderer, shell, viewport);

            if shell.is_event_captured() {
                return;
            }

            if i < end && is_over && !cursor.is_levitating() {
                let interaction = child
                    .as_widget()
                    .mouse_interaction(tree, layout, cursor, viewport, renderer);

                if interaction != mouse::Interaction::None {
                    cursor = cursor.levitate();
                }
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
        self.children
            .iter()
            .rev()
            .zip(tree.children.iter().rev())
            .zip(layout.children().rev())
            .map(|((child, tree), layout)| {
                child
                    .as_widget()
                    .mouse_interaction(tree, layout, cursor, viewport, renderer)
            })
            .find(|&interaction| interaction != mouse::Interaction::None)
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
        let Some(clipped_viewport) = layout.bounds().intersection(viewport) else {
            return;
        };
        let viewport = if self.clip {
            &clipped_viewport
        } else {
            viewport
        };

        let layers_under = if cursor.is_over(layout.bounds()) {
            self.children
                .iter()
                .rev()
                .zip(tree.children.iter().rev())
                .zip(layout.children().rev())
                .position(|((layer, tree), layout)| {
                    layer
                        .as_widget()
                        .mouse_interaction(tree, layout, cursor, viewport, renderer)
                        != mouse::Interaction::None
                })
                .map(|i| self.children.len() - i - 1)
                .unwrap_or_default()
        } else {
            0
        };

        let mut layers = self
            .children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .enumerate();
        let mut draw_layer =
            |i, layer: &Element<'a, Message, Theme, Renderer>, tree, layout, cursor| {
                let widget = layer.as_widget();
                if !widget.paint_bounds(tree, layout).intersects(viewport) {
                    return;
                }

                if i > 0 {
                    renderer.with_layer(*viewport, |renderer| {
                        widget.draw(tree, renderer, theme, style, layout, cursor, viewport);
                    });
                } else {
                    widget.draw(tree, renderer, theme, style, layout, cursor, viewport);
                }
            };

        for (i, ((layer, tree), layout)) in layers.by_ref().take(layers_under) {
            draw_layer(i, layer, tree, layout, mouse::Cursor::Unavailable);
        }
        for (i, ((layer, tree), layout)) in layers {
            draw_layer(i, layer, tree, layout, cursor);
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

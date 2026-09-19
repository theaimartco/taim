use crate::event;
use crate::layout;
use crate::mouse;
use crate::overlay;
use crate::renderer;
use crate::widget;
use crate::{Event, Layout, Shell, Size};

const MAX_OVERLAY_DEPTH: usize = 64;

/// An overlay container that displays nested overlays
pub struct Nested<'a, Message, Theme, Renderer> {
    overlay: overlay::Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Theme, Renderer> Nested<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    /// Creates a nested overlay from the provided [`overlay::Element`]
    pub fn new(element: overlay::Element<'a, Message, Theme, Renderer>) -> Self {
        Self { overlay: element }
    }

    /// Returns the layout [`Node`] of the [`Nested`] overlay.
    ///
    /// [`Node`]: layout::Node
    pub fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        fn recurse<Message, Theme, Renderer>(
            element: &mut overlay::Element<'_, Message, Theme, Renderer>,
            renderer: &Renderer,
            bounds: Size,
            depth: usize,
        ) -> layout::Node
        where
            Renderer: renderer::Renderer,
        {
            let overlay = element.as_overlay_mut();
            let node = overlay.layout(renderer, bounds);

            let nested_node = (depth < MAX_OVERLAY_DEPTH)
                .then(|| overlay.overlay(Layout::new(&node), renderer))
                .flatten()
                .as_mut()
                .map(|nested| recurse(nested, renderer, bounds, depth + 1));

            let mut chain = Vec::with_capacity(1 + usize::from(nested_node.is_some()));
            chain.push(node);
            chain.extend(nested_node);
            layout::Node::with_children(chain[0].size(), chain)
        }

        recurse(&mut self.overlay, renderer, bounds, 0)
    }

    /// Draws the [`Nested`] overlay using the associated `Renderer`.
    pub fn draw(
        &mut self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        fn recurse<Message, Theme, Renderer>(
            element: &mut overlay::Element<'_, Message, Theme, Renderer>,
            layout: Layout<'_>,
            renderer: &mut Renderer,
            theme: &Theme,
            style: &renderer::Style,
            cursor: mouse::Cursor,
            depth: usize,
        ) where
            Renderer: renderer::Renderer,
        {
            if let Some((layout, nested_layout)) = split_layout(layout) {
                let overlay = element.as_overlay_mut();

                let is_over = cursor
                    .position()
                    .zip(nested_layout)
                    .and_then(|(cursor_position, nested_layout)| {
                        (depth < MAX_OVERLAY_DEPTH)
                            .then(|| overlay.overlay(layout, renderer))
                            .flatten()
                            .zip(nested_layout.children().next())
                            .map(|(nested, nested_layout)| {
                                nested.as_overlay().mouse_interaction(
                                    nested_layout,
                                    mouse::Cursor::Available(cursor_position),
                                    renderer,
                                ) != mouse::Interaction::None
                            })
                    })
                    .unwrap_or_default();

                renderer.with_layer(layout.bounds(), |renderer| {
                    overlay.draw(
                        renderer,
                        theme,
                        style,
                        layout,
                        if is_over {
                            mouse::Cursor::Unavailable
                        } else {
                            cursor
                        },
                    );
                });

                if let Some((mut nested, nested_layout)) = (depth < MAX_OVERLAY_DEPTH)
                    .then(|| overlay.overlay(layout, renderer))
                    .flatten()
                    .zip(nested_layout)
                {
                    recurse(
                        &mut nested,
                        nested_layout,
                        renderer,
                        theme,
                        style,
                        cursor,
                        depth + 1,
                    );
                }
            }
        }

        recurse(&mut self.overlay, layout, renderer, theme, style, cursor, 0);
    }

    /// Applies a [`widget::Operation`] to the [`Nested`] overlay.
    /// Collects the semantics of this overlay and every overlay nested
    /// inside it, outermost first.
    ///
    /// A forest rather than a single node: an overlay chain (a menu whose
    /// item opens a submenu) has no natural parent to gather under, and
    /// each level is an independent thing a screen reader may land on.
    ///
    /// This is the piece that was missing entirely before: the runtime
    /// walks widgets for semantics, and an overlay is not in the widget
    /// tree, so nothing ever asked it. See
    /// [`Overlay::semantics`](overlay::Overlay::semantics).
    ///
    /// Takes `&mut self` -- as [`Nested::operate`] does, and for the same
    /// reason: reaching a nested overlay goes through
    /// [`Overlay::overlay`](overlay::Overlay::overlay), which needs a
    /// mutable borrow. Producing semantics is conceptually a read, but the
    /// traversal is not.
    pub fn semantics(
        &mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
    ) -> Vec<crate::semantics::Node> {
        fn recurse<Message, Theme, Renderer>(
            element: &mut overlay::Element<'_, Message, Theme, Renderer>,
            layout: Layout<'_>,
            renderer: &Renderer,
            found: &mut Vec<crate::semantics::Node>,
            depth: usize,
        ) where
            Renderer: renderer::Renderer,
        {
            let Some((layout, nested_layout)) = split_layout(layout) else {
                return;
            };

            let overlay = element.as_overlay_mut();

            if let Some(node) = overlay.semantics(layout) {
                found.push(node);
            }

            if let Some((mut nested, nested_layout)) = (depth < MAX_OVERLAY_DEPTH)
                .then(|| overlay.overlay(layout, renderer))
                .flatten()
                .zip(nested_layout)
            {
                recurse(&mut nested, nested_layout, renderer, found, depth + 1);
            }
        }

        let mut found = Vec::with_capacity(4);
        recurse(&mut self.overlay, layout, renderer, &mut found, 0);
        found
    }

    /// Applies a [`widget::Operation`] to the [`Nested`] overlay.
    pub fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        fn recurse<Message, Theme, Renderer>(
            element: &mut overlay::Element<'_, Message, Theme, Renderer>,
            layout: Layout<'_>,
            renderer: &Renderer,
            operation: &mut dyn widget::Operation,
            depth: usize,
        ) where
            Renderer: renderer::Renderer,
        {
            if let Some((layout, nested_layout)) = split_layout(layout) {
                let overlay = element.as_overlay_mut();

                overlay.operate(layout, renderer, operation);

                if let Some((mut nested, nested_layout)) = (depth < MAX_OVERLAY_DEPTH)
                    .then(|| overlay.overlay(layout, renderer))
                    .flatten()
                    .zip(nested_layout)
                {
                    recurse(&mut nested, nested_layout, renderer, operation, depth + 1);
                }
            }
        }

        recurse(&mut self.overlay, layout, renderer, operation, 0);
    }

    /// Processes a runtime [`Event`].
    pub fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        fn recurse<Message, Theme, Renderer>(
            element: &mut overlay::Element<'_, Message, Theme, Renderer>,
            layout: Layout<'_>,
            event: &Event,
            cursor: mouse::Cursor,
            renderer: &Renderer,
            shell: &mut Shell<'_, Message>,
            depth: usize,
        ) -> bool
        where
            Renderer: renderer::Renderer,
        {
            if let Some((layout, nested_layout)) = split_layout(layout) {
                let overlay = element.as_overlay_mut();

                let nested_is_over = if let Some((mut nested, nested_layout)) =
                    (depth < MAX_OVERLAY_DEPTH)
                        .then(|| overlay.overlay(layout, renderer))
                        .flatten()
                        .zip(nested_layout)
                {
                    recurse(
                        &mut nested,
                        nested_layout,
                        event,
                        cursor,
                        renderer,
                        shell,
                        depth + 1,
                    )
                } else {
                    false
                };

                if shell.event_status() == event::Status::Ignored {
                    let is_over = nested_is_over
                        || cursor
                            .position()
                            .map(|cursor_position| {
                                overlay.mouse_interaction(
                                    layout,
                                    mouse::Cursor::Available(cursor_position),
                                    renderer,
                                ) != mouse::Interaction::None
                            })
                            .unwrap_or_default();

                    overlay.update(
                        event,
                        layout,
                        if nested_is_over {
                            mouse::Cursor::Unavailable
                        } else {
                            cursor
                        },
                        renderer,
                        shell,
                    );

                    is_over
                } else {
                    nested_is_over
                }
            } else {
                false
            }
        }

        let _ = recurse(&mut self.overlay, layout, event, cursor, renderer, shell, 0);
    }

    /// Returns the current [`mouse::Interaction`] of the [`Nested`] overlay.
    pub fn mouse_interaction(
        &mut self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        fn recurse<Message, Theme, Renderer>(
            element: &mut overlay::Element<'_, Message, Theme, Renderer>,
            layout: Layout<'_>,
            cursor: mouse::Cursor,
            renderer: &Renderer,
            depth: usize,
        ) -> Option<mouse::Interaction>
        where
            Renderer: renderer::Renderer,
        {
            let (layout, nested_layout) = split_layout(layout)?;
            let overlay = element.as_overlay_mut();
            let interaction = overlay.mouse_interaction(layout, cursor, renderer);

            Some(
                (depth < MAX_OVERLAY_DEPTH)
                    .then(|| overlay.overlay(layout, renderer))
                    .flatten()
                    .zip(nested_layout)
                    .and_then(|(mut overlay, layout)| {
                        recurse(&mut overlay, layout, cursor, renderer, depth + 1)
                    })
                    .unwrap_or_default()
                    .max(interaction),
            )
        }

        recurse(&mut self.overlay, layout, cursor, renderer, 0).unwrap_or_default()
    }
}

#[inline]
fn split_layout(layout: Layout<'_>) -> Option<(Layout<'_>, Option<Layout<'_>>)> {
    let mut children = layout.children();
    let current = children.next()?;
    Some((current, children.next()))
}

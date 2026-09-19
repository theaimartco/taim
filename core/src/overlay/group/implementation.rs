use super::Group;
use crate::layout;
use crate::mouse;
use crate::overlay;
use crate::renderer;
use crate::widget;
use crate::{Event, Layout, Overlay, Shell, Size};

impl<Message, Theme, Renderer> Overlay<Message, Theme, Renderer>
    for Group<'_, Message, Theme, Renderer>
where
    Renderer: crate::Renderer,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        let mut nodes = Vec::with_capacity(self.children.len());
        nodes.extend(
            self.children
                .iter_mut()
                .map(|child| child.as_overlay_mut().layout(renderer, bounds)),
        );

        layout::Node::with_children(bounds, nodes)
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        for (child, child_layout) in self.children.iter_mut().zip(layout.children()) {
            child
                .as_overlay_mut()
                .update(event, child_layout, cursor, renderer, shell);
        }
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        for (child, child_layout) in self.children.iter().zip(layout.children()) {
            child
                .as_overlay()
                .draw(renderer, theme, style, child_layout, cursor);
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.children.iter().zip(layout.children()).fold(
            mouse::Interaction::None,
            |current, (child, child_layout)| {
                current.max(
                    child
                        .as_overlay()
                        .mouse_interaction(child_layout, cursor, renderer),
                )
            },
        )
    }

    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        operation.traverse(&mut |operation| {
            for (child, child_layout) in self.children.iter_mut().zip(layout.children()) {
                child
                    .as_overlay_mut()
                    .operate(child_layout, renderer, operation);
            }
        });
    }

    fn overlay<'a>(
        &'a mut self,
        layout: Layout<'a>,
        renderer: &Renderer,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        overlay::collect(self.children.iter_mut().zip(layout.children()).filter_map(
            |(child, child_layout)| child.as_overlay_mut().overlay(child_layout, renderer),
        ))
    }

    fn index(&self) -> f32 {
        self.children
            .first()
            .map_or(1.0, |child| child.as_overlay().index())
    }
}

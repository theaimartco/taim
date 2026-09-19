use crate::core;
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::semantics;
use crate::core::widget;
use crate::core::widget::visit;
use crate::core::{Element, Event, Layout, Length, Rectangle, Shell, Size, Vector, Widget};

use super::Pin;

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Pin<'_, Message, Theme, Renderer>
where
    Renderer: core::Renderer,
{
    fn tag(&self) -> widget::tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> widget::tree::State {
        self.content.as_widget().state()
    }

    fn diff(&mut self, tree: &mut widget::Tree) {
        self.content.as_widget_mut().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let limits = limits.width(self.width).height(self.height);
        let maximum = limits.max();
        let available = Size::new(
            (maximum.width - self.position.x).max(0.0),
            (maximum.height - self.position.y).max(0.0),
        );
        let content = self
            .content
            .as_widget_mut()
            .layout(tree, renderer, &layout::Limits::new(Size::ZERO, available))
            .move_to(self.position);
        let size = limits.resolve(self.width, self.height, content.size());

        layout::Node::with_children(size, vec![content])
    }

    fn semantics(&self, tree: &widget::Tree, layout: Layout<'_>) -> Option<semantics::Node> {
        self.content
            .as_widget()
            .semantics(tree, child_layout(layout))
    }

    fn visit_children(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        self.content
            .as_widget_mut()
            .visit_children(tree, child_layout(layout), visit);
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(tree, child_layout(layout), renderer, operation);
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
        self.content.as_widget_mut().update(
            tree,
            event,
            child_layout(layout),
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
            tree,
            child_layout(layout),
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
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let Some(clipped) = layout.bounds().intersection(viewport) else {
            return;
        };

        self.content.as_widget().draw(
            tree,
            renderer,
            theme,
            style,
            child_layout(layout),
            cursor,
            &clipped,
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
        self.content.as_widget_mut().overlay(
            tree,
            child_layout(layout),
            renderer,
            viewport,
            translation,
        )
    }
}

#[inline]
fn child_layout(layout: Layout<'_>) -> Layout<'_> {
    layout.children().next().expect("Pin must have one child")
}

impl<'a, Message, Theme, Renderer> From<Pin<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: core::Renderer + 'a,
{
    fn from(pin: Pin<'a, Message, Theme, Renderer>) -> Self {
        Element::new(pin)
    }
}

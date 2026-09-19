use crate::core;
use crate::core::border;
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::semantics;
use crate::core::widget;
use crate::core::widget::tree;
use crate::core::widget::visit;
use crate::core::{
    Element, Event, Layout, Length, Rectangle, Shell, Size, Transformation, Vector, Widget,
};

use super::{Catalog, Float};

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Float<'_, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: core::Renderer,
{
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn diff(&mut self, tree: &mut widget::Tree) {
        self.content.as_widget_mut().diff(tree);
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
        self.content.as_widget_mut().layout(tree, renderer, limits)
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
        if !self.is_floating(layout.bounds(), *viewport) {
            self.content
                .as_widget_mut()
                .update(tree, event, layout, cursor, renderer, shell, viewport);
        }
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
        if self.is_floating(layout.bounds(), *viewport) {
            return;
        }

        draw_shadow(self, renderer, theme, layout.bounds());
        self.content.as_widget().draw(
            tree,
            renderer,
            theme,
            inherited_style,
            layout,
            cursor,
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
        if self.is_floating(layout.bounds(), *viewport) {
            mouse::Interaction::None
        } else {
            self.content
                .as_widget()
                .mouse_interaction(tree, layout, cursor, viewport, renderer)
        }
    }

    fn semantics(&self, tree: &widget::Tree, layout: Layout<'_>) -> Option<semantics::Node> {
        self.content.as_widget().semantics(tree, layout)
    }

    fn visit_children(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        self.content
            .as_widget_mut()
            .visit_children(tree, layout, visit);
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
            .operate(tree, layout, renderer, operation);
    }

    fn overlay<'a>(
        &'a mut self,
        state: &'a mut widget::Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        offset: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        let bounds = layout.bounds();
        let translation = self
            .translate
            .as_ref()
            .map(|translate| translate(bounds + offset, *viewport))
            .unwrap_or(Vector::ZERO);

        if self.scale == 1.0 && translation == Vector::ZERO {
            return self
                .content
                .as_widget_mut()
                .overlay(state, layout, renderer, viewport, offset);
        }

        let translation = translation + offset;
        let transformation = Transformation::translate(
            bounds.x + bounds.width / 2.0 + translation.x,
            bounds.y + bounds.height / 2.0 + translation.y,
        ) * Transformation::scale(self.scale)
            * Transformation::translate(
                -bounds.x - bounds.width / 2.0,
                -bounds.y - bounds.height / 2.0,
            );
        let inverse = transformation.inverse();

        Some(overlay::Element::new(Box::new(Overlay {
            float: self,
            state,
            layout,
            viewport: *viewport,
            content_viewport: *viewport * inverse,
            transformation,
            inverse,
        })))
    }
}

impl<'a, Message, Theme, Renderer> From<Float<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: Catalog + 'a,
    Renderer: core::Renderer + 'a,
{
    fn from(float: Float<'a, Message, Theme, Renderer>) -> Self {
        Element::new(float)
    }
}

struct Overlay<'a, 'b, Message, Theme, Renderer>
where
    Theme: Catalog,
{
    float: &'a mut Float<'b, Message, Theme, Renderer>,
    state: &'a mut widget::Tree,
    layout: Layout<'a>,
    viewport: Rectangle,
    content_viewport: Rectangle,
    transformation: Transformation,
    inverse: Transformation,
}

impl<Message, Theme, Renderer> core::Overlay<Message, Theme, Renderer>
    for Overlay<'_, '_, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: core::Renderer,
{
    fn layout(&mut self, _renderer: &Renderer, _bounds: Size) -> layout::Node {
        let bounds = self.layout.bounds() * self.transformation;
        layout::Node::new(bounds.size()).move_to(bounds.position())
    }

    fn update(
        &mut self,
        event: &Event,
        _layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        self.float.content.as_widget_mut().update(
            self.state,
            event,
            self.layout,
            cursor * self.inverse,
            renderer,
            shell,
            &self.content_viewport,
        );
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        inherited_style: &renderer::Style,
        _layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        renderer.with_layer(self.viewport, |renderer| {
            renderer.with_transformation(self.transformation, |renderer| {
                draw_shadow(self.float, renderer, theme, self.layout.bounds());
                self.float.content.as_widget().draw(
                    self.state,
                    renderer,
                    theme,
                    inherited_style,
                    self.layout,
                    cursor * self.inverse,
                    &self.content_viewport,
                );
            });
        });
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if !cursor.is_over(layout.bounds()) {
            return mouse::Interaction::None;
        }

        self.float.content.as_widget().mouse_interaction(
            self.state,
            self.layout,
            cursor * self.inverse,
            &self.content_viewport,
            renderer,
        )
    }

    fn index(&self) -> f32 {
        self.float.scale * 0.5
    }

    fn overlay<'a>(
        &'a mut self,
        _layout: Layout<'_>,
        renderer: &Renderer,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        self.float.content.as_widget_mut().overlay(
            self.state,
            self.layout,
            renderer,
            &self.content_viewport,
            self.transformation.translation(),
        )
    }
}

fn draw_shadow<Message, Theme, Renderer>(
    float: &Float<'_, Message, Theme, Renderer>,
    renderer: &mut Renderer,
    theme: &Theme,
    bounds: Rectangle,
) where
    Theme: Catalog,
    Renderer: core::Renderer,
{
    let style = theme.style(&float.class);

    if style.shadow.color.a > 0.0 {
        renderer.fill_quad(
            renderer::Quad {
                bounds: bounds.shrink(1.0),
                shadow: style.shadow,
                border: border::rounded(style.shadow_border_radius),
                snap: false,
                ..renderer::Quad::default()
            },
            style.shadow.color,
        );
    }
}

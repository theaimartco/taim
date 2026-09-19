use crate::container;
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::semantics;
use crate::core::theme;
use crate::core::widget::Operation;
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::visit;
use crate::core::{Element, Event, Layout, Length, Rectangle, Shell, Size, Vector, Widget};

use super::Themer;

impl<Message, Theme, Renderer, AnyTheme> Widget<Message, AnyTheme, Renderer>
    for Themer<'_, Message, Theme, Renderer>
where
    Theme: theme::Base,
    AnyTheme: theme::Base,
    Renderer: crate::core::Renderer,
{
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn diff(&mut self, tree: &mut Tree) {
        self.content.as_widget_mut().diff(tree);
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
        self.content.as_widget_mut().layout(tree, renderer, limits)
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<semantics::Node> {
        self.content.as_widget().semantics(tree, layout)
    }

    fn visit_children(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        self.content
            .as_widget_mut()
            .visit_children(tree, layout, visit);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
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
        self.content
            .as_widget_mut()
            .update(tree, event, layout, cursor, renderer, shell, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        inherited_theme: &AnyTheme,
        inherited_style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let fallback;
        let theme = match self.theme.as_ref() {
            Some(theme) => theme,
            None => {
                fallback = theme::Base::default(inherited_theme.mode());
                &fallback
            }
        };

        if let Some(background) = self.background {
            container::draw_background(
                renderer,
                &container::Style {
                    background: Some(background(theme)),
                    ..container::Style::default()
                },
                layout.bounds(),
            );
        }

        let local_style;
        let style = if let Some(text_color) = self.text_color {
            local_style = renderer::Style {
                text_color: text_color(theme),
            };
            &local_style
        } else {
            inherited_style
        };

        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, AnyTheme, Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
            .map(|content| {
                overlay::Element::new(Box::new(ThemeOverlay {
                    theme: &self.theme,
                    content,
                }))
            })
    }
}

struct ThemeOverlay<'a, Message, Theme, Renderer> {
    theme: &'a Option<Theme>,
    content: overlay::Element<'a, Message, Theme, Renderer>,
}

impl<Message, Theme, Renderer, AnyTheme> overlay::Overlay<Message, AnyTheme, Renderer>
    for ThemeOverlay<'_, Message, Theme, Renderer>
where
    Theme: theme::Base,
    AnyTheme: theme::Base,
    Renderer: crate::core::Renderer,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        self.content.as_overlay_mut().layout(renderer, bounds)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        inherited_theme: &AnyTheme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let fallback;
        let theme = match self.theme.as_ref() {
            Some(theme) => theme,
            None => {
                fallback = theme::Base::default(inherited_theme.mode());
                &fallback
            }
        };

        self.content
            .as_overlay()
            .draw(renderer, theme, style, layout, cursor);
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        self.content
            .as_overlay_mut()
            .update(event, layout, cursor, renderer, shell);
    }

    fn operate(&mut self, layout: Layout<'_>, renderer: &Renderer, operation: &mut dyn Operation) {
        self.content
            .as_overlay_mut()
            .operate(layout, renderer, operation);
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_overlay()
            .mouse_interaction(layout, cursor, renderer)
    }

    fn overlay<'a>(
        &'a mut self,
        layout: Layout<'a>,
        renderer: &Renderer,
    ) -> Option<overlay::Element<'a, Message, AnyTheme, Renderer>> {
        self.content
            .as_overlay_mut()
            .overlay(layout, renderer)
            .map(|content| {
                overlay::Element::new(Box::new(ThemeOverlay {
                    theme: self.theme,
                    content,
                }))
            })
    }
}

impl<'a, Message, Theme, Renderer, AnyTheme> From<Themer<'a, Message, Theme, Renderer>>
    for Element<'a, Message, AnyTheme, Renderer>
where
    Message: 'a,
    Theme: theme::Base + 'a,
    AnyTheme: theme::Base,
    Renderer: 'a + crate::core::Renderer,
{
    fn from(themer: Themer<'a, Message, Theme, Renderer>) -> Self {
        Element::new(themer)
    }
}

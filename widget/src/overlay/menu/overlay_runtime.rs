use super::list::List;
use super::{Catalog, Menu};

use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::renderer;
use crate::core::semantics::{Actions, Node, Role, State as SemanticState};
use crate::core::text;
use crate::core::widget::tree::Tree;
use crate::core::{Event, Length, Point, Rectangle, Shell, Size, Vector, Widget};
use crate::scrollable::{self, Scrollable};

pub(super) struct Overlay<'a, 'b, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    position: Point,
    viewport: Rectangle,
    tree: &'a mut Tree,
    list: Scrollable<'a, Message, Theme, Renderer>,
    width: f32,
    target_height: f32,
    class: &'a <Theme as Catalog>::Class<'b>,
}

impl<'a, 'b, Message, Theme, Renderer> Overlay<'a, 'b, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: Catalog + scrollable::Catalog + 'a,
    Renderer: text::Renderer + 'a,
    'b: 'a,
{
    pub(super) fn new<T>(
        position: Point,
        viewport: Rectangle,
        menu: Menu<'a, 'b, T, Message, Theme, Renderer>,
        target_height: f32,
        menu_height: Length,
    ) -> Self
    where
        T: Clone,
    {
        let Menu {
            state,
            options,
            hovered_option,
            to_string,
            on_selected,
            on_option_hovered,
            width,
            padding,
            font,
            text_size,
            line_height,
            shaping,
            ellipsis,
            class,
        } = menu;

        let mut list = Scrollable::new(List {
            options,
            hovered_option,
            to_string,
            on_selected,
            on_option_hovered,
            font,
            text_size,
            line_height,
            shaping,
            ellipsis,
            padding,
            class,
        })
        .height(menu_height);

        state.tree.diff(&mut list as &mut dyn Widget<_, _, _>);

        Self {
            position,
            viewport,
            tree: &mut state.tree,
            list,
            width,
            target_height,
            class,
        }
    }
}

impl<Message, Theme, Renderer> crate::core::Overlay<Message, Theme, Renderer>
    for Overlay<'_, '_, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        let space_below = (bounds.height - (self.position.y + self.target_height)).max(0.0);
        let space_above = self.position.y.clamp(0.0, bounds.height.max(0.0));
        let available_height = space_below.max(space_above);
        let available_width = (bounds.width - self.position.x).max(0.0);
        let limits = layout::Limits::new(Size::ZERO, Size::new(available_width, available_height))
            .width(self.width.min(available_width));

        let node = self.list.layout(self.tree, renderer, &limits);
        let size = node.size();

        node.move_to(if space_below > space_above {
            self.position + Vector::new(0.0, self.target_height)
        } else {
            self.position - Vector::new(0.0, size.height)
        })
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        let bounds = layout.bounds();

        self.list
            .update(self.tree, event, layout, cursor, renderer, shell, &bounds);
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.list
            .mouse_interaction(self.tree, layout, cursor, &self.viewport, renderer)
    }

    fn semantics(&self, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        Some(
            Node::new(self.tree.id().clone(), layout.bounds(), Role::Group)
                .with_state(SemanticState::EXPANDED)
                .with_actions(Actions::SCROLL_INTO_VIEW),
        )
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        defaults: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let bounds = layout.bounds();
        let style = Catalog::style(theme, self.class);

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: style.border,
                shadow: style.shadow,
                ..renderer::Quad::default()
            },
            style.background,
        );

        self.list.draw(
            self.tree, renderer, theme, defaults, layout, cursor, &bounds,
        );
    }
}

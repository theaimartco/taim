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
        // Fit the menu's anchor horizontally into the actual window.
        // Reject non-finite placement before layout arithmetic so a popup
        // near the right edge does not vanish into a zero-width region.
        let position = crate::overlay::fit_menu_anchor(self.position, bounds, self.width);
        let viewport_height = if bounds.height.is_finite() {
            bounds.height.max(0.0)
        } else {
            0.0
        };
        let target_height = if self.target_height.is_finite() {
            self.target_height.clamp(0.0, viewport_height)
        } else {
            0.0
        };
        let space_below = (viewport_height - (position.y + target_height)).max(0.0);
        let space_above = position.y;
        let available_height = space_below.max(space_above);
        let viewport_width = if bounds.width.is_finite() {
            bounds.width.max(0.0)
        } else {
            0.0
        };
        let available_width = (viewport_width - position.x).max(0.0);
        let desired_width = if self.width.is_finite() {
            self.width.max(0.0)
        } else {
            0.0
        };
        let limits = layout::Limits::new(Size::ZERO, Size::new(available_width, available_height))
            .width(desired_width.min(available_width));

        let node = self.list.layout(self.tree, renderer, &limits);
        let size = node.size();

        node.move_to(if space_below > space_above {
            position + Vector::new(0.0, target_height)
        } else {
            position - Vector::new(0.0, size.height)
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

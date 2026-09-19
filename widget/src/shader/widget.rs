use super::{Action, Program, Shader, primitive};
use crate::core::event;
use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::renderer;
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::{self, Widget};
use crate::core::{Element, Event, Length, Rectangle, Shell, Size};

impl<P, Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Shader<Message, P>
where
    P: Program<Message>,
    Renderer: primitive::Renderer,
{
    fn tag(&self) -> tree::Tag {
        struct ShaderState<T>(T);
        tree::Tag::of::<ShaderState<P::State>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(P::State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::atomic(limits, self.width, self.height)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = tree.state.downcast_mut::<P::State>();

        if let Some(action) = self.program.update(state, event, bounds, cursor) {
            let (message, redraw, status) = action.into_inner();
            shell.request_redraw_at(redraw);

            if let Some(message) = message {
                shell.publish(message);
            }
            if status == event::Status::Captured {
                shell.capture_event();
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();

        if !bounds.intersects(viewport) {
            return mouse::Interaction::None;
        }

        self.program
            .mouse_interaction(tree.state.downcast_ref::<P::State>(), bounds, cursor)
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        // Custom programs may build expensive GPU primitives. Reject fully
        // clipped widgets before invoking user draw code or touching wgpu.
        if !bounds.intersects(viewport) {
            return;
        }

        let state = tree.state.downcast_ref::<P::State>();
        renderer.draw_primitive(bounds, self.program.draw(state, cursor, bounds));
    }
}

impl<'a, Message, Theme, Renderer, P> From<Shader<Message, P>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Renderer: primitive::Renderer,
    P: Program<Message> + 'a,
{
    fn from(shader: Shader<Message, P>) -> Self {
        Element::new(shader)
    }
}

impl<Message, P> Program<Message> for &P
where
    P: Program<Message>,
{
    type State = P::State;
    type Primitive = P::Primitive;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        P::update(self, state, event, bounds, cursor)
    }

    fn draw(
        &self,
        state: &Self::State,
        cursor: mouse::Cursor,
        bounds: Rectangle,
    ) -> Self::Primitive {
        P::draw(self, state, cursor, bounds)
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        P::mouse_interaction(self, state, bounds, cursor)
    }
}

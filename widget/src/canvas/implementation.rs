use crate::core::event;
use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::renderer;
use crate::core::widget::tree::{self, Tree};
use crate::core::window;
use crate::core::{Element, Length, Point, Rectangle, Shell, Size, Vector, Widget};
use crate::graphics::geometry;

use super::{Canvas, DrawContext, Event, Program, UpdateContext};
use std::cell::RefCell;

const MAX_RETAINED_LAYERS: usize = 128;

impl<P, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Canvas<P, Message, Theme, Renderer>
where
    Renderer: geometry::Renderer,
    Renderer::Geometry: 'static,
    P: Program<Message, Theme, Renderer>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<P::State, Renderer::Geometry>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::<P::State, Renderer::Geometry> {
            program: P::State::default(),
            interaction: None,
            layers: RefCell::new(Vec::new()),
        })
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
        let state = tree
            .state
            .downcast_mut::<State<P::State, Renderer::Geometry>>();

        if let Some(action) = self.program.update_with(
            &mut state.program,
            UpdateContext {
                event,
                bounds,
                cursor,
            },
        ) {
            let (message, redraw, status) = action.into_inner();
            shell.request_redraw_at(redraw);

            if let Some(message) = message {
                shell.publish(message);
            }
            if status == event::Status::Captured {
                shell.capture_event();
            }
        }

        if shell.redraw_request() != window::RedrawRequest::NextFrame {
            let interaction = self
                .program
                .mouse_interaction(&state.program, bounds, cursor);

            if state.interaction != Some(interaction) {
                state.interaction = Some(interaction);

                if !matches!(event, Event::Window(window::Event::RedrawRequested(_))) {
                    shell.request_redraw();
                }
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree
            .state
            .downcast_ref::<State<P::State, Renderer::Geometry>>();
        self.program
            .mouse_interaction(&state.program, layout.bounds(), cursor)
    }

    fn paint_bounds(&self, tree: &Tree, layout: Layout<'_>) -> Rectangle {
        let state = tree
            .state
            .downcast_ref::<State<P::State, Renderer::Geometry>>();
        self.program.paint_bounds(&state.program, layout.bounds())
    }

    fn hit_test(&self, tree: &Tree, layout: Layout<'_>, point: Point) -> bool {
        let state = tree
            .state
            .downcast_ref::<State<P::State, Renderer::Geometry>>();
        self.program
            .hit_test(&state.program, layout.bounds(), point)
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        let state = tree
            .state
            .downcast_ref::<State<P::State, Renderer::Geometry>>();
        self.program.semantics(&state.program, layout.bounds())
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        if bounds.width < 1.0 || bounds.height < 1.0 || !bounds.intersects(viewport) {
            return;
        }

        let state = tree
            .state
            .downcast_ref::<State<P::State, Renderer::Geometry>>();
        renderer.with_translation(Vector::new(bounds.x, bounds.y), |renderer| {
            let mut layers = state.layers.borrow_mut();
            debug_assert!(layers.is_empty());
            self.program.render(
                &state.program,
                DrawContext {
                    renderer,
                    theme,
                    bounds,
                    cursor,
                },
                &mut layers,
            );

            for layer in layers.drain(..) {
                renderer.draw_geometry(layer);
            }

            if layers.capacity() > MAX_RETAINED_LAYERS {
                layers.shrink_to(MAX_RETAINED_LAYERS);
            }
        });
    }
}

struct State<ProgramState, Geometry> {
    program: ProgramState,
    interaction: Option<mouse::Interaction>,
    layers: RefCell<Vec<Geometry>>,
}

impl<'a, P, Message, Theme, Renderer> From<Canvas<P, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: 'a + geometry::Renderer,
    Renderer::Geometry: 'static,
    P: 'a + Program<Message, Theme, Renderer>,
{
    fn from(canvas: Canvas<P, Message, Theme, Renderer>) -> Self {
        Element::new(canvas)
    }
}

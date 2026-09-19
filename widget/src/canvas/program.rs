use crate::Action;
use crate::canvas::mouse;
use crate::canvas::{Event, Geometry};
use crate::core::{Point, Rectangle};
use crate::graphics::geometry;

/// Borrowed input passed to context-based canvas updates.
#[derive(Debug, Clone, Copy)]
pub struct UpdateContext<'a> {
    /// Runtime event being processed.
    pub event: &'a Event,
    /// Logical canvas bounds.
    pub bounds: Rectangle,
    /// Pointer position and availability.
    pub cursor: mouse::Cursor,
}

/// Borrowed input passed to context-based canvas drawing.
#[derive(Debug, Clone, Copy)]
pub struct DrawContext<'a, Theme, Renderer> {
    /// Geometry renderer used to create frames and caches.
    pub renderer: &'a Renderer,
    /// Active application theme.
    pub theme: &'a Theme,
    /// Logical canvas bounds.
    pub bounds: Rectangle,
    /// Pointer position and availability.
    pub cursor: mouse::Cursor,
}

/// The state and logic of a [`Canvas`].
///
/// A [`Program`] can mutate internal state and produce messages for an
/// application.
///
/// [`Canvas`]: crate::Canvas
pub trait Program<Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: geometry::Renderer,
{
    /// The internal state mutated by the [`Program`].
    type State: Default + 'static;

    /// Updates the [`State`](Self::State) of the [`Program`].
    ///
    /// When a [`Program`] is used in a [`Canvas`], the runtime will call this
    /// method for each [`Event`].
    ///
    /// This method can optionally return an [`Action`] to either notify an
    /// application of any meaningful interactions, capture the event, or
    /// request a redraw.
    ///
    /// By default, this method does and returns nothing.
    ///
    /// [`Canvas`]: crate::Canvas
    fn update(
        &self,
        _state: &mut Self::State,
        _event: &Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        None
    }

    /// Context-based update entry point used by the TAIM canvas runtime.
    fn update_with(
        &self,
        state: &mut Self::State,
        context: UpdateContext<'_>,
    ) -> Option<Action<Message>> {
        self.update(state, context.event, context.bounds, context.cursor)
    }

    /// Draws the state of the [`Program`], producing a bunch of [`Geometry`].
    ///
    /// [`Geometry`] can be easily generated with a [`Frame`] or stored in a
    /// [`Cache`].
    ///
    /// [`Geometry`]: crate::canvas::Geometry
    /// [`Frame`]: crate::canvas::Frame
    /// [`Cache`]: crate::canvas::Cache
    fn draw(
        &self,
        _state: &Self::State,
        _renderer: &Renderer,
        _theme: &Theme,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry<Renderer>> {
        Vec::new()
    }

    /// Appends geometry to a caller-owned retained buffer.
    ///
    /// The default bridges existing [`Program::draw`] implementations. A
    /// high-frequency canvas can override this method and push directly into
    /// `output`, avoiding construction of a temporary `Vec` every frame.
    fn draw_into(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
        output: &mut Vec<Geometry<Renderer>>,
    ) {
        output.extend(self.draw(state, renderer, theme, bounds, cursor));
    }

    /// Context-based retained drawing entry point used by the runtime.
    fn render(
        &self,
        state: &Self::State,
        context: DrawContext<'_, Theme, Renderer>,
        output: &mut Vec<Geometry<Renderer>>,
    ) {
        self.draw_into(
            state,
            context.renderer,
            context.theme,
            context.bounds,
            context.cursor,
            output,
        );
    }

    /// Returns the current mouse interaction of the [`Program`].
    ///
    /// The interaction returned will be in effect even if the cursor position
    /// is out of bounds of the program's [`Canvas`].
    ///
    /// [`Canvas`]: crate::Canvas
    fn mouse_interaction(
        &self,
        _state: &Self::State,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        mouse::Interaction::default()
    }

    /// Returns the region that can be changed by the next draw.
    ///
    /// Programs drawing shadows or content outside the canvas rectangle can
    /// widen this region so damage-driven rendering remains correct.
    fn paint_bounds(&self, _state: &Self::State, bounds: Rectangle) -> Rectangle {
        bounds
    }

    /// Performs program-specific hit testing.
    fn hit_test(&self, _state: &Self::State, bounds: Rectangle, point: Point) -> bool {
        bounds.contains(point)
    }

    /// Describes this canvas to assistive technology.
    fn semantics(
        &self,
        _state: &Self::State,
        _bounds: Rectangle,
    ) -> Option<crate::core::semantics::Node> {
        None
    }
}

impl<Message, Theme, Renderer, T> Program<Message, Theme, Renderer> for &T
where
    Renderer: geometry::Renderer,
    T: Program<Message, Theme, Renderer>,
{
    type State = T::State;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        T::update(self, state, event, bounds, cursor)
    }

    fn update_with(
        &self,
        state: &mut Self::State,
        context: UpdateContext<'_>,
    ) -> Option<Action<Message>> {
        T::update_with(self, state, context)
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry<Renderer>> {
        T::draw(self, state, renderer, theme, bounds, cursor)
    }

    fn draw_into(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
        output: &mut Vec<Geometry<Renderer>>,
    ) {
        T::draw_into(self, state, renderer, theme, bounds, cursor, output);
    }

    fn render(
        &self,
        state: &Self::State,
        context: DrawContext<'_, Theme, Renderer>,
        output: &mut Vec<Geometry<Renderer>>,
    ) {
        T::render(self, state, context, output);
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        T::mouse_interaction(self, state, bounds, cursor)
    }

    fn paint_bounds(&self, state: &Self::State, bounds: Rectangle) -> Rectangle {
        T::paint_bounds(self, state, bounds)
    }

    fn hit_test(&self, state: &Self::State, bounds: Rectangle, point: Point) -> bool {
        T::hit_test(self, state, bounds, point)
    }

    fn semantics(
        &self,
        state: &Self::State,
        bounds: Rectangle,
    ) -> Option<crate::core::semantics::Node> {
        T::semantics(self, state, bounds)
    }
}

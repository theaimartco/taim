//! What a view can see and ask for in each pass.
use crate::{Background, Border, Color, Point, Rectangle, Renderer, Theme, mouse};
use taim_core::Renderer as _;
use taim_core::renderer::Quad;

/// Available while measuring.
pub struct MeasureCx<'a> {
    pub(crate) renderer: &'a Renderer,
    pub(crate) theme: &'a Theme,
}

impl<'a> MeasureCx<'a> {
    /// The renderer, for measuring text.
    pub fn renderer(&self) -> &'a Renderer {
        self.renderer
    }

    /// The active theme.
    pub fn theme(&self) -> &'a Theme {
        self.theme
    }
}

/// Available while painting.
pub struct PaintCx<'a> {
    pub(crate) renderer: &'a mut Renderer,
    pub(crate) theme: &'a Theme,
    pub(crate) bounds: Rectangle,
    pub(crate) pointer: Option<Point>,
    pub(crate) text_color: Color,
}

impl PaintCx<'_> {
    /// Where this view is, in window coordinates.
    pub fn bounds(&self) -> Rectangle {
        self.bounds
    }

    /// Whether the pointer is over this view.
    pub fn is_hovered(&self) -> bool {
        self.pointer.is_some_and(|at| self.bounds.contains(at))
    }

    /// The active theme.
    pub fn theme(&self) -> &Theme {
        self.theme
    }

    /// Text color where a view does not choose one.
    pub fn text_color(&self) -> Color {
        self.text_color
    }

    /// Fills `area` (window coordinates) with `background` inside `border`.
    pub fn fill(&mut self, area: Rectangle, background: impl Into<Background>, border: Border) {
        self.renderer.fill_quad(
            Quad {
                bounds: area,
                border,
                ..Quad::default()
            },
            background,
        );
    }

    /// The renderer, for drawing primitives the context does not wrap.
    pub fn renderer(&mut self) -> &mut Renderer {
        self.renderer
    }
}

/// Available while handling an event.
pub struct EventCx<'a> {
    pub(crate) bounds: Rectangle,
    pub(crate) pointer: Option<Point>,
    pub(crate) requests: &'a mut Requests,
}

/// What views asked for while handling events.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Requests {
    pub paint: bool,
    pub layout: bool,
}

impl EventCx<'_> {
    /// Where this view is, in window coordinates.
    pub fn bounds(&self) -> Rectangle {
        self.bounds
    }

    /// The pointer position in window coordinates, if known.
    pub fn pointer(&self) -> Option<Point> {
        self.pointer
    }

    /// Whether the pointer is over this view.
    pub fn is_hovered(&self) -> bool {
        self.pointer.is_some_and(|at| self.bounds.contains(at))
    }

    /// The view's appearance changed; paint the next frame.
    pub fn request_paint(&mut self) {
        self.requests.paint = true;
    }

    /// The view's size may have changed; measure again.
    pub fn request_layout(&mut self) {
        self.requests.layout = true;
    }
}

/// Available while describing a view to assistive technology.
pub struct DescribeCx<'a> {
    pub(crate) bounds: Rectangle,
    pub(crate) _marker: std::marker::PhantomData<&'a ()>,
}

impl DescribeCx<'_> {
    /// Where this view is, in window coordinates.
    pub fn bounds(&self) -> Rectangle {
        self.bounds
    }
}

/// The cursor a view shows when nothing more specific applies.
pub(crate) const DEFAULT_CURSOR: mouse::Interaction = mouse::Interaction::None;

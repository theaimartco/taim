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
    pub(crate) focused: bool,
    pub(crate) now: crate::Instant,
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

    /// Whether this view has keyboard focus.
    pub fn has_focus(&self) -> bool {
        self.focused
    }

    /// The time this frame is painted for; animations sample at it.
    pub fn now(&self) -> crate::Instant {
        self.now
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

    /// Like [`fill`](Self::fill), with a drop shadow under the shape.
    pub fn fill_shadowed(
        &mut self,
        area: Rectangle,
        background: impl Into<Background>,
        border: Border,
        shadow: crate::Shadow,
    ) {
        self.renderer.fill_quad(
            Quad {
                bounds: area,
                border,
                shadow,
                ..Quad::default()
            },
            background,
        );
    }

    /// Fills `area` with a surface of `material`: glass that refracts and
    /// frosts what was painted before it when the renderer can, and an
    /// opaque `background` when it cannot. Glass must be painted in a layer
    /// of its own ([`View::is_layer`](crate::View::is_layer)) so there is a
    /// finished backdrop to look through.
    pub fn fill_material(
        &mut self,
        area: Rectangle,
        background: impl Into<Background>,
        border: Border,
        shadow: crate::Shadow,
        material: taim_core::material::Material,
    ) {
        self.renderer.fill_quad(
            Quad {
                bounds: area,
                border,
                shadow,
                material,
                ..Quad::default()
            },
            background,
        );
    }

    /// Like [`fill_material`](Self::fill_material), for liquid glass: a
    /// press lights and magnifies it, and it melts into the neighbours
    /// `liquid` lists. Without the liquid pass this is plain glass.
    pub fn fill_liquid(
        &mut self,
        area: Rectangle,
        tint: Color,
        border: Border,
        shadow: crate::Shadow,
        material: taim_core::material::Material,
        liquid: taim_core::material::Liquid,
    ) {
        self.renderer.fill_quad(
            Quad {
                bounds: area,
                border,
                shadow,
                material,
                liquid: Some(liquid),
                snap: false,
                ..Quad::default()
            },
            tint,
        );
    }

    /// Text color for secondary information: the text color faded toward
    /// the window's background as far as it can go while still reading at
    /// WCAG AA contrast (4.5:1) on it.
    pub fn muted(&self) -> Color {
        let background = self.theme.palette().background.base.color;
        crate::views::look::quiet(
            self.text_color,
            background,
            crate::views::look::TEXT_CONTRAST,
        )
    }

    /// Secondary text on `surface` (a field, a segment track), as quiet as
    /// WCAG AA allows on that surface.
    pub fn muted_on(&self, surface: Color) -> Color {
        crate::views::look::quiet(self.text_color, surface, crate::views::look::TEXT_CONTRAST)
    }

    /// An icon or other graphic that is not the focus on `surface`: as
    /// quiet as WCAG's 3:1 for non-text content allows.
    pub fn quiet_icon_on(&self, surface: Color) -> Color {
        crate::views::look::quiet(self.text_color, surface, crate::views::look::ICON_CONTRAST)
    }

    /// The renderer, for drawing primitives the context does not wrap.
    pub fn renderer(&mut self) -> &mut Renderer {
        self.renderer
    }
}

/// Available while handling an event.
pub struct EventCx<'a> {
    pub(crate) renderer: Option<&'a Renderer>,
    pub(crate) now: crate::Instant,
    pub(crate) bounds: Rectangle,
    pub(crate) pointer: Option<Point>,
    pub(crate) requests: &'a mut Requests,
    pub(crate) focus: &'a mut Focus,
    pub(crate) path: &'a [u16],
    pub(crate) touch: bool,
}

/// What views asked for while handling events.
#[derive(Debug, Default, Clone)]
pub(crate) struct Requests {
    pub paint: bool,
    pub layout: bool,
    pub animate: bool,
    pub copy: Option<String>,
    pub paste: bool,
}

/// Which view has keyboard focus, as a path of child indices from the root.
#[derive(Debug, Default)]
pub(crate) struct Focus {
    pub current: Option<Vec<u16>>,
    /// Set when a view took focus during the current dispatch.
    pub taken: bool,
}

impl EventCx<'_> {
    /// The active renderer when dispatching through a platform shell.
    /// Pure routing tests using `Root::handle` do not supply a renderer.
    pub fn renderer(&self) -> Option<&Renderer> {
        self.renderer
    }
    /// The time of this event; start animations from it.
    pub fn now(&self) -> crate::Instant {
        self.now
    }

    /// Where this view is, in window coordinates.
    pub fn bounds(&self) -> Rectangle {
        self.bounds
    }

    /// The pointer position in window coordinates, if known.
    pub fn pointer(&self) -> Option<Point> {
        self.pointer
    }

    /// Whether the pointer is a finger rather than a mouse or pen.
    ///
    /// The shell turns the first finger on the screen into pointer events,
    /// so every view works with touch; this tells the views that behave
    /// differently for touch (dragging to scroll, say) which one it is.
    pub fn is_touch(&self) -> bool {
        self.touch
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

    /// Keep painting frames while an animation runs.
    pub fn request_animation(&mut self) {
        self.requests.animate = true;
        self.requests.paint = true;
    }

    /// Whether this view has keyboard focus.
    pub fn has_focus(&self) -> bool {
        self.focus.current.as_deref() == Some(self.path)
    }

    /// Takes keyboard focus.
    pub fn request_focus(&mut self) {
        if !self.has_focus() {
            self.requests.paint = true;
        }
        self.focus.current = Some(self.path.to_vec());
        self.focus.taken = true;
    }

    /// Gives keyboard focus up, if this view has it.
    pub fn release_focus(&mut self) {
        if self.has_focus() {
            self.focus.current = None;
            self.requests.paint = true;
        }
    }

    /// Puts `text` on the clipboard.
    pub fn copy(&mut self, text: impl Into<String>) {
        self.requests.copy = Some(text.into());
    }

    /// Asks for the clipboard text; it arrives as a clipboard event.
    pub fn paste(&mut self) {
        self.requests.paste = true;
    }
}

/// Available while describing a view to assistive technology.
pub struct DescribeCx<'a> {
    pub(crate) bounds: Rectangle,
    pub(crate) focused: bool,
    pub(crate) _marker: std::marker::PhantomData<&'a ()>,
}

impl DescribeCx<'_> {
    /// Where this view is, in window coordinates.
    pub fn bounds(&self) -> Rectangle {
        self.bounds
    }

    /// Whether this view has keyboard focus.
    pub fn has_focus(&self) -> bool {
        self.focused
    }
}

/// What [`View::visual`](crate::View::visual) is told: the time and where
/// layout put the view.
#[derive(Debug, Clone, Copy)]
pub struct VisualCx {
    pub(crate) now: crate::Instant,
    pub(crate) frame: Rectangle,
    pub(crate) bounds: Rectangle,
}

impl VisualCx {
    /// A context for drawing at `now`, with the view laid out at `frame`
    /// in its parent and at `bounds` in the window. For calling a view's
    /// [`visual`](crate::View::visual) directly, as tests do.
    pub fn new(now: crate::Instant, frame: Rectangle, bounds: Rectangle) -> Self {
        Self { now, frame, bounds }
    }

    /// The frame time.
    pub fn now(&self) -> crate::Instant {
        self.now
    }

    /// Where layout put the view, relative to its parent: it changes when
    /// layout moves the view, not when an ancestor scrolls.
    pub fn frame(&self) -> Rectangle {
        self.frame
    }

    /// Where layout put the view in window coordinates, before any
    /// ancestor's [`Visual`](crate::Visual) moves it.
    pub fn bounds(&self) -> Rectangle {
        self.bounds
    }
}

/// The cursor a view shows when nothing more specific applies.
pub(crate) const DEFAULT_CURSOR: mouse::Interaction = mouse::Interaction::None;

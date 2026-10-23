//! The view contract.
use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::{Constraints, Sizing};
use crate::{Event, Instant, Point, Rectangle, Size, Vector, mouse, semantics};

/// A piece of user interface.
///
/// A view measures itself, paints itself and reacts to input. It never
/// paints or routes input to its children: it exposes them through
/// [`children`](Self::children) and the runtime walks the tree, using the
/// position each child was given during [`measure`](Self::measure).
///
/// Views own their state. The runtime keeps the tree alive between frames
/// and asks again only when a signal a view depends on changes.
pub trait View: 'static {
    /// How the view wants to be sized; parents use this to share space.
    fn sizing(&self) -> Sizing {
        Sizing::HUG
    }

    /// Measures the view within `constraints`, measuring and placing its
    /// children on the way, and returns its size.
    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size;

    /// Paints the view itself. Children are painted afterwards, on top.
    fn paint(&self, cx: &mut PaintCx<'_>);

    /// Paints on top of the children (scrollbars, overlays, focus rings
    /// around containers). Not clipped by the view's own clipping.
    fn paint_over(&self, _cx: &mut PaintCx<'_>) {}

    /// Reacts to `event`. Children see every event first; a view only sees
    /// events none of its children handled.
    fn event(&mut self, _cx: &mut EventCx<'_>, _event: &Event) -> Handled {
        Handled::No
    }

    /// Describes the view to assistive technology.
    ///
    /// There is deliberately no default: every view must decide whether it
    /// is perceivable. Purely decorative views return `None`.
    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node>;

    /// The cursor to show while the pointer is over this view, if it cares.
    fn cursor(&self) -> Option<mouse::Interaction> {
        None
    }

    /// Whether the view is mid-animation at `now` and needs another frame.
    fn is_animating(&self, _now: Instant) -> bool {
        false
    }

    /// Whether the view can take keyboard focus (and be reached with Tab).
    fn focusable(&self) -> bool {
        false
    }

    /// Whether the view and its subtree are drawn in a layer of their own,
    /// above everything painted before them. Overlays (drawers, sheets,
    /// notices) need this: within one layer the renderer draws all shapes
    /// before all text, so earlier text would show through.
    fn is_layer(&self) -> bool {
        false
    }

    /// The text color this view's subtree inherits, if it sets one (a
    /// solid button, say, wants text readable on its fill).
    fn foreground(&self, _theme: &crate::Theme) -> Option<crate::Color> {
        None
    }

    /// Whether children are clipped to this view's bounds.
    fn clips(&self) -> bool {
        false
    }

    /// How far the children are scrolled: they are drawn and hit-tested
    /// shifted by minus this amount.
    fn scroll_offset(&self) -> Vector {
        Vector::ZERO
    }

    /// Hands each child to `visit`, in paint order.
    fn children(&mut self, _visit: &mut dyn FnMut(&mut Child)) {}

    /// The view's type, for diagnostics and developer tools.
    fn debug_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Whether a view consumed an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handled {
    /// Consumed; ancestors do not see it.
    Yes,
    /// Not consumed; it continues to the parent.
    No,
}

impl Handled {
    /// `Yes` when `consumed`.
    pub fn from(consumed: bool) -> Self {
        if consumed { Self::Yes } else { Self::No }
    }
}

/// Any [`View`], boxed.
pub struct AnyView(Box<dyn View>);

impl AnyView {
    /// Boxes `view`.
    pub fn new(view: impl View) -> Self {
        Self(Box::new(view))
    }

    /// Converts anything view-like (a view, or an `AnyView`).
    pub fn of(view: impl Into<AnyView>) -> Self {
        view.into()
    }

    /// The boxed view.
    pub fn as_view(&self) -> &dyn View {
        &*self.0
    }

    /// The boxed view, mutably.
    pub fn as_view_mut(&mut self) -> &mut dyn View {
        &mut *self.0
    }
}

impl std::fmt::Debug for AnyView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AnyView")
    }
}

impl<V: View> From<V> for AnyView {
    fn from(view: V) -> Self {
        Self::new(view)
    }
}

/// A view as held by its parent: the view plus where the parent put it.
#[derive(Debug)]
pub struct Child {
    view: AnyView,
    /// Position relative to the parent's top-left corner, and measured size.
    frame: Rectangle,
}

impl Child {
    /// Wraps `view`, not yet measured.
    pub fn new(view: impl Into<AnyView>) -> Self {
        Self {
            view: view.into(),
            frame: Rectangle::default(),
        }
    }

    /// The child's sizing preference.
    pub fn sizing(&self) -> Sizing {
        self.view.as_view().sizing()
    }

    /// Measures the child within `constraints` and remembers its size.
    pub fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let size = constraints.clamp(self.view.as_view_mut().measure(cx, constraints));
        self.frame.width = size.width;
        self.frame.height = size.height;
        size
    }

    /// Puts the child at `origin`, relative to the parent.
    pub fn place(&mut self, origin: Point) {
        self.frame.x = origin.x;
        self.frame.y = origin.y;
    }

    /// Where the child is, relative to its parent.
    pub fn frame(&self) -> Rectangle {
        self.frame
    }

    #[cfg(test)]
    pub(crate) fn frame_mut(&mut self) -> &mut Rectangle {
        &mut self.frame
    }

    /// The measured size.
    pub fn size(&self) -> Size {
        self.frame.size()
    }

    /// The child view.
    pub fn view(&self) -> &dyn View {
        self.view.as_view()
    }

    /// The child view, mutably.
    pub fn view_mut(&mut self) -> &mut dyn View {
        self.view.as_view_mut()
    }
}

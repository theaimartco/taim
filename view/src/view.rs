//! The view contract.
use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::{Constraints, Sizing};
use crate::{Event, Instant, Point, Rectangle, Size, Vector, mouse, semantics};

/// How a view is drawn this frame, on top of where layout put it: moved,
/// scaled about its centre and faded. It changes only the drawing, never
/// layout or where input lands, so it is free to animate every frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Visual {
    /// How far it is drawn from its laid-out position.
    pub offset: Vector,
    /// Its scale about its centre; `1.0` is its own size.
    pub scale: f32,
    /// Its opacity, with everything inside it: `1.0` is opaque.
    pub opacity: f32,
}

impl Visual {
    /// Drawn exactly where and as layout put it.
    pub const IDENTITY: Self = Self {
        offset: Vector::ZERO,
        scale: 1.0,
        opacity: 1.0,
    };

    /// Whether this changes nothing.
    pub fn is_identity(&self) -> bool {
        *self == Self::IDENTITY
    }

    /// `self` applied inside `outer`.
    #[must_use]
    pub fn then(self, outer: Self) -> Self {
        Self {
            offset: self.offset + outer.offset,
            scale: self.scale * outer.scale,
            opacity: self.opacity * outer.opacity,
        }
    }
}

impl Default for Visual {
    fn default() -> Self {
        Self::IDENTITY
    }
}

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
    /// Optional Android hierarchy slot. The native child owns its own drawing, input and accessibility.
    fn native_view(&self) -> Option<crate::NativeViewDescriptor> {
        None
    }
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
    /// events none of its children handled. Focus loss and touch cancellation
    /// are lifecycle notifications and always reach every view, even if captured.
    fn event(&mut self, _cx: &mut EventCx<'_>, _event: &Event) -> Handled {
        Handled::No
    }

    /// Looks at `event` before any child does, and may take it.
    ///
    /// This is how a scrollable area claims a drag that began on a button:
    /// it lets the press through, watches the finger move, and once the
    /// movement is clearly a scroll returns [`Handled::Yes`]. The children
    /// that saw the press then receive a cancellation (the pointer leaves
    /// and the button is released with no pointer), so nothing activates.
    /// Android calls this `onInterceptTouchEvent`.
    fn intercept(&mut self, _cx: &mut EventCx<'_>, _event: &Event) -> Handled {
        Handled::No
    }

    /// Cancels temporary interactions when this view is deactivated.
    /// This must be idempotent and must not activate controls or discard model data.
    fn cancel_interaction(&mut self) {}

    /// Performs a validated accessibility request on the window thread.
    /// The runtime checks the live semantic identity, enabled state and
    /// advertised actions before calling this method.
    fn accessibility_action(
        &mut self,
        cx: &mut EventCx<'_>,
        request: &semantics::Request,
    ) -> Handled {
        if *request == semantics::Request::Focus && self.focusable() {
            cx.request_focus();
            Handled::Yes
        } else {
            Handled::No
        }
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

    /// A one-shot redraw deadline, for caret blinks and other idle timers.
    /// Returning a deadline avoids keeping a continuous animation active.
    fn redraw_at(&self) -> Option<Instant> {
        None
    }

    /// Whether the view can take keyboard focus (and be reached with Tab).
    fn focusable(&self) -> bool {
        false
    }

    /// Whether the view edits text while it has focus. The shell shows the
    /// on-screen keyboard (and enables the input method) while it does.
    fn takes_text(&self) -> bool {
        false
    }

    /// The platform text input request when this view has keyboard focus.
    /// `bounds` is in window logical coordinates, including ancestor scroll.
    fn input_method(&self, _bounds: Rectangle) -> taim_core::input_method::InputMethod {
        taim_core::input_method::InputMethod::Disabled
    }

    /// Native input-connection context. Secure controls leave this unavailable
    /// so their contents and length are never exported to platform editors.
    fn editor_state(&self) -> Option<taim_core::input_method::EditorState> {
        None
    }

    /// Whether this subtree currently owns modal keyboard and accessibility focus.
    /// Background controls are excluded until the scope closes; prior focus is restored.
    fn is_modal(&self) -> bool {
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

    /// How the view, with everything inside it, is drawn: moved, scaled or
    /// faded from where layout put it (see [`VisualCx`](crate::VisualCx)).
    /// Return a changing value only while
    /// [`is_animating`](Self::is_animating) says so. Only drawing changes:
    /// pointer input still finds the view at its laid-out place.
    fn visual(&self, _cx: &crate::VisualCx) -> Visual {
        Visual::IDENTITY
    }

    /// Whether this view's [`visual`](Self::visual) opacity replaces the
    /// opacity it inherits instead of multiplying it: a shared element
    /// flying between fading pages.
    #[doc(hidden)]
    fn resets_opacity(&self) -> bool {
        false
    }

    /// This view as a [`glass`](crate::views::glass) pane, so a
    /// [`glass_group`](crate::views::glass_group) can find the panes it
    /// draws. Only glass answers.
    #[doc(hidden)]
    fn as_glass(&mut self) -> Option<&mut crate::views::Glass> {
        None
    }

    /// Hands each child to `visit` in reverse paint order: topmost first,
    /// the order input is offered in.
    ///
    /// The default walks [`View::children`] once per child, which is fine
    /// for a handful of children. Views that hold many (stacks, lists,
    /// grids) override it so routing an event stays linear in the number
    /// of children.
    fn children_rev(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        let mut count = 0_usize;
        self.children(&mut |_| count += 1);

        for target in (0..count).rev() {
            let mut index = 0_usize;
            self.children(&mut |child| {
                if index == target {
                    visit(child);
                }
                index += 1;
            });
        }
    }

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

/// A view as held by its parent: its stable identity and assigned frame.
/// Moving a retained child preserves focus; replacing it creates a new identity.
#[derive(Debug)]
pub struct Child {
    pub(crate) identity: taim_core::widget::Id,
    active: std::cell::Cell<bool>,
    view: AnyView,
    /// Position relative to the parent's top-left corner, and measured size.
    frame: Rectangle,
}

impl Child {
    /// Wraps `view`, not yet measured.
    pub fn new(view: impl Into<AnyView>) -> Self {
        Self {
            identity: taim_core::widget::Id::unique(),
            active: std::cell::Cell::new(false),
            view: view.into(),
            frame: Rectangle::default(),
        }
    }

    pub(crate) fn activate(&self) {
        self.active.set(true);
    }

    /// Cancels interactions once when an owned subtree is omitted from `View::children`.
    /// Custom conditional containers should call this when hiding a retained child.
    pub fn deactivate(&mut self) {
        if self.active.replace(false) {
            self.view.as_view_mut().cancel_interaction();
            self.view
                .as_view_mut()
                .children(&mut |child| child.deactivate());
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    struct Probe(Rc<Cell<usize>>);
    impl View for Probe {
        fn measure(&mut self, _: &mut MeasureCx<'_>, _: Constraints) -> Size {
            Size::ZERO
        }
        fn paint(&self, _: &mut PaintCx<'_>) {}
        fn describe(&self, _: &DescribeCx<'_>) -> Option<semantics::Node> {
            None
        }
        fn cancel_interaction(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn a_retained_child_is_cancelled_once_per_deactivation() {
        let calls = Rc::new(Cell::new(0));
        let mut child = Child::new(Probe(calls.clone()));
        child.activate();
        child.deactivate();
        child.deactivate();
        assert_eq!(calls.get(), 1);
        child.activate();
        child.deactivate();
        assert_eq!(calls.get(), 2);
    }
}

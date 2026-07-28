//! Choose different content depending on how much space is available.
//!
//! [`Adaptive`] classifies the width it is given into one of three
//! [`WindowClass`]es (compact/medium/expanded, the same three-bucket
//! split most adaptive-layout guidance converges on) and builds only the
//! [`Element`] for the class that actually applies -- the same
//! build-only-what-is-needed discipline [`crate::VirtualList`] applies
//! to rows, applied here to layout variants instead.
use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::renderer;
use crate::core::widget::visit;
use crate::core::widget::{Operation, Tree};
use crate::core::{Element, Event, Length, Rectangle, Shell, Size, Widget};

/// A width-based size class, used by [`Adaptive`] to decide which
/// content to build.
///
/// The default breakpoints (600 and 840 logical pixels) match the
/// widely-used compact/medium/expanded split for phones, tablets, and
/// desktop windows respectively; [`Adaptive::breakpoints`] can override
/// them for content with different natural thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowClass {
    /// Narrow: a typical phone in portrait, or a narrow split-screen
    /// pane.
    Compact,
    /// Intermediate: a tablet, a phone in landscape, a half-width
    /// desktop window.
    Medium,
    /// Wide: a full desktop window or a large tablet.
    Expanded,
}

/// The width thresholds an [`Adaptive`] widget uses to classify its
/// available width into a [`WindowClass`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Breakpoints {
    /// The width at or above which the class becomes
    /// [`WindowClass::Medium`] (below it, [`WindowClass::Compact`]).
    pub medium: f32,
    /// The width at or above which the class becomes
    /// [`WindowClass::Expanded`].
    pub expanded: f32,
}

impl Breakpoints {
    /// The default breakpoints: `600` and `840` logical pixels.
    pub const DEFAULT: Self = Self {
        medium: 600.0,
        expanded: 840.0,
    };

    /// Classifies `width` into a [`WindowClass`] using these
    /// [`Breakpoints`].
    pub fn classify(self, width: f32) -> WindowClass {
        if width >= self.expanded {
            WindowClass::Expanded
        } else if width >= self.medium {
            WindowClass::Medium
        } else {
            WindowClass::Compact
        }
    }
}

impl Default for Breakpoints {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// A widget that builds different content depending on the
/// [`WindowClass`] of the width it is given.
pub struct Adaptive<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
{
    breakpoints: Breakpoints,
    builder: Box<dyn Fn(WindowClass) -> Element<'a, Message, Theme, Renderer> + 'a>,
    width: Length,
    height: Length,
}

impl<'a, Message, Theme, Renderer> Adaptive<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    /// Creates a new [`Adaptive`] widget that calls `builder` with the
    /// [`WindowClass`] matching whatever width it is eventually laid out
    /// with.
    pub fn new(
        builder: impl Fn(WindowClass) -> Element<'a, Message, Theme, Renderer> + 'a,
    ) -> Self {
        Self {
            breakpoints: Breakpoints::default(),
            builder: Box::new(builder),
            width: Length::Fill,
            height: Length::Shrink,
        }
    }

    /// Sets the [`Breakpoints`] used to classify the available width.
    pub fn breakpoints(mut self, breakpoints: Breakpoints) -> Self {
        self.breakpoints = breakpoints;
        self
    }

    /// Sets the width of the [`Adaptive`] widget.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`Adaptive`] widget.
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }
}

/// The internal state of an [`Adaptive`] widget: the [`WindowClass`] its
/// currently-built content matches, and that content's own [`Tree`].
#[derive(Default)]
struct State {
    class: Option<WindowClass>,
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Adaptive<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    fn tag(&self) -> crate::core::widget::tree::Tag {
        crate::core::widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> crate::core::widget::tree::State {
        crate::core::widget::tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn diff(&mut self, tree: &mut Tree) {
        // The concrete child depends on a width this method is not
        // given, so re-diffing (and, if the class changed, rebuilding
        // from scratch) happens in `layout`, the first point an actual
        // width is known. An empty `tree.children` here is corrected by
        // `layout` on the very next call, exactly like a freshly
        // created `Tree` would be.
        if tree.children.is_empty() {
            tree.children.push(Tree::empty());
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let limits = limits.width(self.width).height(self.height);
        let available_width = limits.max().width;

        let class = self.breakpoints.classify(available_width);

        let state = tree.state.downcast_mut::<State>();

        let mut content = (self.builder)(class);

        if state.class != Some(class) {
            tree.children[0] = Tree::new(content.as_widget());
            state.class = Some(class);
        } else {
            tree.children[0].diff(content.as_widget_mut());
        }

        layout::contained(&limits, self.width, self.height, |limits| {
            content
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, limits)
        })
    }

    fn visit_children(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        let state = tree.state.downcast_ref::<State>();

        let Some(class) = state.class else {
            return;
        };

        let mut content = (self.builder)(class);

        visit(&mut visit::Widgets::new(
            content.as_widget_mut(),
            &mut tree.children[0],
            layout.children().next().unwrap(),
        ));
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_ref::<State>();

        let Some(class) = state.class else {
            return;
        };

        let mut content = (self.builder)(class);

        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            content.as_widget_mut().operate(
                &mut tree.children[0],
                layout.children().next().unwrap(),
                renderer,
                operation,
            );
        });
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
        let state = tree.state.downcast_ref::<State>();

        let Some(class) = state.class else {
            return;
        };

        let mut content = (self.builder)(class);

        content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout.children().next().unwrap(),
            cursor,
            renderer,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State>();

        let Some(class) = state.class else {
            return mouse::Interaction::None;
        };

        let content = (self.builder)(class);

        content.as_widget().mouse_interaction(
            &tree.children[0],
            layout.children().next().unwrap(),
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();

        let Some(class) = state.class else {
            return;
        };

        let content = (self.builder)(class);

        content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout.children().next().unwrap(),
            cursor,
            viewport,
        );
    }

    // `Widget::overlay` is intentionally left at its default (`None`),
    // for the same reason documented on `VirtualList::overlay`: content
    // built fresh from `self.builder` on every call has no persisted
    // `Element` an overlay could soundly borrow from.
}

impl<'a, Message, Theme, Renderer> From<Adaptive<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: crate::core::Renderer + 'a,
{
    fn from(adaptive: Adaptive<'a, Message, Theme, Renderer>) -> Self {
        Self::new(adaptive)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_below_medium_breakpoint() {
        let breakpoints = Breakpoints::default();

        assert_eq!(breakpoints.classify(0.0), WindowClass::Compact);
        assert_eq!(breakpoints.classify(599.0), WindowClass::Compact);
    }

    #[test]
    fn medium_between_breakpoints() {
        let breakpoints = Breakpoints::default();

        assert_eq!(breakpoints.classify(600.0), WindowClass::Medium);
        assert_eq!(breakpoints.classify(839.0), WindowClass::Medium);
    }

    #[test]
    fn expanded_at_or_above_expanded_breakpoint() {
        let breakpoints = Breakpoints::default();

        assert_eq!(breakpoints.classify(840.0), WindowClass::Expanded);
        assert_eq!(breakpoints.classify(2000.0), WindowClass::Expanded);
    }

    #[test]
    fn custom_breakpoints_are_respected() {
        let breakpoints = Breakpoints {
            medium: 100.0,
            expanded: 200.0,
        };

        assert_eq!(breakpoints.classify(50.0), WindowClass::Compact);
        assert_eq!(breakpoints.classify(150.0), WindowClass::Medium);
        assert_eq!(breakpoints.classify(250.0), WindowClass::Expanded);
    }
}

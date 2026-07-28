//! Display interactive elements on top of other widgets.
mod element;
mod group;
mod nested;

pub use element::Element;
pub use group::Group;
pub use nested::Nested;

use crate::layout;
use crate::mouse;
use crate::renderer;
use crate::widget;
use crate::widget::Tree;
use crate::{Event, Layout, Rectangle, Shell, Size, Vector};

/// An interactive component that can be displayed on top of other widgets.
pub trait Overlay<Message, Theme, Renderer>
where
    Renderer: crate::Renderer,
{
    /// Returns the layout [`Node`] of the [`Overlay`].
    ///
    /// This [`Node`] is used by the runtime to compute the [`Layout`] of the
    /// user interface.
    ///
    /// [`Node`]: layout::Node
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node;

    /// Draws the [`Overlay`] using the associated `Renderer`.
    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    );

    /// Returns the [`semantics::Node`] describing this [`Overlay`] to
    /// assistive technology, if it has one.
    ///
    /// # Why an overlay needs this at all
    ///
    /// An overlay is not decoration: a dropdown, a menu, or a sheet is
    /// often the *only* thing the user can currently interact with. Before
    /// this method existed, every overlay was invisible to the semantic
    /// tree -- `aura_runtime::semantics::build` walks widgets, and an
    /// overlay is reached through [`crate::Widget::overlay`] rather than
    /// through the widget tree, so nothing ever asked it.
    ///
    /// The practical effect was that an open `pick_list` announced
    /// nothing: a screen-reader user could hear the closed control, open
    /// it, and then find the entire list of choices unreachable.
    ///
    /// # Contract
    ///
    /// Like [`crate::Widget::semantics`], this is **non-recursive**: it
    /// describes this overlay only. Composing a subtree is the runtime's
    /// job. `None` (the default) means "no semantics of my own", which is
    /// correct for a purely presentational overlay; it does not stop the
    /// runtime from reaching nested overlays via [`Overlay::overlay`].
    ///
    /// An overlay reporting a node must give it a stable
    /// [`semantics::Node::id`], for the same reason a widget must: an
    /// identity regenerated per frame makes the node look new on every
    /// repaint. An overlay produced from a widget should derive its
    /// identity from that widget's [`Tree::id`](widget::tree::Tree::id).
    fn semantics(&self, _layout: Layout<'_>) -> Option<crate::semantics::Node> {
        None
    }

    /// Applies a [`widget::Operation`] to the [`Overlay`].
    fn operate(
        &mut self,
        _layout: Layout<'_>,
        _renderer: &Renderer,
        _operation: &mut dyn widget::Operation,
    ) {
    }

    /// Processes a runtime [`Event`].
    ///
    /// By default, it does nothing.
    fn update(
        &mut self,
        _event: &Event,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        _shell: &mut Shell<'_, Message>,
    ) {
    }

    /// Returns the current [`mouse::Interaction`] of the [`Overlay`].
    ///
    /// By default, it returns [`mouse::Interaction::None`].
    fn mouse_interaction(
        &self,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        mouse::Interaction::None
    }

    /// Returns the nested overlay of the [`Overlay`], if there is any.
    fn overlay<'a>(
        &'a mut self,
        _layout: Layout<'a>,
        _renderer: &Renderer,
    ) -> Option<Element<'a, Message, Theme, Renderer>> {
        None
    }

    /// The index of the overlay.
    ///
    /// Overlays with a higher index will be rendered on top of overlays with
    /// a lower index.
    ///
    /// By default, it returns `1.0`.
    fn index(&self) -> f32 {
        1.0
    }
}

/// Returns a [`Group`] of overlay [`Element`] children.
///
/// This method will generally only be used by advanced users that are
/// implementing the [`Widget`](crate::Widget) trait.
pub fn from_children<'a, Message, Theme, Renderer>(
    children: &'a mut [crate::Element<'_, Message, Theme, Renderer>],
    tree: &'a mut Tree,
    layout: Layout<'a>,
    renderer: &Renderer,
    viewport: &Rectangle,
    translation: Vector,
) -> Option<Element<'a, Message, Theme, Renderer>>
where
    Renderer: crate::Renderer,
{
    let children = children
        .iter_mut()
        .zip(&mut tree.children)
        .zip(layout.children())
        .filter_map(|((child, state), layout)| {
            child
                .as_widget_mut()
                .overlay(state, layout, renderer, viewport, translation)
        })
        .collect::<Vec<_>>();

    (!children.is_empty()).then(|| Group::with_children(children).overlay())
}

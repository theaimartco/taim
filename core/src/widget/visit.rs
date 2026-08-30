//! Walk a widget tree without naming its `Message`, `Theme`, or
//! `Renderer`.
//!
//! [`Visit`] is the **type-erased projection of [`Widget`]**: exactly
//! those of its methods whose signatures mention none of the three type
//! parameters, plus a way to reach children. [`Widgets`] adapts a real
//! `(widget, Tree, Layout)` triple to it.
//!
//! # Why this exists
//!
//! A [`Widget`] cannot be asked for its children generically — every one
//! stores them differently (a `Vec<Element>`, a single `Element`, an
//! `Option<Element>`, none at all) — so, exactly as with [`diff`],
//! [`layout`], [`draw`], [`update`] and [`operate`], the traversal is
//! driven by the widget itself, through [`Widget::visit_children`].
//!
//! The visitor is handed a `&mut dyn Visit` rather than a `&mut dyn
//! Widget<..>` because a widget reference cannot cross a **theme
//! boundary**: `themer` is a `Widget<Message, AnyTheme, Renderer>` whose
//! content is a `Widget<Message, Theme, Renderer>`, so there is no single
//! `dyn Widget<..>` type a visitor could accept for both. `operate`
//! crosses that boundary only because `&mut dyn Operation` mentions no
//! theme at all; [`Visit`] is the same trick, generalized.
//!
//! # Consumers
//!
//! Both of the framework's whole-tree traversals ride on this:
//!
//! - `taim_runtime::semantics::build` — accessibility forest assembly.
//! - `taim_runtime::hit_test` — the linear oracle and the spatial
//!   `Index`, and through them pointer/gesture routing.
//!
//! They share one hook deliberately. A second, parallel children hook
//! would mean every container implementing the same zip twice, with
//! nothing to keep the copies in step.
//!
//! # Tree-transparent wrappers
//!
//! `container`, `pin`, `float` and `themer` defer `tag`/`state`/`diff` to
//! their content, so wrapper and content **share a single [`Tree`]** —
//! they are one node, with one `semantics_with_cache` slot. Each
//! therefore delegates [`Widget::semantics`] and
//! [`Widget::visit_children`] to its content rather than reporting a node
//! and visiting the content as a child: reporting separately would cache
//! the wrapper's `None` into the shared slot and clear the dirty flag,
//! and the content's own lookup would then hit the poisoned cache and
//! report nothing.
//!
//! One consequence, worth knowing before it bites: because the content is
//! not visited as a level of its own, **its [`Widget::hit_test`] is not
//! consulted separately** — the wrapper's is, against the wrapper's
//! bounds. No widget overrides `hit_test` today (every one takes the
//! default rectangular check, and a wrapper's bounds are a superset of
//! its content's), so nothing is wrong now. A content widget with a
//! genuinely non-rectangular hit area — a circular button — would be
//! hit-tested rectangularly once wrapped in a `container`. Fixing that
//! needs the wrapper to delegate `hit_test` too, and a decision about
//! whether a `container`'s padding should remain hittable.
//!
//! [`Tree`]: crate::widget::Tree
//! [`Widget::semantics`]: crate::Widget::semantics
//! [`Widget::hit_test`]: crate::Widget::hit_test
//!
//! [`Widget`]: crate::Widget
//! [`Widget::visit_children`]: crate::Widget::visit_children
//! [`diff`]: crate::Widget::diff
//! [`layout`]: crate::Widget::layout
//! [`draw`]: crate::Widget::draw
//! [`update`]: crate::Widget::update
//! [`operate`]: crate::Widget::operate

use crate::widget::Tree;
use crate::{Layout, Point, Rectangle, semantics};

/// A node of a widget tree, with its state and layout, seen without its
/// `Message`, `Theme`, or `Renderer`.
///
/// Implement this directly only to feed a consumer something that is not
/// a widget tree (a synthetic tree in a test, say). For a real one, use
/// [`Widgets`].
pub trait Visit {
    /// This node's own, non-recursive [`semantics::Node`], reusing a
    /// cached result when the node's [`Tree`] is not semantics-dirty.
    ///
    /// See [`Widget::semantics`](crate::Widget::semantics).
    fn semantics(&mut self) -> Option<semantics::Node>;

    /// Whether `point` is part of this node.
    ///
    /// See [`Widget::hit_test`](crate::Widget::hit_test).
    fn contains(&self, point: Point) -> bool;

    /// This node's own bounds, in the same coordinate space
    /// [`Visit::contains`] tests `point` against.
    ///
    /// # Invariant
    /// Must be a superset of wherever [`Visit::contains`] can ever return
    /// `true`. A node with a smaller effective hit area than its bounds
    /// (a circular button, a clipped scrollable) satisfies this
    /// trivially; a node whose `contains` accepted a point *outside* its
    /// reported `bounds` would let a spatial index prune a subtree
    /// containing a real hit.
    fn bounds(&self) -> Rectangle;

    /// Visits each child, in document order.
    fn children(&mut self, visit: &mut dyn FnMut(&mut dyn Visit));
}

/// Adapts a real `(widget, Tree, Layout)` triple to [`Visit`].
///
/// Owning the [`Tree::semantics_with_cache`] call here, rather than
/// leaving it to each widget, is deliberate: a widget that forgot it
/// would silently recompute its node every frame, and nothing would
/// fail.
///
/// [`Tree::semantics_with_cache`]: crate::widget::tree::Tree::semantics_with_cache
pub struct Widgets<'a, Message, Theme, Renderer> {
    widget: &'a mut dyn crate::Widget<Message, Theme, Renderer>,
    tree: &'a mut Tree,
    layout: Layout<'a>,
}

impl<'a, Message, Theme, Renderer> Widgets<'a, Message, Theme, Renderer> {
    /// Adapts the given `widget`, its `tree`, and its `layout`.
    ///
    /// The three must belong to one another — the same triple every other
    /// traversal (`draw`, `operate`) is handed.
    pub fn new(
        widget: &'a mut dyn crate::Widget<Message, Theme, Renderer>,
        tree: &'a mut Tree,
        layout: Layout<'a>,
    ) -> Self {
        Self {
            widget,
            tree,
            layout,
        }
    }
}

impl<Message, Theme, Renderer> Visit for Widgets<'_, Message, Theme, Renderer>
where
    Renderer: crate::Renderer,
{
    fn semantics(&mut self) -> Option<semantics::Node> {
        let widget = &*self.widget;
        let layout = self.layout;

        self.tree
            .semantics_with_cache(|tree| widget.semantics(tree, layout))
    }

    fn contains(&self, point: Point) -> bool {
        self.widget.hit_test(self.tree, self.layout, point)
    }

    fn bounds(&self) -> Rectangle {
        self.layout.bounds()
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut dyn Visit)) {
        let layout = self.layout;

        self.widget.visit_children(self.tree, layout, visit);
    }
}

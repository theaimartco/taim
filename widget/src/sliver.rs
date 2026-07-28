//! A protocol for scrollable regions that report and consume their
//! extent lazily, so several of them can share a single scroll offset
//! without any one of them needing to know its full extent up front.
//!
//! [`crate::VirtualList`] solves "one huge list" by owning its scroll
//! offset outright. [`Sliver`] solves a different, complementary
//! problem: composing *several* scrollable regions -- a header, a huge
//! list, a footer -- under one shared scroll position, where the second
//! region should not need to lay out (or even know the extent of) the
//! first before it can determine what of *itself* is visible.
//!
//! The protocol is intentionally small. A [`Sliver`] is given
//! [`SliverConstraints`] (how far the shared scroll position has already
//! moved past the start of this sliver, and how much viewport extent
//! remains after earlier slivers claimed their share) and returns
//! [`SliverGeometry`] (how much extent it actually occupies and how much
//! of that is currently painted) -- never a [`crate::core::layout::Node`]
//! sized to its full, potentially unbounded, content.
use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::renderer;
use crate::core::widget::visit;
use crate::core::widget::{Operation, Tree};
use crate::core::{Element, Event, Rectangle, Shell, Size, Vector};

/// What a [`Sliver`] is asked to lay itself out against: how far the
/// shared scroll offset has already scrolled past this sliver's own
/// start, and how much viewport extent is left for it to claim.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliverConstraints {
    /// The width available to the sliver (slivers only vary along the
    /// scroll axis; the cross axis behaves like an ordinary [`Widget`]).
    pub cross_axis_extent: f32,

    /// How far the shared scroll offset has moved past the start of
    /// this sliver, in its own local scroll-axis coordinate space.
    ///
    /// `0.0` means this sliver's start coincides with the top of the
    /// viewport (nothing of it has been scrolled past yet); a positive
    /// value means that much of its extent, from its own start, is
    /// already scrolled out of view above the viewport.
    pub scroll_offset: f32,

    /// How much viewport extent remains for this sliver and any sliver
    /// after it to occupy, after whatever came before it in the same
    /// scroll view already claimed its share.
    pub remaining_paint_extent: f32,

    /// Extent beyond the remaining viewport that should still be kept
    /// built/laid out (mirrors [`crate::VirtualList::cache_extent`]),
    /// so a fast scroll does not momentarily flash blank content.
    pub cache_extent: f32,
}

impl SliverConstraints {
    /// The total extent, from `scroll_offset` onward, that is worth a
    /// [`Sliver`] actually building/laying out content for: the visible
    /// remainder plus the cache extent on either side.
    pub fn paintable_extent(&self) -> f32 {
        self.remaining_paint_extent + self.cache_extent * 2.0
    }
}

/// What a [`Sliver`] reports back after being laid out against some
/// [`SliverConstraints`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliverGeometry {
    /// The total extent this sliver occupies along the scroll axis,
    /// regardless of how much of that is actually visible right now.
    ///
    /// A parent scroll view uses the sum of every sliver's
    /// [`SliverGeometry::scroll_extent`] (not
    /// [`SliverGeometry::paint_extent`]) to size its scrollbar --
    /// exactly the property that lets a sliver report "I am 50,000
    /// pixels tall in total" without ever having laid out all 50,000
    /// pixels of content.
    pub scroll_extent: f32,

    /// How much of [`SliverGeometry::scroll_extent`] is actually
    /// painted for the [`SliverConstraints`] this was computed from
    /// (i.e. intersects the visible-plus-cache-extent window). Always
    /// `<= constraints.paintable_extent()`.
    pub paint_extent: f32,

    /// The offset, from the start of this sliver's own extent, at which
    /// its painted content begins -- `0.0` unless the sliver's start is
    /// itself scrolled past the viewport, in which case painted content
    /// begins partway through its full extent.
    pub paint_origin: f32,
}

impl SliverGeometry {
    /// The geometry of a sliver with no content at all.
    pub const ZERO: Self = Self {
        scroll_extent: 0.0,
        paint_extent: 0.0,
        paint_origin: 0.0,
    };
}

/// A scrollable region that can report and consume its extent lazily,
/// for composition under one shared scroll offset.
///
/// [`Sliver`] deliberately mirrors [`Widget`]'s shape (`layout`, `draw`,
/// `update`, ...) rather than inventing unrelated names, since a
/// [`Sliver`] *is* conceptually a [`Widget`] restricted to the scroll
/// axis -- the only real difference is that [`Sliver::layout`] receives
/// [`SliverConstraints`] instead of [`layout::Limits`], and returns
/// [`SliverGeometry`] instead of a sized [`layout::Node`].
pub trait Sliver<Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    /// Lays out this sliver against `constraints`, returning the
    /// [`SliverGeometry`] it occupies. Implementations must build/lay
    /// out content only for the `constraints.paintable_extent()` window,
    /// exactly as [`crate::VirtualList`] does internally.
    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        constraints: SliverConstraints,
    ) -> SliverGeometry;

    /// Draws whatever this sliver last laid out, translated so that its
    /// own local scroll-axis origin lands at `paint_offset` within the
    /// parent scroll view's own coordinate space.
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        bounds: Rectangle,
        paint_offset: Vector,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    );

    /// Processes a runtime [`Event`], analogous to [`Widget::update`].
    fn update(
        &mut self,
        _tree: &mut Tree,
        _event: &Event,
        _bounds: Rectangle,
        _paint_offset: Vector,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        _shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
    }

    /// Visits this [`Sliver`]'s content for semantic assembly, analogous
    /// to [`Widget::semantics_children`].
    ///
    /// Mirrors [`Sliver::operate`]. The default visits nothing, which is
    /// correct for a sliver that draws its own content directly.
    fn visit_children(
        &mut self,
        _tree: &mut Tree,
        _bounds: Rectangle,
        _paint_offset: Vector,
        _visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
    }

    /// Applies an [`Operation`], analogous to [`Widget::operate`].
    fn operate(
        &mut self,
        _tree: &mut Tree,
        _bounds: Rectangle,
        _paint_offset: Vector,
        _renderer: &Renderer,
        _operation: &mut dyn Operation,
    ) {
    }
}

/// The simplest possible [`Sliver`]: an ordinary, fixed-extent
/// [`Element`] wrapped to participate in the [`Sliver`] protocol.
///
/// This exists to prove the protocol composes with the rest of the
/// widget tree (an ordinary [`Widget`] can always become a trivial,
/// non-virtualized [`Sliver`]), and as the natural choice for a small
/// header/footer sliver that doesn't need virtualization itself.
pub struct FixedExtent<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Theme, Renderer> FixedExtent<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    /// Wraps `content` as a fixed-extent [`Sliver`].
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            content: content.into(),
        }
    }
}

impl<'a, Message, Theme, Renderer> Sliver<Message, Theme, Renderer>
    for FixedExtent<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        constraints: SliverConstraints,
    ) -> SliverGeometry {
        let limits = layout::Limits::new(
            Size::new(constraints.cross_axis_extent, 0.0),
            Size::new(constraints.cross_axis_extent, f32::INFINITY),
        );

        let node = self.content.as_widget_mut().layout(tree, renderer, &limits);
        let scroll_extent = node.size().height;

        let visible_start = constraints.scroll_offset.max(0.0);
        let visible_end = (constraints.scroll_offset
            + constraints.remaining_paint_extent
            + constraints.cache_extent)
            .min(scroll_extent);

        let paint_extent = (visible_end - visible_start).max(0.0);

        SliverGeometry {
            scroll_extent,
            paint_extent,
            paint_origin: visible_start,
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        bounds: Rectangle,
        paint_offset: Vector,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let node = layout::Node::new(Size::new(bounds.width, bounds.height));
        let layout = Layout::with_offset(paint_offset, &node);

        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        bounds: Rectangle,
        paint_offset: Vector,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let node = layout::Node::new(Size::new(bounds.width, bounds.height));
        let layout = Layout::with_offset(paint_offset, &node);

        self.content
            .as_widget_mut()
            .update(tree, event, layout, cursor, renderer, shell, viewport);
    }

    fn visit_children(
        &mut self,
        tree: &mut Tree,
        bounds: Rectangle,
        paint_offset: Vector,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        let node = layout::Node::new(Size::new(bounds.width, bounds.height));
        let layout = Layout::with_offset(paint_offset, &node);

        visit(&mut visit::Widgets::new(
            self.content.as_widget_mut(),
            tree,
            layout,
        ));
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        bounds: Rectangle,
        paint_offset: Vector,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let node = layout::Node::new(Size::new(bounds.width, bounds.height));
        let layout = Layout::with_offset(paint_offset, &node);

        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paintable_extent_covers_the_remaining_viewport_plus_both_cache_sides() {
        let constraints = SliverConstraints {
            cross_axis_extent: 100.0,
            scroll_offset: 0.0,
            remaining_paint_extent: 500.0,
            cache_extent: 50.0,
        };

        assert_eq!(constraints.paintable_extent(), 600.0);
    }

    #[test]
    fn zero_geometry_has_no_extent() {
        assert_eq!(SliverGeometry::ZERO.scroll_extent, 0.0);
        assert_eq!(SliverGeometry::ZERO.paint_extent, 0.0);
        assert_eq!(SliverGeometry::ZERO.paint_origin, 0.0);
    }
}

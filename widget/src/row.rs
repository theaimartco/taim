//! Distribute content horizontally.
use crate::core::alignment::{self, Alignment};
use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::widget::visit;
use crate::core::widget::{self, Operation, Tree};
use crate::core::{
    Element, Event, Length, Padding, Pixels, Rectangle, Shell, Size, Vector, Widget,
};

/// The internal, per-instance state of a [`Row`]: a bounded measure
/// cache keyed on the [`layout::Limits`] it is asked to resolve under.
///
/// A fresh [`Row`] (or one whose children changed shape, see
/// [`Widget::diff`]) starts with an empty cache, so the first `layout`
/// call after any structural change always computes for real; only a
/// `layout` call under previously-seen [`layout::Limits`] with an
/// unchanged set of children can take the fast, cached path.
#[derive(Debug, Default)]
struct State {
    cache: layout::Cache,
}

/// A container that distributes its contents horizontally.
///
/// # Example
/// ```no_run
/// # mod taim { pub mod widget { pub use taim_widget::*; } }
/// # pub type State = ();
/// # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
/// use taim::widget::{button, row};
///
/// #[derive(Debug, Clone)]
/// enum Message {
///     // ...
/// }
///
/// fn view(state: &State) -> Element<'_, Message> {
///     row![
///         "I am to the left!",
///         button("I am in the middle!"),
///         "I am to the right!",
///     ].into()
/// }
/// ```
pub struct Row<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer> {
    spacing: f32,
    padding: Padding,
    width: Length,
    height: Length,
    align: Alignment,
    clip: bool,
    children: Vec<Element<'a, Message, Theme, Renderer>>,
}

impl<'a, Message, Theme, Renderer> Row<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    /// Creates an empty [`Row`].
    pub fn new() -> Self {
        Self::from_vec(Vec::new())
    }

    /// Creates a [`Row`] with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self::from_vec(Vec::with_capacity(capacity))
    }

    /// Creates a [`Row`] with the given elements.
    pub fn with_children(
        children: impl IntoIterator<Item = Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        let iterator = children.into_iter();

        Self::with_capacity(iterator.size_hint().0).extend(iterator)
    }

    /// Creates a [`Row`] from an already allocated [`Vec`].
    ///
    /// Keep in mind that the [`Row`] will not inspect the [`Vec`], which means
    /// it won't automatically adapt to the sizing strategy of its contents.
    ///
    /// If any of the children have a [`Length::Fill`] strategy, you will need to
    /// call [`Row::width`] or [`Row::height`] accordingly.
    pub fn from_vec(mut children: Vec<Element<'a, Message, Theme, Renderer>>) -> Self {
        children.retain(|child| !child.as_widget().is_void());

        Self {
            spacing: 0.0,
            padding: Padding::ZERO,
            width: Length::Fit,
            height: Length::Fit,
            align: Alignment::Start,
            clip: false,
            children,
        }
    }

    /// Sets the horizontal spacing _between_ elements.
    ///
    /// Custom margins per element do not exist in taim. You should use this
    /// method instead! While less flexible, it helps you keep spacing between
    /// elements consistent.
    pub fn spacing(mut self, amount: impl Into<Pixels>) -> Self {
        self.spacing = finite_nonnegative(amount.into().0);
        self
    }

    /// Sets the [`Padding`] of the [`Row`].
    pub fn padding<P: Into<Padding>>(mut self, padding: P) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the width of the [`Row`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`Row`].
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the vertical alignment of the contents of the [`Row`] .
    pub fn align_y(mut self, align: impl Into<alignment::Vertical>) -> Self {
        self.align = Alignment::from(align.into());
        self
    }

    /// Sets whether the contents of the [`Row`] should be clipped on
    /// overflow.
    pub fn clip(mut self, clip: bool) -> Self {
        self.clip = clip;
        self
    }

    /// Adds an [`Element`] to the [`Row`].
    pub fn push(mut self, child: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        let child = child.into();

        if !child.as_widget().is_void() {
            self.children.push(child);
        }

        self
    }

    /// Extends the [`Row`] with the given children.
    pub fn extend(
        mut self,
        children: impl IntoIterator<Item = Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        let iterator = children.into_iter();
        self.children.reserve(iterator.size_hint().0);

        for child in iterator {
            if !child.as_widget().is_void() {
                self.children.push(child);
            }
        }

        self
    }

    /// Turns the [`Row`] into a [`Wrapping`] row.
    ///
    /// The original alignment of the [`Row`] is preserved per row wrapped.
    pub fn wrap(self) -> Wrapping<'a, Message, Theme, Renderer> {
        Wrapping {
            row: self,
            vertical_spacing: None,
            align_x: alignment::Horizontal::Left,
        }
    }
}

impl<Message, Renderer> Default for Row<'_, Message, Renderer>
where
    Renderer: crate::core::Renderer,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, Message, Theme, Renderer: crate::core::Renderer>
    FromIterator<Element<'a, Message, Theme, Renderer>> for Row<'a, Message, Theme, Renderer>
{
    fn from_iter<T: IntoIterator<Item = Element<'a, Message, Theme, Renderer>>>(iter: T) -> Self {
        Self::with_children(iter)
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Row<'_, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(&mut self.children);

        // Ordinary Elm widget properties do not report fine-grained "my
        // content changed in a way that affects measurement" revisions.
        // Signal-backed lazy subtrees have that contract, but a general Row
        // may still contain legacy children. Any Row `diff` pass is therefore
        // conservatively assumed to
        // be able to invalidate a child's measurement, so the cache is
        // cleared here rather than risk serving a stale `Node` for
        // changed content. This still lets `layout` calls that happen
        // *without* an intervening `diff` (e.g. `UserInterface::relayout`
        // re-measuring an otherwise-untouched tree) hit the cache.
        tree.state.downcast_mut::<State>().cache.clear();

        if self.width.is_fit() || self.height.is_fit() {
            for child in &self.children {
                let size = child.as_widget().size();

                self.width = self.width.stack(size.width);
                self.height = self.height.cross(size.height);
            }
        }
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let fingerprint = layout::cache::Fingerprint::build(|hasher| {
            hasher.write_usize(self.children.len());
            self.width.hash_stable(hasher);
            self.height.hash_stable(hasher);
            hasher.write_u32(self.spacing.to_bits());
            hash_padding(self.padding, hasher);
            hasher.write_u8(alignment_tag(self.align));
        });

        let state = tree.state.downcast_mut::<State>();
        let children = &mut self.children;
        let padding = self.padding;
        let spacing = self.spacing;
        let align = self.align;
        let width = self.width;
        let height = self.height;

        layout::flex::resolve_cached(
            &mut state.cache,
            fingerprint,
            layout::flex::Axis::Horizontal,
            renderer,
            limits,
            width,
            height,
            padding,
            spacing,
            align,
            children,
            &mut tree.children,
        )
    }

    fn visit_children(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        self.children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .for_each(|((child, state), layout)| {
                visit(&mut visit::Widgets::new(
                    child.as_widget_mut(),
                    state,
                    layout,
                ));
            });
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            self.children
                .iter_mut()
                .zip(&mut tree.children)
                .zip(layout.children())
                .for_each(|((child, state), layout)| {
                    child
                        .as_widget_mut()
                        .operate(state, layout, renderer, operation);
                });
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
        for ((child, tree), layout) in self
            .children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
        {
            child
                .as_widget_mut()
                .update(tree, event, layout, cursor, renderer, shell, viewport);
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((child, tree), layout)| {
                child
                    .as_widget()
                    .mouse_interaction(tree, layout, cursor, viewport, renderer)
            })
            .max()
            .unwrap_or_default()
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
        if let Some(clipped_viewport) = layout.bounds().intersection(viewport) {
            let viewport = if self.clip {
                &clipped_viewport
            } else {
                viewport
            };

            for ((child, tree), layout) in self
                .children
                .iter()
                .zip(&tree.children)
                .zip(layout.children())
            {
                let widget = child.as_widget();

                if widget.paint_bounds(tree, layout).intersects(viewport) {
                    widget.draw(tree, renderer, theme, style, layout, cursor, viewport);
                }
            }
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        overlay::from_children(
            &mut self.children,
            tree,
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<Row<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: crate::core::Renderer + 'a,
{
    fn from(row: Row<'a, Message, Theme, Renderer>) -> Self {
        Self::new(row)
    }
}

/// A [`Row`] that wraps its contents.
///
/// Create a [`Row`] first, and then call [`Row::wrap`] to
/// obtain a [`Row`] that wraps its contents.
///
/// The original alignment of the [`Row`] is preserved per row wrapped.
pub struct Wrapping<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer> {
    row: Row<'a, Message, Theme, Renderer>,
    vertical_spacing: Option<f32>,
    align_x: alignment::Horizontal,
}

impl<Message, Theme, Renderer> Wrapping<'_, Message, Theme, Renderer> {
    /// Sets the vertical spacing _between_ lines.
    pub fn vertical_spacing(mut self, amount: impl Into<Pixels>) -> Self {
        self.vertical_spacing = Some(finite_nonnegative(amount.into().0));
        self
    }

    /// Sets the horizontal alignment of the wrapping [`Row`].
    pub fn align_x(mut self, align_x: impl Into<alignment::Horizontal>) -> Self {
        self.align_x = align_x.into();
        self
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Wrapping<'_, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    fn tag(&self) -> widget::tree::Tag {
        self.row.tag()
    }

    fn state(&self) -> widget::tree::State {
        self.row.state()
    }

    fn diff(&mut self, tree: &mut Tree) {
        self.row.diff(tree);
    }

    fn size(&self) -> Size<Length> {
        self.row.size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let limits = limits
            .width(self.row.width)
            .height(self.row.height)
            .shrink(self.row.padding);

        // TAIM uses the same explicit-group placement contract for Row and
        // Column. Child measurement is a single pass; positioning requires
        // no second invocation of widget layout or inference from x == 0.
        let child_limits = limits.loose();
        let nodes: Vec<_> = self
            .row
            .children
            .iter_mut()
            .zip(&mut tree.children)
            .map(|(element, state)| {
                element.as_widget_mut().layout(state, renderer, &child_limits)
            })
            .collect();

        let (intrinsic, nodes) = crate::column::placement::arrange_rows(
            nodes,
            limits.max().width,
            self.row.spacing,
            self.vertical_spacing.unwrap_or(self.row.spacing),
            self.row.padding,
            self.row.align,
            self.align_x,
        );

        let size = limits.resolve(self.row.width, self.row.height, intrinsic);
        layout::Node::with_children(size.expand(self.row.padding), nodes)
    }

    fn visit_children(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        self.row.visit_children(tree, layout, visit);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.row.operate(tree, layout, renderer, operation);
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
        self.row
            .update(tree, event, layout, cursor, renderer, shell, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.row
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
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
        self.row
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.row
            .overlay(tree, layout, renderer, viewport, translation)
    }
}

impl<'a, Message, Theme, Renderer> From<Wrapping<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: crate::core::Renderer + 'a,
{
    fn from(row: Wrapping<'a, Message, Theme, Renderer>) -> Self {
        Self::new(row)
    }
}

#[inline]
fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[inline]
fn hash_padding(padding: Padding, hasher: &mut dyn std::hash::Hasher) {
    hasher.write_u32(padding.top.to_bits());
    hasher.write_u32(padding.right.to_bits());
    hasher.write_u32(padding.bottom.to_bits());
    hasher.write_u32(padding.left.to_bits());
}

#[inline]
const fn alignment_tag(alignment: Alignment) -> u8 {
    match alignment {
        Alignment::Start => 0,
        Alignment::Center => 1,
        Alignment::End => 2,
    }
}

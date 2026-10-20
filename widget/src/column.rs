//! Distribute content vertically.
pub(crate) mod placement;
use crate::core::alignment::{self, Alignment};
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::widget::visit;
use crate::core::widget::{self, Operation, Tree};
use crate::core::{
    Element, Event, Layout, Length, Padding, Pixels, Rectangle, Shell, Size, Vector, Widget,
};

/// The internal, per-instance state of a [`Column`]: a bounded measure
/// cache keyed on the [`layout::Limits`] it is asked to resolve under.
///
/// Mirrors `Row`'s identical caching strategy along the vertical axis;
/// see that type's internal `State` for the full rationale.
#[derive(Debug, Default)]
struct State {
    cache: layout::Cache,
}

/// A container that distributes its contents vertically.
///
/// # Example
/// ```no_run
/// # mod taim { pub mod widget { pub use taim_widget::*; } }
/// # pub type State = ();
/// # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
/// use taim::widget::{button, column};
///
/// #[derive(Debug, Clone)]
/// enum Message {
///     // ...
/// }
///
/// fn view(state: &State) -> Element<'_, Message> {
///     column![
///         "I am on top!",
///         button("I am in the center!"),
///         "I am below.",
///     ].into()
/// }
/// ```
pub struct Column<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer> {
    spacing: f32,
    padding: Padding,
    width: Length,
    height: Length,
    /// Original builder lengths, unaffected by inferred child sizing.
    declared: Size<Length>,
    align: Alignment,
    clip: bool,
    children: Vec<Element<'a, Message, Theme, Renderer>>,
}

impl<'a, Message, Theme, Renderer> Column<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    /// Creates an empty [`Column`].
    pub fn new() -> Self {
        Self::from_vec(Vec::new())
    }

    /// Creates a [`Column`] with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self::from_vec(Vec::with_capacity(capacity))
    }

    /// Creates a [`Column`] with the given elements.
    pub fn with_children(
        children: impl IntoIterator<Item = Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        let iterator = children.into_iter();

        Self::with_capacity(iterator.size_hint().0).extend(iterator)
    }

    /// Creates a [`Column`] from an already allocated [`Vec`].
    pub fn from_vec(mut children: Vec<Element<'a, Message, Theme, Renderer>>) -> Self {
        children.retain(|child| !child.as_widget().is_void());

        Self {
            spacing: 0.0,
            padding: Padding::ZERO,
            width: Length::Fit,
            height: Length::Fit,
            declared: Size::new(Length::Fit, Length::Fit),
            align: Alignment::Start,
            clip: false,
            children,
        }
    }

    /// Sets the vertical spacing _between_ elements.
    ///
    /// Custom margins per element do not exist in taim. You should use this
    /// method instead! While less flexible, it helps you keep spacing between
    /// elements consistent.
    pub fn spacing(mut self, amount: impl Into<Pixels>) -> Self {
        self.spacing = finite_nonnegative(amount.into().0);
        self
    }

    /// Sets the [`Padding`] of the [`Column`].
    pub fn padding<P: Into<Padding>>(mut self, padding: P) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the width of the [`Column`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        let width = width.into();
        self.declared.width = width;
        self.width = width;
        self
    }

    /// Sets the height of the [`Column`].
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        let height = height.into();
        self.declared.height = height;
        self.height = height;
        self
    }

    /// Sets the horizontal alignment of the contents of the [`Column`] .
    pub fn align_x(mut self, align: impl Into<alignment::Horizontal>) -> Self {
        self.align = Alignment::from(align.into());
        self
    }

    /// Sets whether the contents of the [`Column`] should be clipped on
    /// overflow.
    pub fn clip(mut self, clip: bool) -> Self {
        self.clip = clip;
        self
    }

    /// Adds an element to the [`Column`].
    pub fn push(mut self, child: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        let child = child.into();

        if !child.as_widget().is_void() {
            self.children.push(child);
        }

        self
    }

    /// Extends the [`Column`] with the given children.
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

    /// Turns the [`Column`] into a [`Wrapping`] column.
    ///
    /// The original alignment of the [`Column`] is preserved per column wrapped.
    pub fn wrap(self) -> Wrapping<'a, Message, Theme, Renderer> {
        Wrapping {
            column: self,
            horizontal_spacing: None,
            align_y: alignment::Vertical::Top,
        }
    }
}

impl<Message, Renderer> Default for Column<'_, Message, Renderer>
where
    Renderer: crate::core::Renderer,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, Message, Theme, Renderer: crate::core::Renderer>
    FromIterator<Element<'a, Message, Theme, Renderer>> for Column<'a, Message, Theme, Renderer>
{
    fn from_iter<T: IntoIterator<Item = Element<'a, Message, Theme, Renderer>>>(iter: T) -> Self {
        Self::with_children(iter)
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Column<'_, Message, Theme, Renderer>
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

        // See `Row::diff`'s identical comment: clearing unconditionally
        // is the conservative, currently-correct choice until per-widget
        // `diff` reports fine-grained layout-affecting changes.
        tree.state.downcast_mut::<State>().cache.clear();

        let size = crate::linear::measure_intent(
            self.declared,
            crate::linear::Flow::Column,
            self.children.iter().map(|child| child.as_widget().size()),
        );
        self.width = size.width;
        self.height = size.height;
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
        let fingerprint = crate::linear::fingerprint(
            self.children.len(),
            Size::new(self.width, self.height),
            self.padding,
            self.spacing,
            self.align,
        );

        let state = tree.state.downcast_mut::<State>();

        layout::flex::resolve_cached(
            &mut state.cache,
            fingerprint,
            layout::flex::Axis::Vertical,
            renderer,
            limits,
            self.width,
            self.height,
            self.padding,
            self.spacing,
            self.align,
            &mut self.children,
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

impl<'a, Message, Theme, Renderer> From<Column<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: crate::core::Renderer + 'a,
{
    fn from(column: Column<'a, Message, Theme, Renderer>) -> Self {
        Self::new(column)
    }
}

/// A [`Column`] that wraps its contents.
///
/// Create a [`Column`] first, and then call [`Column::wrap`] to
/// obtain a [`Column`] that wraps its contents.
///
/// The original alignment of the [`Column`] is preserved per column wrapped.
#[allow(missing_debug_implementations)]
pub struct Wrapping<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer> {
    column: Column<'a, Message, Theme, Renderer>,
    horizontal_spacing: Option<f32>,
    align_y: alignment::Vertical,
}

impl<Message, Theme, Renderer> Wrapping<'_, Message, Theme, Renderer> {
    /// Sets the horizontal spacing _between_ columns.
    pub fn horizontal_spacing(mut self, amount: impl Into<Pixels>) -> Self {
        self.horizontal_spacing = Some(finite_nonnegative(amount.into().0));
        self
    }

    /// Sets the vertical alignment of the wrapping [`Column`].
    pub fn align_y(mut self, align_y: impl Into<alignment::Vertical>) -> Self {
        self.align_y = align_y.into();
        self
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Wrapping<'_, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    fn tag(&self) -> widget::tree::Tag {
        self.column.tag()
    }

    fn state(&self) -> widget::tree::State {
        self.column.state()
    }

    fn diff(&mut self, tree: &mut Tree) {
        self.column.diff(tree);
    }

    fn size(&self) -> Size<Length> {
        self.column.size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let limits = limits
            .width(self.column.width)
            .height(self.column.height)
            .shrink(self.column.padding);

        // TAIM separates measurement from placement. Each child is laid out
        // exactly once; the grouping pass only moves the returned nodes.
        // Group boundaries are explicit, not inferred from a child's Y value
        // (which changes under padding and alignment).
        let child_limits = limits.loose();
        let nodes: Vec<_> = self
            .column
            .children
            .iter_mut()
            .zip(&mut tree.children)
            .map(|(element, state)| {
                element
                    .as_widget_mut()
                    .layout(state, renderer, &child_limits)
            })
            .collect();

        let (intrinsic, nodes) = placement::arrange(
            nodes,
            limits.max().height,
            self.column.spacing,
            self.horizontal_spacing.unwrap_or(self.column.spacing),
            self.column.padding,
            self.column.align,
            self.align_y,
        );

        let size = limits.resolve(self.column.width, self.column.height, intrinsic);
        layout::Node::with_children(size.expand(self.column.padding), nodes)
    }

    fn visit_children(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        self.column.visit_children(tree, layout, visit);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.column.operate(tree, layout, renderer, operation);
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
        self.column
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
        self.column
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
        self.column
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
        self.column
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
    fn from(column: Wrapping<'a, Message, Theme, Renderer>) -> Self {
        Self::new(column)
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


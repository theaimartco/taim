//! Display a large, possibly huge, list of items without paying for the
//! ones that are not on screen.
//!
//! Unlike [`crate::Scrollable`] wrapping a [`crate::Column`] of every
//! item, [`VirtualList`] never builds, diffs, lays out, or paints an
//! [`Element`] for an index outside its visible window (plus a small,
//! configurable [`VirtualList::cache_extent`] beyond it) -- the whole
//! point being that a `100,000`-row list costs the same, at any given
//! frame, as however many rows actually fit on screen.
use crate::core::keyboard;
use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::renderer;
use crate::core::widget::visit;
use crate::core::widget::{self, Operation, Tree};
use crate::core::{Element, Event, Length, Pixels, Point, Rectangle, Shell, Size, Widget};

/// The height strategy of the items in a [`VirtualList`].
pub enum ItemHeight<'a> {
    /// Every item has the same, fixed height.
    Fixed(f32),
    /// Each item's height is computed by the given function from its
    /// index, before that item is ever built into an [`Element`].
    ///
    /// This must be cheap and must return a value consistent with the
    /// [`Element`] the builder eventually produces for the same index --
    /// [`VirtualList`] positions items using only this function, never
    /// the [`Element`]'s own measured size, so a mismatch produces
    /// visible overlap or gaps, not a panic.
    Variable(Box<dyn Fn(usize) -> f32 + 'a>),
}

impl std::fmt::Debug for ItemHeight<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fixed(height) => f.debug_tuple("Fixed").field(height).finish(),
            Self::Variable(_) => f.debug_tuple("Variable").field(&"<function>").finish(),
        }
    }
}

/// A container that lazily builds and displays only the visible slice of
/// a, possibly huge, sequence of items.
///
/// # Example
/// ```no_run
/// # mod aura { pub mod widget { pub use aura_widget::*; } }
/// # pub type Element<'a, Message> = aura_widget::core::Element<'a, Message, aura_widget::Theme, aura_widget::Renderer>;
/// use aura::widget::{text, virtual_list};
///
/// #[derive(Debug, Clone)]
/// enum Message {
///     // ...
/// }
///
/// fn view() -> Element<'static, Message> {
///     virtual_list(100_000, 24.0, |i| text(format!("Row {i}")).into()).into()
/// }
/// ```
pub struct VirtualList<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
{
    item_count: usize,
    item_height: ItemHeight<'a>,
    cache_extent: f32,
    width: Length,
    height: Length,
    builder: Box<dyn Fn(usize) -> Element<'a, Message, Theme, Renderer> + 'a>,
}

impl<'a, Message, Theme, Renderer> VirtualList<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    /// Creates a new [`VirtualList`] with `item_count` items, each with
    /// the given fixed `item_height`, built lazily by calling `builder`
    /// with the index of each item that is actually visible.
    pub fn new(
        item_count: usize,
        item_height: impl Into<Pixels>,
        builder: impl Fn(usize) -> Element<'a, Message, Theme, Renderer> + 'a,
    ) -> Self {
        Self {
            item_count,
            item_height: ItemHeight::Fixed(item_height.into().0),
            cache_extent: 400.0,
            width: Length::Fill,
            height: Length::Fill,
            builder: Box::new(builder),
        }
    }

    /// Creates a new [`VirtualList`] whose items may have different
    /// heights, computed by `height_of` from each index.
    pub fn variable(
        item_count: usize,
        height_of: impl Fn(usize) -> f32 + 'a,
        builder: impl Fn(usize) -> Element<'a, Message, Theme, Renderer> + 'a,
    ) -> Self {
        Self {
            item_count,
            item_height: ItemHeight::Variable(Box::new(height_of)),
            cache_extent: 400.0,
            width: Length::Fill,
            height: Length::Fill,
            builder: Box::new(builder),
        }
    }

    /// Sets the width of the [`VirtualList`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`VirtualList`].
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets how many pixels of items beyond the visible window are kept
    /// built, laid out, and painted on either side of it.
    ///
    /// A larger [`cache_extent`](Self::cache_extent) makes a fast scroll
    /// (a large wheel delta, a jump-to-position) less likely to
    /// momentarily show a blank gap while new items are built, at the
    /// cost of doing proportionally more work per frame. `0.0` builds
    /// exactly the items that intersect the viewport and no more.
    pub fn cache_extent(mut self, pixels: impl Into<Pixels>) -> Self {
        self.cache_extent = pixels.into().0.max(0.0);
        self
    }
}

/// A cumulative position table over `0..item_count`, letting
/// [`VirtualList`] find "which index is at pixel Y" without a linear
/// scan -- essential to stay fast at `100,000+` items.
///
/// The fixed-height case never needs a real cumulative table (`index *
/// height` is `O(1)`), so it is kept as its own variant rather than
/// materializing a same-value-everywhere `Vec` just to share one code
/// path; the variable-height case pays for a real table and a binary
/// search, in exchange for supporting per-item heights at all.
#[derive(Debug, Clone)]
enum Positions {
    Fixed {
        item_count: usize,
        height: f32,
    },
    Variable {
        /// Cumulative start offset of each item; `starts[i]` is the top
        /// of item `i`, and `starts[item_count]` (a sentinel) is the
        /// total extent of every item combined.
        starts: Vec<f32>,
    },
}

impl Positions {
    fn fixed(item_count: usize, height: f32) -> Self {
        Self::Fixed {
            item_count,
            height: height.max(0.0),
        }
    }

    fn variable(item_count: usize, height_of: impl Fn(usize) -> f32) -> Self {
        let mut starts = Vec::with_capacity(item_count + 1);
        let mut cursor = 0.0;

        for index in 0..item_count {
            starts.push(cursor);
            cursor += height_of(index).max(0.0);
        }

        starts.push(cursor);

        Self::Variable { starts }
    }

    fn item_count(&self) -> usize {
        match self {
            Self::Fixed { item_count, .. } => *item_count,
            Self::Variable { starts } => starts.len().saturating_sub(1),
        }
    }

    fn total_extent(&self) -> f32 {
        match self {
            Self::Fixed { item_count, height } => *item_count as f32 * height,
            Self::Variable { starts } => starts.last().copied().unwrap_or(0.0),
        }
    }

    fn start_of(&self, index: usize) -> f32 {
        match self {
            Self::Fixed { item_count, height } => index.min(*item_count) as f32 * height,
            Self::Variable { starts } => starts[index.min(self.item_count())],
        }
    }

    fn height_of(&self, index: usize) -> f32 {
        match self {
            Self::Fixed { height, .. } => *height,
            Self::Variable { .. } => self.start_of(index + 1) - self.start_of(index),
        }
    }

    /// Returns the index of the first item whose extent intersects
    /// `y` (content-space).
    fn index_at(&self, y: f32) -> usize {
        if self.item_count() == 0 {
            return 0;
        }

        match self {
            Self::Fixed { item_count, height } if *height > 0.0 => {
                ((y / height).floor().max(0.0) as usize).min(item_count.saturating_sub(1))
            }
            Self::Fixed { .. } => 0,
            Self::Variable { starts } => {
                let count = self.item_count();

                match starts[..count].binary_search_by(|start| start.total_cmp(&y)) {
                    Ok(index) => index,
                    Err(0) => 0,
                    Err(next) => next - 1,
                }
            }
        }
    }

    /// Returns the range of indices whose extent intersects `visible`
    /// (a content-space vertical range), expanded by `cache_extent`
    /// pixels on either side and clamped to `0..item_count`.
    fn visible_range(&self, visible: (f32, f32), cache_extent: f32) -> std::ops::Range<usize> {
        if self.item_count() == 0 {
            return 0..0;
        }

        let (start, end) = visible;
        let expanded_start = (start - cache_extent).max(0.0);
        let expanded_end = (end + cache_extent).min(self.total_extent());

        if expanded_end <= expanded_start {
            return 0..0;
        }

        let first = self.index_at(expanded_start);
        let last = self.index_at(expanded_end.max(expanded_start));

        first..(last + 1).min(self.item_count())
    }
}

/// A recycled [`Tree`] for a previously-built item, kept around across
/// scroll positions so an item that leaves and later re-enters the
/// visible window (a small back-and-forth scroll) does not lose its
/// state (e.g. a `text_input`'s cursor) if it happens to be recycled
/// back into place, and so a genuinely new index reuses an
/// already-allocated [`Tree`] shell rather than starting from nothing.
struct Slot {
    index: usize,
    tree: Tree,
}

/// The internal, per-instance state of a [`VirtualList`]: its own scroll
/// offset (a [`VirtualList`] is a self-contained scrollable region, not
/// content meant to be wrapped in [`crate::Scrollable`] -- doing so would
/// defeat virtualization, since [`crate::Scrollable`] measures its content
/// at unbounded height) and the recycled [`Slot`]s from the last layout.
struct State {
    offset: f32,
    slots: Vec<Slot>,
    keyboard_modifiers: keyboard::Modifiers,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("offset", &self.offset)
            .field("slot_count", &self.slots.len())
            .finish()
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            offset: 0.0,
            slots: Vec::new(),
            keyboard_modifiers: keyboard::Modifiers::default(),
        }
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for VirtualList<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::default())
    }

    fn diff(&mut self, _tree: &mut Tree) {
        // Each visible item's `Tree` is diffed individually at `layout`
        // time (the only point `VirtualList` knows which indices are
        // actually visible); there is nothing uniform to diff here.
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
        let limits = limits.width(self.width).height(self.height);
        let size = limits.resolve(self.width, self.height, Size::ZERO);

        let positions = match &self.item_height {
            ItemHeight::Fixed(height) => Positions::fixed(self.item_count, *height),
            ItemHeight::Variable(f) => Positions::variable(self.item_count, f),
        };

        let state = tree.state.downcast_mut::<State>();

        let max_offset = (positions.total_extent() - size.height).max(0.0);
        state.offset = state.offset.clamp(0.0, max_offset);

        let visible = (state.offset, state.offset + size.height);
        let range = positions.visible_range(visible, self.cache_extent);

        let mut slots = std::mem::take(&mut state.slots);
        let mut new_slots = Vec::with_capacity(range.len());
        let mut nodes = Vec::with_capacity(range.len());

        for index in range {
            let mut element = (self.builder)(index);

            let mut item_tree = slots
                .iter()
                .position(|slot| slot.index == index)
                .map(|position| slots.remove(position).tree)
                .or_else(|| (!slots.is_empty()).then(|| slots.remove(0).tree))
                .unwrap_or_else(|| Tree::new(element.as_widget()));

            item_tree.diff(element.as_widget_mut());

            let item_limits = layout::Limits::new(
                Size::new(size.width, positions.height_of(index)),
                Size::new(size.width, positions.height_of(index)),
            );

            let node = element
                .as_widget_mut()
                .layout(&mut item_tree, renderer, &item_limits)
                .move_to(Point::new(0.0, positions.start_of(index) - state.offset));

            nodes.push(node);
            new_slots.push(Slot {
                index,
                tree: item_tree,
            });
        }

        // Anything left in `slots` scrolled out of the visible+cache
        // range this pass and is dropped for real -- keeping every slot
        // ever built forever would turn "bounded live widget count"
        // into an ever-growing cache, defeating virtualization's whole
        // memory guarantee.
        state.slots = new_slots;

        layout::Node::with_children(size, nodes)
    }

    fn visit_children(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        let state = tree.state.downcast_mut::<State>();

        for (slot, layout) in state.slots.iter_mut().zip(layout.children()) {
            let mut element = (self.builder)(slot.index);

            visit(&mut visit::Widgets::new(
                element.as_widget_mut(),
                &mut slot.tree,
                layout,
            ));
        }
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());

        let state = tree.state.downcast_mut::<State>();

        operation.traverse(&mut |operation| {
            for (slot, layout) in state.slots.iter_mut().zip(layout.children()) {
                let mut element = (self.builder)(slot.index);

                element
                    .as_widget_mut()
                    .operate(&mut slot.tree, layout, renderer, operation);
            }
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
        let bounds = layout.bounds();
        let state = tree.state.downcast_mut::<State>();

        if let Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) = event {
            state.keyboard_modifiers = *modifiers;
        }

        if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event {
            let is_over = cursor.is_over(bounds);

            if is_over {
                let dy = match *delta {
                    mouse::ScrollDelta::Lines { y, .. } => -y * 60.0,
                    mouse::ScrollDelta::Pixels { y, .. } => -y,
                };

                let previous = state.offset;

                // The real clamp happens in `layout` (it alone knows
                // `positions.total_extent()`); here we only need a
                // conservative, cheap guard so a huge one-shot delta
                // cannot temporarily store a wildly out-of-range value
                // that would flash empty content for a frame.
                state.offset = (state.offset + dy).max(0.0);

                if state.offset != previous {
                    shell.invalidate_layout();
                    shell.request_redraw();
                    shell.capture_event();
                }
            }
        }

        for (slot, layout) in state.slots.iter_mut().zip(layout.children()) {
            let mut element = (self.builder)(slot.index);

            element.as_widget_mut().update(
                &mut slot.tree,
                event,
                layout,
                cursor,
                renderer,
                shell,
                viewport,
            );

            if shell.is_event_captured() {
                return;
            }
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
        let state = tree.state.downcast_ref::<State>();

        state
            .slots
            .iter()
            .zip(layout.children())
            .map(|(slot, layout)| {
                let element = (self.builder)(slot.index);

                element
                    .as_widget()
                    .mouse_interaction(&slot.tree, layout, cursor, viewport, renderer)
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
        let bounds = layout.bounds();

        let Some(clipped_viewport) = bounds.intersection(viewport) else {
            return;
        };

        let state = tree.state.downcast_ref::<State>();

        renderer.with_layer(clipped_viewport, |renderer| {
            for (slot, layout) in state.slots.iter().zip(layout.children()) {
                if !layout.bounds().intersects(&clipped_viewport) {
                    continue;
                }

                let element = (self.builder)(slot.index);

                element.as_widget().draw(
                    &slot.tree,
                    renderer,
                    theme,
                    style,
                    layout,
                    cursor,
                    &clipped_viewport,
                );
            }
        });
    }

    // `Widget::overlay` is intentionally left at its default (`None`).
    //
    // The overlay API requires returning an `overlay::Element` that
    // borrows an `Element` living at least as long as the overlay
    // itself (see `overlay::from_children`'s signature, which borrows
    // from a caller-owned `&mut [Element]`). `VirtualList` never
    // persists the `Element`s its `builder` produces -- only their
    // `Tree`s survive between calls -- so there is no long-lived
    // `Element` storage an overlay could soundly borrow from without
    // changing that model. A future revision that caches built
    // `Element`s per visible slot (not just their `Tree`s) could close
    // this gap; until then, a visible item that itself wants to show an
    // overlay (a nested pick_list, a tooltip) will not have it appear
    // when placed inside a `VirtualList`, which is a documented
    // limitation rather than a silent correctness bug.
}

impl<'a, Message, Theme, Renderer> From<VirtualList<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: crate::core::Renderer + 'a,
{
    fn from(list: VirtualList<'a, Message, Theme, Renderer>) -> Self {
        Self::new(list)
    }
}

//! Distribute content on a grid.
use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::widget::visit;
use crate::core::widget::{self, Operation, Tree};
use crate::core::{Element, Event, Length, Pixels, Rectangle, Shell, Size, Vector, Widget};

/// A container that distributes its contents on a responsive grid.
pub struct Grid<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer> {
    spacing: f32,
    columns: Constraint,
    width: Option<Pixels>,
    height: Sizing,
    children: Vec<Element<'a, Message, Theme, Renderer>>,
}

/// The resolved geometry of a [`Grid`]'s cells for one particular
/// `available` [`Size`] and child count -- how many columns fit, how big
/// each cell is, and how many rows result.
///
/// Computing this is cheap ([`Grid::layout`] itself is `O(children)`
/// regardless), but it is still pure overhead to redo on every call when
/// nothing that could change it has changed; [`Grid`]'s internal
/// [`State`] caches the last [`Track`] it produced and reuses it
/// whenever `available`, `spacing`, `columns`, and child count are all
/// unchanged from the previous call.
///
/// [`Track`] also backs [`Grid::visible_range`]: any consumer that knows
/// which vertical range of a [`Grid`] is actually visible (e.g. an
/// ancestor `scrollable`) can map that range to a child-index range
/// without needing [`Grid::layout`] to have run first.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Track {
    cells_per_row: usize,
    cell_width: f32,
    cell_height: Option<f32>,
    total_rows: usize,
}

impl Track {
    /// Returns the vertical span, in the [`Grid`]'s own coordinate
    /// space, occupied by `row` (0-indexed), given a fallback row height
    /// for [`Sizing::AspectRatio`]-less variable-height rows.
    ///
    /// `row_height` is the height to assume for a row when [`Track`]
    /// itself has no fixed [`Track::cell_height`] (i.e.
    /// `Sizing::EvenlyDistribute(Length::Shrink)`, where every row's
    /// real height depends on its tallest child and cannot be known
    /// without laying it out) -- callers with only [`Track`] available
    /// (no measured children) should pass their best estimate.
    fn row_span(&self, row: usize, spacing: f32, row_height: f32) -> (f32, f32) {
        let height = self.cell_height.unwrap_or(row_height);
        let start = row as f32 * (height + spacing);

        (start, start + height)
    }
}

/// The internal, per-instance state of a [`Grid`]: a small cache of the
/// last [`Track`] computed, keyed on everything that can change it.
#[derive(Debug, Default)]
struct State {
    track: Option<(TrackKey, Track)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TrackKey {
    available_width_bits: u32,
    available_height_bits: u32,
    spacing_bits: u32,
    child_count: usize,
}

enum Constraint {
    MaxWidth(Pixels),
    Amount(usize),
}

impl<'a, Message, Theme, Renderer> Grid<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    /// Creates an empty [`Grid`].
    pub fn new() -> Self {
        Self::from_vec(Vec::new())
    }

    /// Creates a [`Grid`] with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self::from_vec(Vec::with_capacity(capacity))
    }

    /// Creates a [`Grid`] with the given elements.
    pub fn with_children(
        children: impl IntoIterator<Item = Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        let iterator = children.into_iter();

        Self::with_capacity(iterator.size_hint().0).extend(iterator)
    }

    /// Creates a [`Grid`] from an already allocated [`Vec`].
    pub fn from_vec(children: Vec<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            spacing: 0.0,
            columns: Constraint::Amount(3),
            width: None,
            height: Sizing::AspectRatio(1.0),
            children,
        }
    }

    /// Sets the spacing _between_ cells in the [`Grid`].
    pub fn spacing(mut self, amount: impl Into<Pixels>) -> Self {
        self.spacing = amount.into().0;
        self
    }

    /// Sets the width of the [`Grid`] in [`Pixels`].
    ///
    /// By default, a [`Grid`] will [`Fill`] its parent.
    ///
    /// [`Fill`]: Length::Fill
    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = Some(width.into());
        self
    }

    /// Sets the height of the [`Grid`].
    ///
    /// By default, a [`Grid`] uses a cell aspect ratio of `1.0` (i.e. squares).
    pub fn height(mut self, height: impl Into<Sizing>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the amount of columns in the [`Grid`].
    pub fn columns(mut self, column: usize) -> Self {
        self.columns = Constraint::Amount(column);
        self
    }

    /// Makes the amount of columns dynamic in the [`Grid`], never
    /// exceeding the provided `max_width`.
    pub fn fluid(mut self, max_width: impl Into<Pixels>) -> Self {
        self.columns = Constraint::MaxWidth(max_width.into());
        self
    }

    /// Adds an [`Element`] to the [`Grid`].
    pub fn push(mut self, child: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        self.children.push(child.into());
        self
    }

    /// Adds an element to the [`Grid`], if `Some`.
    pub fn push_maybe(
        self,
        child: Option<impl Into<Element<'a, Message, Theme, Renderer>>>,
    ) -> Self {
        if let Some(child) = child {
            self.push(child)
        } else {
            self
        }
    }

    /// Extends the [`Grid`] with the given children.
    pub fn extend(
        self,
        children: impl IntoIterator<Item = Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        children.into_iter().fold(self, Self::push)
    }
}

impl<Message, Renderer> Default for Grid<'_, Message, Renderer>
where
    Renderer: crate::core::Renderer,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, Message, Theme, Renderer: crate::core::Renderer>
    FromIterator<Element<'a, Message, Theme, Renderer>> for Grid<'a, Message, Theme, Renderer>
{
    fn from_iter<T: IntoIterator<Item = Element<'a, Message, Theme, Renderer>>>(iter: T) -> Self {
        Self::with_children(iter)
    }
}

impl<'a, Message, Theme, Renderer> Grid<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    /// Computes this [`Grid`]'s [`Track`] for the given `available` size,
    /// reusing `state`'s cached value when `available`, [`Self::spacing`],
    /// the column [`Constraint`], the cell [`Sizing`], and the child
    /// count are all unchanged from the call that produced it.
    ///
    /// Returns `None` when the grid has no children or no columns fit --
    /// the same "nothing to lay out" cases [`Widget::layout`] already
    /// special-cased before this cache existed.
    fn track(&self, state: &mut State, available: Size) -> Option<Track> {
        let key = TrackKey {
            available_width_bits: available.width.to_bits(),
            available_height_bits: available.height.to_bits(),
            spacing_bits: self.spacing.to_bits(),
            child_count: self.children.len(),
        };

        if let Some((cached_key, track)) = state.track
            && cached_key == key
        {
            return Some(track);
        }

        let cells_per_row = match self.columns {
            // width = n * (cell + spacing) - spacing, given n > 0
            Constraint::MaxWidth(pixels) => {
                ((available.width + self.spacing) / (pixels.0 + self.spacing)).ceil() as usize
            }
            Constraint::Amount(amount) => amount,
        };

        if self.children.is_empty() || cells_per_row == 0 {
            return None;
        }

        let cell_width =
            (available.width - self.spacing * (cells_per_row - 1) as f32) / cells_per_row as f32;

        let total_rows = self.children.len().div_ceil(cells_per_row);

        let cell_height = match self.height {
            Sizing::AspectRatio(ratio) => Some(cell_width / ratio),
            Sizing::EvenlyDistribute(Length::Shrink) => None,
            Sizing::EvenlyDistribute(_) => Some(
                (available.height - self.spacing * (total_rows - 1) as f32) / total_rows as f32,
            ),
        };

        let track = Track {
            cells_per_row,
            cell_width,
            cell_height,
            total_rows,
        };

        state.track = Some((key, track));

        Some(track)
    }

    /// Returns the range of child indices whose row intersects
    /// `visible`, a vertical range in the [`Grid`]'s own coordinate
    /// space (e.g. an ancestor `scrollable`'s viewport, translated into
    /// this grid's local coordinates).
    ///
    /// This lets a virtualizing ancestor decide which children are worth
    /// laying out/painting at all without needing this [`Grid`] to have
    /// computed a full [`layout::Node`] first -- it only needs
    /// [`Track`], which depends solely on the available width and the
    /// grid's own configuration, not on any child's content.
    ///
    /// Returns `0..self.children.len()` (i.e. "everything is visible")
    /// when the [`Track`] cannot be determined (e.g. before a first
    /// [`Widget::layout`] call has ever populated the cache and no
    /// `available` size can be supplied) -- a conservative answer that
    /// never wrongly hides a child, at the cost of not virtualizing
    /// anything until real geometry is known.
    pub fn visible_range(
        &self,
        tree: &mut Tree,
        available: Size,
        visible: (f32, f32),
        estimated_row_height: f32,
    ) -> std::ops::Range<usize> {
        let state = tree.state.downcast_mut::<State>();

        let Some(track) = self.track(state, available) else {
            return 0..self.children.len();
        };

        let (visible_start, visible_end) = visible;

        let first_row = (0..track.total_rows)
            .find(|&row| track.row_span(row, self.spacing, estimated_row_height).1 > visible_start)
            .unwrap_or(track.total_rows);

        let last_row = (0..track.total_rows)
            .rev()
            .find(|&row| track.row_span(row, self.spacing, estimated_row_height).0 < visible_end)
            .map_or(0, |row| row + 1);

        let start = first_row.saturating_mul(track.cells_per_row);
        let end = last_row
            .saturating_mul(track.cells_per_row)
            .min(self.children.len());

        start.min(self.children.len())..end.max(start.min(self.children.len()))
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Grid<'_, Message, Theme, Renderer>
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

        // See `Row::diff`'s identical rationale for clearing
        // unconditionally rather than trying to detect finer-grained
        // "did a child's content change" today.
        tree.state.downcast_mut::<State>().track = None;
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self
                .width
                .map(|pixels| Length::Fixed(pixels.0))
                .unwrap_or(Length::Fill),
            height: match self.height {
                Sizing::AspectRatio(_) => Length::Shrink,
                Sizing::EvenlyDistribute(length) => length,
            },
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let size = self.size();
        let limits = limits.width(size.width).height(size.height);
        let available = limits.max();

        if limits.compression().width && self.width.is_none() {
            return layout::Node::new(Size::ZERO);
        }

        let state = tree.state.downcast_mut::<State>();

        let Some(track) = self.track(state, available) else {
            return layout::Node::new(limits.resolve(size.width, size.height, Size::ZERO));
        };

        let Track {
            cells_per_row,
            cell_width,
            cell_height,
            ..
        } = track;

        let cell_limits = layout::Limits::new(
            Size::new(cell_width, cell_height.unwrap_or(0.0)),
            Size::new(cell_width, cell_height.unwrap_or(available.height)),
        );

        let mut nodes = Vec::with_capacity(self.children.len());
        let mut x = 0.0;
        let mut y = 0.0;
        let mut row_height = 0.0f32;

        for (i, (child, tree)) in self.children.iter_mut().zip(&mut tree.children).enumerate() {
            let node = child
                .as_widget_mut()
                .layout(tree, renderer, &cell_limits)
                .move_to((x, y));

            let size = node.size();

            x += size.width + self.spacing;
            row_height = row_height.max(size.height);

            if (i + 1) % cells_per_row == 0 {
                y += cell_height.unwrap_or(row_height) + self.spacing;
                x = 0.0;
                row_height = 0.0;
            }

            nodes.push(node);
        }

        if x == 0.0 {
            y -= self.spacing;
        } else {
            y += cell_height.unwrap_or(row_height);
        }

        layout::Node::with_children(Size::new(available.width, y), nodes)
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
        if let Some(viewport) = layout.bounds().intersection(viewport) {
            for ((child, tree), layout) in self
                .children
                .iter()
                .zip(&tree.children)
                .zip(layout.children())
                .filter(|(_, layout)| layout.bounds().intersects(&viewport))
            {
                child
                    .as_widget()
                    .draw(tree, renderer, theme, style, layout, cursor, &viewport);
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

impl<'a, Message, Theme, Renderer> From<Grid<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: crate::core::Renderer + 'a,
{
    fn from(row: Grid<'a, Message, Theme, Renderer>) -> Self {
        Self::new(row)
    }
}

/// The sizing strategy of a [`Grid`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sizing {
    /// The [`Grid`] will ensure each cell follows the given aspect ratio and the
    /// total size will be the sum of the cells and the spacing between them.
    ///
    /// The ratio is the amount of horizontal pixels per each vertical pixel of a cell
    /// in the [`Grid`].
    AspectRatio(f32),

    /// The [`Grid`] will evenly distribute the space available in the given [`Length`]
    /// for each cell.
    EvenlyDistribute(Length),
}

impl From<f32> for Sizing {
    fn from(height: f32) -> Self {
        Self::EvenlyDistribute(Length::from(height))
    }
}

impl From<Length> for Sizing {
    fn from(height: Length) -> Self {
        Self::EvenlyDistribute(height)
    }
}

/// Creates a new [`Sizing`] strategy that maintains the given aspect ratio.
pub fn aspect_ratio(width: impl Into<Pixels>, height: impl Into<Pixels>) -> Sizing {
    Sizing::AspectRatio(width.into().0 / height.into().0)
}

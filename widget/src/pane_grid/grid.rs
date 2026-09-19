use super::interaction::hovered_split;
use super::*;

/// A collection of panes distributed using either vertical or horizontal splits
/// to completely fill the space available.
///
///
/// This distribution of space is common in tiling window managers (like
/// [`awesome`](https://awesomewm.org/), [`i3`](https://i3wm.org/), or even
/// [`tmux`](https://github.com/tmux/tmux)).
///
/// A [`PaneGrid`] supports:
///
/// * Vertical and horizontal splits
/// * Tracking of the last active pane
/// * Mouse-based resizing
/// * Drag and drop to reorganize panes
/// * Hotkey support
/// * Configurable modifier keys
/// * [`State`] API to perform actions programmatically (`split`, `swap`, `resize`, etc.)
///
/// # Example
/// ```no_run
/// # mod taim { pub mod widget { pub use taim_widget::*; } pub use taim_widget::Renderer; pub use taim_widget::core::*; }
/// # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
/// #
/// use taim::widget::{pane_grid, text};
///
/// struct State {
///     panes: pane_grid::State<Pane>,
/// }
///
/// enum Pane {
///     SomePane,
///     AnotherKindOfPane,
/// }
///
/// enum Message {
///     PaneDragged(pane_grid::DragEvent),
///     PaneResized(pane_grid::ResizeEvent),
/// }
///
/// fn view(state: &State) -> Element<'_, Message> {
///     pane_grid(&state.panes, |pane, state, is_maximized| {
///         pane_grid::Content::new(match state {
///             Pane::SomePane => text("This is some pane"),
///             Pane::AnotherKindOfPane => text("This is another kind of pane"),
///         })
///     })
///     .on_drag(Message::PaneDragged)
///     .on_resize(10, Message::PaneResized)
///     .into()
/// }
/// ```
pub struct PaneGrid<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Theme: Catalog,
    Renderer: core::Renderer,
{
    pub(super) internal: &'a state::Internal,
    pub(super) panes: Vec<Pane>,
    pub(super) contents: Vec<Content<'a, Message, Theme, Renderer>>,
    pub(super) width: Length,
    pub(super) height: Length,
    pub(super) spacing: f32,
    pub(super) min_size: f32,
    pub(super) on_click: Option<Box<dyn Fn(Pane) -> Message + 'a>>,
    pub(super) on_drag: Option<Box<dyn Fn(DragEvent) -> Message + 'a>>,
    pub(super) on_resize: Option<(f32, Box<dyn Fn(ResizeEvent) -> Message + 'a>)>,
    pub(super) class: <Theme as Catalog>::Class<'a>,
}

impl<'a, Message, Theme, Renderer> PaneGrid<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: core::Renderer,
{
    /// Creates a [`PaneGrid`] with the given [`State`] and view function.
    ///
    /// The view function will be called to display each [`Pane`] present in the
    /// [`State`]. [`bool`] is set if the pane is maximized.
    pub fn new<T>(
        state: &'a State<T>,
        view: impl Fn(Pane, &'a T, bool) -> Content<'a, Message, Theme, Renderer>,
    ) -> Self {
        let panes = state.panes.keys().copied().collect();
        let maximized = state.maximized();
        let contents = state
            .panes
            .iter()
            .map(|(pane, pane_state)| match maximized {
                Some(p) if *pane == p => view(*pane, pane_state, true),
                _ => view(*pane, pane_state, false),
            })
            .collect();

        Self {
            internal: &state.internal,
            panes,
            contents,
            width: Length::Fill,
            height: Length::Fill,
            spacing: 0.0,
            min_size: 50.0,
            on_click: None,
            on_drag: None,
            on_resize: None,
            class: <Theme as Catalog>::default(),
        }
    }

    /// Sets the width of the [`PaneGrid`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`PaneGrid`].
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the spacing _between_ the panes of the [`PaneGrid`].
    pub fn spacing(mut self, amount: impl Into<Pixels>) -> Self {
        self.spacing = amount.into().0;
        self
    }

    /// Sets the minimum size of a [`Pane`] in the [`PaneGrid`] on both axes.
    pub fn min_size(mut self, min_size: impl Into<Pixels>) -> Self {
        self.min_size = min_size.into().0;
        self
    }

    /// Sets the message that will be produced when a [`Pane`] of the
    /// [`PaneGrid`] is clicked.
    pub fn on_click<F>(mut self, f: F) -> Self
    where
        F: 'a + Fn(Pane) -> Message,
    {
        self.on_click = Some(Box::new(f));
        self
    }

    /// Enables the drag and drop interactions of the [`PaneGrid`], which will
    /// use the provided function to produce messages.
    pub fn on_drag<F>(mut self, f: F) -> Self
    where
        F: 'a + Fn(DragEvent) -> Message,
    {
        if self.internal.maximized().is_none() {
            self.on_drag = Some(Box::new(f));
        }
        self
    }

    /// Enables the resize interactions of the [`PaneGrid`], which will
    /// use the provided function to produce messages.
    ///
    /// The `leeway` describes the amount of space around a split that can be
    /// used to grab it.
    ///
    /// The grabbable area of a split will have a length of `spacing + leeway`,
    /// properly centered. In other words, a length of
    /// `(spacing + leeway) / 2.0` on either side of the split line.
    pub fn on_resize<F>(mut self, leeway: impl Into<Pixels>, f: F) -> Self
    where
        F: 'a + Fn(ResizeEvent) -> Message,
    {
        if self.internal.maximized().is_none() {
            self.on_resize = Some((leeway.into().0, Box::new(f)));
        }
        self
    }

    /// Sets the style of the [`PaneGrid`].
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        <Theme as Catalog>::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style class of the [`PaneGrid`].
    #[cfg(feature = "advanced")]
    #[must_use]
    pub fn class(mut self, class: impl Into<<Theme as Catalog>::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    pub(super) fn drag_enabled(&self) -> bool {
        if self.internal.maximized().is_none() {
            self.on_drag.is_some()
        } else {
            Default::default()
        }
    }

    pub(super) fn grid_interaction(
        &self,
        action: &state::Action,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) -> Option<mouse::Interaction> {
        if action.picked_pane().is_some() {
            return Some(mouse::Interaction::Grabbing);
        }

        let resize_leeway = self.on_resize.as_ref().map(|(leeway, _)| *leeway);
        let node = self.internal.layout();

        let resize_axis = action.picked_split().map(|(_, axis)| axis).or_else(|| {
            resize_leeway.and_then(|leeway| {
                let cursor_position = cursor.position()?;
                let bounds = layout.bounds();

                let splits = node.split_regions(self.spacing, self.min_size, bounds.size());

                let relative_cursor =
                    Point::new(cursor_position.x - bounds.x, cursor_position.y - bounds.y);

                hovered_split(splits.iter(), self.spacing + leeway, relative_cursor)
                    .map(|(_, axis, _)| axis)
            })
        });

        if let Some(resize_axis) = resize_axis {
            return Some(match resize_axis {
                Axis::Horizontal => mouse::Interaction::ResizingVertically,
                Axis::Vertical => mouse::Interaction::ResizingHorizontally,
            });
        }

        None
    }
}

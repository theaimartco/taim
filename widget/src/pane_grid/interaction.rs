use super::*;

pub(super) fn layout_region(layout: Layout<'_>, cursor_position: Point) -> Option<Region> {
    let bounds = layout.bounds();

    if !bounds.contains(cursor_position) {
        return None;
    }

    let region = if cursor_position.x < (bounds.x + bounds.width / 3.0) {
        Region::Edge(Edge::Left)
    } else if cursor_position.x > (bounds.x + 2.0 * bounds.width / 3.0) {
        Region::Edge(Edge::Right)
    } else if cursor_position.y < (bounds.y + bounds.height / 3.0) {
        Region::Edge(Edge::Top)
    } else if cursor_position.y > (bounds.y + 2.0 * bounds.height / 3.0) {
        Region::Edge(Edge::Bottom)
    } else {
        Region::Center
    };

    Some(region)
}

pub(super) fn click_pane<'a, Message, T>(
    action: &mut state::Action,
    layout: Layout<'_>,
    cursor_position: Point,
    shell: &mut Shell<'_, Message>,
    contents: impl Iterator<Item = (Pane, T)>,
    on_click: &Option<Box<dyn Fn(Pane) -> Message + 'a>>,
    on_drag: &Option<Box<dyn Fn(DragEvent) -> Message + 'a>>,
) where
    T: Draggable,
{
    let mut clicked_region = contents
        .zip(layout.children())
        .filter(|(_, layout)| layout.bounds().contains(cursor_position));

    if let Some(((pane, content), layout)) = clicked_region.next() {
        if let Some(on_click) = &on_click {
            shell.publish(on_click(pane));
        }

        if let Some(on_drag) = &on_drag
            && content.can_be_dragged_at(layout, cursor_position)
        {
            *action = state::Action::Dragging {
                pane,
                origin: cursor_position,
            };

            shell.publish(on_drag(DragEvent::Picked { pane }));
        }
    }
}

pub(super) fn in_edge(layout: Layout<'_>, cursor: Point) -> Option<Edge> {
    let bounds = layout.bounds();

    let height_thickness = bounds.height / THICKNESS_RATIO;
    let width_thickness = bounds.width / THICKNESS_RATIO;
    let thickness = height_thickness.min(width_thickness);

    if cursor.x > bounds.x && cursor.x < bounds.x + thickness {
        Some(Edge::Left)
    } else if cursor.x > bounds.x + bounds.width - thickness && cursor.x < bounds.x + bounds.width {
        Some(Edge::Right)
    } else if cursor.y > bounds.y && cursor.y < bounds.y + thickness {
        Some(Edge::Top)
    } else if cursor.y > bounds.y + bounds.height - thickness && cursor.y < bounds.y + bounds.height
    {
        Some(Edge::Bottom)
    } else {
        None
    }
}

pub(super) fn edge_bounds(layout: Layout<'_>, edge: Edge) -> Rectangle {
    let bounds = layout.bounds();

    let height_thickness = bounds.height / THICKNESS_RATIO;
    let width_thickness = bounds.width / THICKNESS_RATIO;
    let thickness = height_thickness.min(width_thickness);

    match edge {
        Edge::Top => Rectangle {
            height: thickness,
            ..bounds
        },
        Edge::Left => Rectangle {
            width: thickness,
            ..bounds
        },
        Edge::Right => Rectangle {
            x: bounds.x + bounds.width - thickness,
            width: thickness,
            ..bounds
        },
        Edge::Bottom => Rectangle {
            y: bounds.y + bounds.height - thickness,
            height: thickness,
            ..bounds
        },
    }
}

pub(super) fn layout_region_bounds(layout: Layout<'_>, region: Region) -> Rectangle {
    let bounds = layout.bounds();

    match region {
        Region::Center => bounds,
        Region::Edge(edge) => match edge {
            Edge::Top => Rectangle {
                height: bounds.height / 2.0,
                ..bounds
            },
            Edge::Left => Rectangle {
                width: bounds.width / 2.0,
                ..bounds
            },
            Edge::Right => Rectangle {
                x: bounds.x + bounds.width / 2.0,
                width: bounds.width / 2.0,
                ..bounds
            },
            Edge::Bottom => Rectangle {
                y: bounds.y + bounds.height / 2.0,
                height: bounds.height / 2.0,
                ..bounds
            },
        },
    }
}

/// An event produced during a drag and drop interaction of a [`PaneGrid`].
#[derive(Debug, Clone, Copy)]
pub enum DragEvent {
    /// A [`Pane`] was picked for dragging.
    Picked {
        /// The picked [`Pane`].
        pane: Pane,
    },

    /// A [`Pane`] was dropped on top of another [`Pane`].
    Dropped {
        /// The picked [`Pane`].
        pane: Pane,

        /// The [`Target`] where the picked [`Pane`] was dropped on.
        target: Target,
    },

    /// A [`Pane`] was picked and then dropped outside of other [`Pane`]
    /// boundaries.
    Canceled {
        /// The picked [`Pane`].
        pane: Pane,
    },
}

/// The [`Target`] area a pane can be dropped on.
#[derive(Debug, Clone, Copy)]
pub enum Target {
    /// An [`Edge`] of the full [`PaneGrid`].
    Edge(Edge),
    /// A single [`Pane`] of the [`PaneGrid`].
    Pane(Pane, Region),
}

/// The region of a [`Pane`].
#[derive(Debug, Clone, Copy, Default)]
pub enum Region {
    /// Center region.
    #[default]
    Center,
    /// Edge region.
    Edge(Edge),
}

/// The edges of an area.
#[derive(Debug, Clone, Copy)]
pub enum Edge {
    /// Top edge.
    Top,
    /// Left edge.
    Left,
    /// Right edge.
    Right,
    /// Bottom edge.
    Bottom,
}

/// An event produced during a resize interaction of a [`PaneGrid`].
#[derive(Debug, Clone, Copy)]
pub struct ResizeEvent {
    /// The [`Split`] that is being dragged for resizing.
    pub split: Split,

    /// The new ratio of the [`Split`].
    ///
    /// The ratio is a value in [0, 1], representing the exact position of a
    /// [`Split`] between two panes.
    pub ratio: f32,
}

/*
 * Helpers
 */
pub(super) fn hovered_split<'a>(
    mut splits: impl Iterator<Item = (&'a Split, &'a (Axis, Rectangle, f32))>,
    spacing: f32,
    cursor_position: Point,
) -> Option<(Split, Axis, Rectangle)> {
    splits.find_map(|(split, (axis, region, ratio))| {
        let bounds = axis.split_line_bounds(*region, *ratio, spacing);

        if bounds.contains(cursor_position) {
            Some((*split, *axis, bounds))
        } else {
            None
        }
    })
}

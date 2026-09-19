use super::*;

pub(super) fn notify_scroll<Message>(
    state: &mut State,
    on_scroll: &Option<Box<dyn Fn(Viewport) -> Message + '_>>,
    bounds: Rectangle,
    content_bounds: Rectangle,
    shell: &mut Shell<'_, Message>,
) -> bool {
    if notify_viewport(state, on_scroll, bounds, content_bounds, shell) {
        state.last_scrolled = Some(Instant::now());

        true
    } else {
        false
    }
}

pub(super) fn notify_viewport<Message>(
    state: &mut State,
    on_scroll: &Option<Box<dyn Fn(Viewport) -> Message + '_>>,
    bounds: Rectangle,
    content_bounds: Rectangle,
    shell: &mut Shell<'_, Message>,
) -> bool {
    if content_bounds.width <= bounds.width && content_bounds.height <= bounds.height {
        return false;
    }

    let viewport = Viewport {
        offset_x: state.offset_x,
        offset_y: state.offset_y,
        bounds,
        content_bounds,
    };

    // Don't publish redundant viewports to shell
    if let Some(last_notified) = state.last_notified {
        let last_absolute_offset = last_notified.absolute_offset();
        let current_absolute_offset = viewport.absolute_offset();

        let unchanged =
            |a: f32, b: f32| (a - b).abs() <= f32::EPSILON || (a.is_nan() && b.is_nan());

        if last_notified.bounds == bounds
            && last_notified.content_bounds == content_bounds
            && unchanged(last_absolute_offset.x, current_absolute_offset.x)
            && unchanged(last_absolute_offset.y, current_absolute_offset.y)
        {
            return false;
        }
    }

    state.last_notified = Some(viewport);

    if let Some(on_scroll) = on_scroll {
        shell.publish(on_scroll(viewport));
    }

    true
}

#[derive(Debug, Clone, Copy)]
pub(super) struct State {
    pub(super) offset_y: Offset,
    pub(super) offset_x: Offset,
    pub(super) interaction: Interaction,
    pub(super) keyboard_modifiers: keyboard::Modifiers,
    pub(super) last_notified: Option<Viewport>,
    pub(super) last_scrolled: Option<Instant>,
    pub(super) is_scrollbar_visible: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Interaction {
    None,
    YScrollerGrabbed(f32),
    XScrollerGrabbed(f32),
    TouchScrolling(Point),
    AutoScrolling {
        origin: Point,
        current: Point,
        last_frame: Option<Instant>,
    },
}

impl Default for State {
    fn default() -> Self {
        Self {
            offset_y: Offset::Absolute(0.0),
            offset_x: Offset::Absolute(0.0),
            interaction: Interaction::None,
            keyboard_modifiers: keyboard::Modifiers::default(),
            last_notified: None,
            last_scrolled: None,
            is_scrollbar_visible: true,
        }
    }
}

impl operation::Scrollable for State {
    fn snap_to(&mut self, offset: RelativeOffset<Option<f32>>) {
        State::snap_to(self, offset);
    }

    fn scroll_to(&mut self, offset: AbsoluteOffset<Option<f32>>) {
        State::scroll_to(self, offset);
    }

    fn scroll_by(&mut self, offset: AbsoluteOffset, bounds: Rectangle, content_bounds: Rectangle) {
        State::scroll_by(self, offset, bounds, content_bounds);
    }

    fn adjust_offset(&mut self, delta: Vector, bounds: Rectangle, content_bounds: Rectangle) {
        State::adjust_offset(self, delta, bounds, content_bounds);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Offset {
    Absolute(f32),
    Relative(f32),
}

impl Offset {
    fn absolute(self, viewport: f32, content: f32) -> f32 {
        match self {
            Offset::Absolute(absolute) => absolute.min((content - viewport).max(0.0)),
            Offset::Relative(percentage) => ((content - viewport) * percentage).max(0.0),
        }
    }

    fn translation(self, viewport: f32, content: f32, alignment: Anchor) -> f32 {
        let offset = self.absolute(viewport, content);

        match alignment {
            Anchor::Start => offset,
            Anchor::End => ((content - viewport).max(0.0) - offset).max(0.0),
        }
    }
}

/// The current [`Viewport`] of the [`Scrollable`].
#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    offset_x: Offset,
    offset_y: Offset,
    bounds: Rectangle,
    content_bounds: Rectangle,
}

impl Viewport {
    /// Returns the [`AbsoluteOffset`] of the current [`Viewport`].
    pub fn absolute_offset(&self) -> AbsoluteOffset {
        let x = self
            .offset_x
            .absolute(self.bounds.width, self.content_bounds.width);
        let y = self
            .offset_y
            .absolute(self.bounds.height, self.content_bounds.height);

        AbsoluteOffset { x, y }
    }

    /// Returns the [`AbsoluteOffset`] of the current [`Viewport`], but with its
    /// alignment reversed.
    ///
    /// This method can be useful to switch the alignment of a [`Scrollable`]
    /// while maintaining its scrolling position.
    pub fn absolute_offset_reversed(&self) -> AbsoluteOffset {
        let AbsoluteOffset { x, y } = self.absolute_offset();

        AbsoluteOffset {
            x: (self.content_bounds.width - self.bounds.width).max(0.0) - x,
            y: (self.content_bounds.height - self.bounds.height).max(0.0) - y,
        }
    }

    /// Returns the [`RelativeOffset`] of the current [`Viewport`].
    pub fn relative_offset(&self) -> RelativeOffset {
        let AbsoluteOffset { x, y } = self.absolute_offset();

        let x = x / (self.content_bounds.width - self.bounds.width);
        let y = y / (self.content_bounds.height - self.bounds.height);

        RelativeOffset { x, y }
    }

    /// Returns the bounds of the current [`Viewport`].
    pub fn bounds(&self) -> Rectangle {
        self.bounds
    }

    /// Returns the content bounds of the current [`Viewport`].
    pub fn content_bounds(&self) -> Rectangle {
        self.content_bounds
    }

    /// Returns the currently visible region, expressed in the content's
    /// own local coordinate space (i.e. `(0, 0)` is the content's own
    /// top-left corner, regardless of where the [`Scrollable`] itself
    /// sits on screen).
    ///
    /// This is the primitive a virtualizing child (e.g. a
    /// `virtual_list`) needs to decide which of its rows are actually
    /// visible: it can intersect its own row geometry against this
    /// [`Rectangle`] without needing to know anything about the
    /// [`Scrollable`] wrapping it, or about screen-space coordinates at
    /// all.
    pub fn visible_bounds(&self) -> Rectangle {
        let AbsoluteOffset { x, y } = self.absolute_offset();

        Rectangle {
            x,
            y,
            width: self.bounds.width,
            height: self.bounds.height,
        }
    }
}

impl State {
    pub(super) fn new() -> Self {
        State::default()
    }

    pub(super) fn scroll(
        &mut self,
        delta: Vector<f32>,
        bounds: Rectangle,
        content_bounds: Rectangle,
    ) {
        if bounds.height < content_bounds.height {
            self.offset_y = Offset::Absolute(
                (self.offset_y.absolute(bounds.height, content_bounds.height) + delta.y)
                    .clamp(0.0, content_bounds.height - bounds.height),
            );
        }

        if bounds.width < content_bounds.width {
            self.offset_x = Offset::Absolute(
                (self.offset_x.absolute(bounds.width, content_bounds.width) + delta.x)
                    .clamp(0.0, content_bounds.width - bounds.width),
            );
        }
    }

    /// Returns `true` if applying `delta` via [`State::scroll`] would
    /// actually change the current offset in at least one axis, given
    /// `bounds` and `content_bounds`.
    ///
    /// This is the hook a nested [`Scrollable`] uses to decide whether a
    /// wheel/touch event that it cannot act on (because it is already at
    /// the boundary in the requested direction) should still be treated
    /// as "handled" -- an event this method reports as unable to move
    /// anything should be released to an ancestor scrollable instead of
    /// being captured, so scrolling past a nested scrollable's limit
    /// naturally continues scrolling its parent.
    pub(super) fn can_scroll(
        &self,
        delta: Vector<f32>,
        bounds: Rectangle,
        content_bounds: Rectangle,
    ) -> bool {
        let can_scroll_y = bounds.height < content_bounds.height && {
            let current = self.offset_y.absolute(bounds.height, content_bounds.height);
            let max = content_bounds.height - bounds.height;

            (delta.y < 0.0 && current > 0.0) || (delta.y > 0.0 && current < max)
        };

        let can_scroll_x = bounds.width < content_bounds.width && {
            let current = self.offset_x.absolute(bounds.width, content_bounds.width);
            let max = content_bounds.width - bounds.width;

            (delta.x < 0.0 && current > 0.0) || (delta.x > 0.0 && current < max)
        };

        can_scroll_y || can_scroll_x
    }

    pub(super) fn scroll_y_to(
        &mut self,
        percentage: f32,
        bounds: Rectangle,
        content_bounds: Rectangle,
    ) {
        self.offset_y = Offset::Relative(percentage.clamp(0.0, 1.0));
        self.unsnap(bounds, content_bounds);
    }

    pub(super) fn scroll_x_to(
        &mut self,
        percentage: f32,
        bounds: Rectangle,
        content_bounds: Rectangle,
    ) {
        self.offset_x = Offset::Relative(percentage.clamp(0.0, 1.0));
        self.unsnap(bounds, content_bounds);
    }

    pub(super) fn snap_to(&mut self, offset: RelativeOffset<Option<f32>>) {
        if let Some(x) = offset.x {
            self.offset_x = Offset::Relative(x.clamp(0.0, 1.0));
        }

        if let Some(y) = offset.y {
            self.offset_y = Offset::Relative(y.clamp(0.0, 1.0));
        }
    }

    pub(super) fn scroll_to(&mut self, offset: AbsoluteOffset<Option<f32>>) {
        if let Some(x) = offset.x {
            self.offset_x = Offset::Absolute(x.max(0.0));
        }

        if let Some(y) = offset.y {
            self.offset_y = Offset::Absolute(y.max(0.0));
        }
    }

    /// Scroll by the provided [`AbsoluteOffset`].
    pub(super) fn scroll_by(
        &mut self,
        offset: AbsoluteOffset,
        bounds: Rectangle,
        content_bounds: Rectangle,
    ) {
        self.scroll(Vector::new(offset.x, offset.y), bounds, content_bounds);
    }

    /// Shifts the current offset by `delta` without going through
    /// [`State::scroll`]'s user-scroll semantics: unlike
    /// [`State::scroll_by`] (a wheel/touch-shaped movement subject to the
    /// same clamping a real scroll gesture would see), this always
    /// normalizes to [`Offset::Absolute`] first, so a
    /// [`Offset::Relative`] (percentage-based) offset -- which would
    /// otherwise silently reinterpret the same percentage against the
    /// new `content_bounds`, producing a different pixel position than
    /// intended -- is pinned to its current pixel position before the
    /// correction is applied.
    pub(super) fn adjust_offset(
        &mut self,
        delta: Vector,
        bounds: Rectangle,
        content_bounds: Rectangle,
    ) {
        self.offset_x =
            Offset::Absolute(self.offset_x.absolute(bounds.width, content_bounds.width));
        self.offset_y =
            Offset::Absolute(self.offset_y.absolute(bounds.height, content_bounds.height));

        self.scroll(delta, bounds, content_bounds);
    }

    /// Unsnaps the current scroll position, if snapped, given the bounds of the
    /// [`Scrollable`] and its contents.
    pub(super) fn unsnap(&mut self, bounds: Rectangle, content_bounds: Rectangle) {
        self.offset_x =
            Offset::Absolute(self.offset_x.absolute(bounds.width, content_bounds.width));
        self.offset_y =
            Offset::Absolute(self.offset_y.absolute(bounds.height, content_bounds.height));
    }

    /// Returns the scrolling translation of the [`State`], given a [`Direction`],
    /// the bounds of the [`Scrollable`] and its contents.
    pub(super) fn translation(
        &self,
        direction: Direction,
        bounds: Rectangle,
        content_bounds: Rectangle,
    ) -> Vector {
        Vector::new(
            if let Some(horizontal) = direction.horizontal() {
                self.offset_x
                    .translation(bounds.width, content_bounds.width, horizontal.alignment)
                    .round()
            } else {
                0.0
            },
            if let Some(vertical) = direction.vertical() {
                self.offset_y
                    .translation(bounds.height, content_bounds.height, vertical.alignment)
                    .round()
            } else {
                0.0
            },
        )
    }

    pub(super) fn scrollers_grabbed(&self) -> bool {
        matches!(
            self.interaction,
            Interaction::YScrollerGrabbed(_) | Interaction::XScrollerGrabbed(_),
        )
    }

    pub fn y_scroller_grabbed_at(&self) -> Option<f32> {
        let Interaction::YScrollerGrabbed(at) = self.interaction else {
            return None;
        };

        Some(at)
    }

    pub fn x_scroller_grabbed_at(&self) -> Option<f32> {
        let Interaction::XScrollerGrabbed(at) = self.interaction else {
            return None;
        };

        Some(at)
    }
}

#[derive(Debug)]
/// State of both [`Scrollbar`]s.
pub(super) struct Scrollbars {
    pub(super) y: Option<internals::Scrollbar>,
    pub(super) x: Option<internals::Scrollbar>,
}

impl Scrollbars {
    /// Create y and/or x scrollbar(s) if content is overflowing the [`Scrollable`] bounds.
    pub(super) fn new(
        state: &State,
        direction: Direction,
        bounds: Rectangle,
        content_bounds: Rectangle,
    ) -> Self {
        let translation = state.translation(direction, bounds, content_bounds);

        let show_scrollbar_x = direction
            .horizontal()
            .filter(|_scrollbar| content_bounds.width > bounds.width);

        let show_scrollbar_y = direction
            .vertical()
            .filter(|_scrollbar| content_bounds.height > bounds.height);

        let y_scrollbar = if let Some(vertical) = show_scrollbar_y {
            let Scrollbar {
                width,
                margin,
                scroller_width,
                ..
            } = *vertical;

            // Adjust the height of the vertical scrollbar if the horizontal scrollbar
            // is present
            let x_scrollbar_height =
                show_scrollbar_x.map_or(0.0, |h| h.width.max(h.scroller_width) + h.margin);

            let total_scrollbar_width = width.max(scroller_width) + 2.0 * margin;

            // Total bounds of the scrollbar + margin + scroller width
            let total_scrollbar_bounds = Rectangle {
                x: bounds.x + bounds.width - total_scrollbar_width,
                y: bounds.y,
                width: total_scrollbar_width,
                height: (bounds.height - x_scrollbar_height).max(0.0),
            };

            // Bounds of just the scrollbar
            let scrollbar_bounds = Rectangle {
                x: bounds.x + bounds.width - total_scrollbar_width / 2.0 - width / 2.0,
                y: bounds.y,
                width,
                height: (bounds.height - x_scrollbar_height).max(0.0),
            };

            let ratio = bounds.height / content_bounds.height;

            let scroller = if ratio >= 1.0 {
                None
            } else {
                // min height for easier grabbing with super tall content
                let scroller_height = (scrollbar_bounds.height * ratio).max(2.0);
                let scroller_offset =
                    translation.y * ratio * scrollbar_bounds.height / bounds.height;

                let scroller_bounds = Rectangle {
                    x: bounds.x + bounds.width - total_scrollbar_width / 2.0 - scroller_width / 2.0,
                    y: (scrollbar_bounds.y + scroller_offset).max(0.0),
                    width: scroller_width,
                    height: scroller_height,
                };

                Some(internals::Scroller {
                    bounds: scroller_bounds,
                })
            };

            Some(internals::Scrollbar {
                total_bounds: total_scrollbar_bounds,
                bounds: scrollbar_bounds,
                scroller,
                alignment: vertical.alignment,
                disabled: content_bounds.height <= bounds.height,
            })
        } else {
            None
        };

        let x_scrollbar = if let Some(horizontal) = show_scrollbar_x {
            let Scrollbar {
                width,
                margin,
                scroller_width,
                ..
            } = *horizontal;

            // Need to adjust the width of the horizontal scrollbar if the vertical scrollbar
            // is present
            let scrollbar_y_width =
                y_scrollbar.map_or(0.0, |scrollbar| scrollbar.total_bounds.width);

            let total_scrollbar_height = width.max(scroller_width) + 2.0 * margin;

            // Total bounds of the scrollbar + margin + scroller width
            let total_scrollbar_bounds = Rectangle {
                x: bounds.x,
                y: bounds.y + bounds.height - total_scrollbar_height,
                width: (bounds.width - scrollbar_y_width).max(0.0),
                height: total_scrollbar_height,
            };

            // Bounds of just the scrollbar
            let scrollbar_bounds = Rectangle {
                x: bounds.x,
                y: bounds.y + bounds.height - total_scrollbar_height / 2.0 - width / 2.0,
                width: (bounds.width - scrollbar_y_width).max(0.0),
                height: width,
            };

            let ratio = bounds.width / content_bounds.width;

            let scroller = if ratio >= 1.0 {
                None
            } else {
                // min width for easier grabbing with extra wide content
                let scroller_length = (scrollbar_bounds.width * ratio).max(2.0);
                let scroller_offset = translation.x * ratio * scrollbar_bounds.width / bounds.width;

                let scroller_bounds = Rectangle {
                    x: (scrollbar_bounds.x + scroller_offset).max(0.0),
                    y: bounds.y + bounds.height
                        - total_scrollbar_height / 2.0
                        - scroller_width / 2.0,
                    width: scroller_length,
                    height: scroller_width,
                };

                Some(internals::Scroller {
                    bounds: scroller_bounds,
                })
            };

            Some(internals::Scrollbar {
                total_bounds: total_scrollbar_bounds,
                bounds: scrollbar_bounds,
                scroller,
                alignment: horizontal.alignment,
                disabled: content_bounds.width <= bounds.width,
            })
        } else {
            None
        };

        Self {
            y: y_scrollbar,
            x: x_scrollbar,
        }
    }

    pub(super) fn is_mouse_over(&self, cursor: mouse::Cursor) -> (bool, bool) {
        if let Some(cursor_position) = cursor.position() {
            (
                self.y
                    .as_ref()
                    .map(|scrollbar| scrollbar.is_mouse_over(cursor_position))
                    .unwrap_or(false),
                self.x
                    .as_ref()
                    .map(|scrollbar| scrollbar.is_mouse_over(cursor_position))
                    .unwrap_or(false),
            )
        } else {
            (false, false)
        }
    }

    pub(super) fn is_y_disabled(&self) -> bool {
        self.y.map(|y| y.disabled).unwrap_or(false)
    }

    pub(super) fn is_x_disabled(&self) -> bool {
        self.x.map(|x| x.disabled).unwrap_or(false)
    }

    pub(super) fn grab_y_scroller(&self, cursor_position: Point) -> Option<f32> {
        let scrollbar = self.y?;
        let scroller = scrollbar.scroller?;

        if scrollbar.total_bounds.contains(cursor_position) {
            Some(if scroller.bounds.contains(cursor_position) {
                (cursor_position.y - scroller.bounds.y) / scroller.bounds.height
            } else {
                0.5
            })
        } else {
            None
        }
    }

    pub(super) fn grab_x_scroller(&self, cursor_position: Point) -> Option<f32> {
        let scrollbar = self.x?;
        let scroller = scrollbar.scroller?;

        if scrollbar.total_bounds.contains(cursor_position) {
            Some(if scroller.bounds.contains(cursor_position) {
                (cursor_position.x - scroller.bounds.x) / scroller.bounds.width
            } else {
                0.5
            })
        } else {
            None
        }
    }

    pub(super) fn active(&self) -> bool {
        self.y.is_some() || self.x.is_some()
    }
}

pub(super) mod internals {
    use crate::core::{Point, Rectangle};

    use super::Anchor;

    #[derive(Debug, Copy, Clone)]
    pub struct Scrollbar {
        pub total_bounds: Rectangle,
        pub bounds: Rectangle,
        pub scroller: Option<Scroller>,
        pub alignment: Anchor,
        pub disabled: bool,
    }

    impl Scrollbar {
        /// Returns whether the mouse is over the scrollbar or not.
        pub fn is_mouse_over(&self, cursor_position: Point) -> bool {
            self.total_bounds.contains(cursor_position)
        }

        /// Returns the y-axis scrolled percentage from the cursor position.
        pub fn scroll_percentage_y(&self, grabbed_at: f32, cursor_position: Point) -> f32 {
            if let Some(scroller) = self.scroller {
                let percentage =
                    (cursor_position.y - self.bounds.y - scroller.bounds.height * grabbed_at)
                        / (self.bounds.height - scroller.bounds.height);

                match self.alignment {
                    Anchor::Start => percentage,
                    Anchor::End => 1.0 - percentage,
                }
            } else {
                0.0
            }
        }

        /// Returns the x-axis scrolled percentage from the cursor position.
        pub fn scroll_percentage_x(&self, grabbed_at: f32, cursor_position: Point) -> f32 {
            if let Some(scroller) = self.scroller {
                let percentage =
                    (cursor_position.x - self.bounds.x - scroller.bounds.width * grabbed_at)
                        / (self.bounds.width - scroller.bounds.width);

                match self.alignment {
                    Anchor::Start => percentage,
                    Anchor::End => 1.0 - percentage,
                }
            } else {
                0.0
            }
        }
    }

    /// The handle of a [`Scrollbar`].
    #[derive(Debug, Clone, Copy)]
    pub struct Scroller {
        /// The bounds of the [`Scroller`].
        pub bounds: Rectangle,
    }
}

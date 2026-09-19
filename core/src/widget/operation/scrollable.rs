//! Operate on widgets that can be scrolled.
use crate::widget::{Id, Operation};
use crate::{Rectangle, Vector};

/// The internal state of a widget that can be scrolled.
pub trait Scrollable {
    /// Snaps the scroll of the widget to the given `percentage` along the horizontal & vertical axis.
    fn snap_to(&mut self, offset: RelativeOffset<Option<f32>>);

    /// Scroll the widget to the given [`AbsoluteOffset`] along the horizontal & vertical axis.
    fn scroll_to(&mut self, offset: AbsoluteOffset<Option<f32>>);

    /// Scroll the widget by the given [`AbsoluteOffset`] along the horizontal & vertical axis.
    fn scroll_by(&mut self, offset: AbsoluteOffset, bounds: Rectangle, content_bounds: Rectangle);

    /// Shifts the current scroll position by `delta` **without** treating
    /// it as a user-initiated scroll: no `on_scroll` callback fires, and
    /// no scroll transaction/snapping state is disturbed.
    ///
    /// This is the hook content that changes size *above* or *before*
    /// the current viewport (most notably a virtualized list inserting
    /// or removing rows above the visible window) uses to keep the same
    /// content under the viewport across the resize -- a plain
    /// [`Scrollable::scroll_by`] would be indistinguishable from the
    /// user actually scrolling, which is semantically wrong for a
    /// correction the content made on the user's behalf.
    fn adjust_offset(&mut self, delta: Vector, bounds: Rectangle, content_bounds: Rectangle);
}

/// Produces an [`Operation`] that snaps the widget with the given [`Id`] to
/// the provided `percentage`.
pub fn snap_to<T>(target: Id, offset: RelativeOffset<Option<f32>>) -> impl Operation<T> {
    TargetScroll::new(target, ScrollCommand::Snap(offset))
}

/// Produces an [`Operation`] that scrolls the widget with the given [`Id`] to
/// the provided [`AbsoluteOffset`].
pub fn scroll_to<T>(target: Id, offset: AbsoluteOffset<Option<f32>>) -> impl Operation<T> {
    TargetScroll::new(target, ScrollCommand::To(offset))
}

/// Produces an [`Operation`] that scrolls the widget with the given [`Id`] by
/// the provided [`AbsoluteOffset`].
pub fn scroll_by<T>(target: Id, offset: AbsoluteOffset) -> impl Operation<T> {
    TargetScroll::new(target, ScrollCommand::By(offset))
}

/// Produces an [`Operation`] that shifts the scroll position of the
/// widget with the given [`Id`] by `delta`, without treating it as a
/// user-initiated scroll (see [`Scrollable::adjust_offset`]).
pub fn adjust_offset<T>(target: Id, delta: Vector) -> impl Operation<T> {
    TargetScroll::new(target, ScrollCommand::Adjust(delta))
}

#[derive(Debug, Clone, Copy)]
enum ScrollCommand {
    Snap(RelativeOffset<Option<f32>>),
    To(AbsoluteOffset<Option<f32>>),
    By(AbsoluteOffset),
    Adjust(Vector),
}

/// One compact traversal implementation for every targeted scroll mutation.
/// This keeps one virtual-dispatch shape in the instruction cache while still
/// applying to every matching custom ID, preserving the public contract.
struct TargetScroll {
    target: Id,
    command: ScrollCommand,
}

impl TargetScroll {
    fn new(target: Id, command: ScrollCommand) -> Self {
        Self { target, command }
    }
}

impl<T> Operation<T> for TargetScroll {
    fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation<T>)) {
        visit(self);
    }

    fn scrollable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        content_bounds: Rectangle,
        _translation: Vector,
        state: &mut dyn Scrollable,
    ) {
        if id != Some(&self.target) {
            return;
        }

        match self.command {
            ScrollCommand::Snap(offset) => state.snap_to(offset),
            ScrollCommand::To(offset) => state.scroll_to(offset),
            ScrollCommand::By(offset) => state.scroll_by(offset, bounds, content_bounds),
            ScrollCommand::Adjust(delta) => {
                state.adjust_offset(delta, bounds, content_bounds);
            }
        }
    }
}

/// The amount of absolute offset in each direction of a [`Scrollable`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AbsoluteOffset<T = f32> {
    /// The amount of horizontal offset
    pub x: T,
    /// The amount of vertical offset
    pub y: T,
}

impl From<AbsoluteOffset> for AbsoluteOffset<Option<f32>> {
    fn from(offset: AbsoluteOffset) -> Self {
        Self {
            x: Some(offset.x),
            y: Some(offset.y),
        }
    }
}

/// The amount of relative offset in each direction of a [`Scrollable`].
///
/// A value of `0.0` means start, while `1.0` means end.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RelativeOffset<T = f32> {
    /// The amount of horizontal offset
    pub x: T,
    /// The amount of vertical offset
    pub y: T,
}

impl RelativeOffset {
    /// A relative offset that points to the top-left of a [`Scrollable`].
    pub const START: Self = Self { x: 0.0, y: 0.0 };

    /// A relative offset that points to the bottom-right of a [`Scrollable`].
    pub const END: Self = Self { x: 1.0, y: 1.0 };
}

impl From<RelativeOffset> for RelativeOffset<Option<f32>> {
    fn from(offset: RelativeOffset) -> Self {
        Self {
            x: Some(offset.x),
            y: Some(offset.y),
        }
    }
}

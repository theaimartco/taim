//! A scrolling list that builds only the items near the screen.
use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::{Constraints, Sizing};
use crate::state::{self, IntoProp, Invalidation, Prop};
use crate::views::hero::{self, Stage};
use crate::views::scroll::Scroller;
use crate::{AnyView, Child, Event, Handled, Instant, Point, Size, Vector, View, semantics};

use taim_core::widget;

use std::cell::Cell;
use std::sync::Arc;

/// The height assumed for an item that has never been built, until some
/// have been and their average is known.
const ESTIMATE: f32 = 48.0;

/// A vertical, scrolling list of `count` items, built by `item(index)` only
/// while they are on screen or close to it, like Flutter's
/// `ListView.builder` or Android's `RecyclerView`.
///
/// A list of a hundred thousand items costs what the visible ones cost.
/// Items scrolled far away are dropped and built again when they come
/// back, so keep an item's state in the application (a state handle the
/// item reads), not in the item. Items may have different heights; ones
/// not yet built are assumed to be as tall as the average of those that
/// were, or [`List::item_height`] if given.
///
/// It scrolls like [`scroll`](super::scroll): wheel, finger drag and fling.
///
/// ```ignore
/// let messages = cx.signal(load_messages());
/// list(move || messages.with(Vec::len), move |index| {
///     text(move || messages.with(|all| all[index].subject.clone())).into()
/// })
/// ```
pub fn list(count: impl IntoProp<usize>, item: impl Fn(usize) -> AnyView + 'static) -> List {
    List {
        count: count.into_prop(),
        item: Box::new(item),
        built: Vec::new(),
        heights: Vec::new(),
        measured: (0.0, 0),
        hint: None,
        gap: 0.0,
        scroller: Scroller::relayouting(),
        viewport: Size::ZERO,
        covered: Cell::new((0.0, 0.0)),
        id: widget::Id::unique(),
        window: state::building(),
        stage: hero::current(),
    }
}

/// See [`list`].
pub struct List {
    count: Prop<usize>,
    item: Box<dyn Fn(usize) -> AnyView>,
    /// The built items, by index, in order and contiguous.
    built: Vec<(usize, Child)>,
    /// Each item's measured height; `NAN` until it has been built.
    heights: Vec<f32>,
    /// The sum and number of measured heights, for the estimate.
    measured: (f64, u32),
    hint: Option<f32>,
    gap: f32,
    scroller: Scroller,
    viewport: Size,
    /// The span of content offsets the built items cover.
    covered: Cell<(f32, f32)>,
    id: widget::Id,
    window: Option<Arc<Invalidation>>,
    stage: Option<Stage>,
}

impl List {
    /// Space between items.
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    /// The height of items that have not been built yet. Giving the real
    /// height of uniform items keeps the scroll bar exact.
    pub fn item_height(mut self, height: f32) -> Self {
        if height.is_finite() && height > 0.0 {
            self.hint = Some(height);
        }
        self
    }

    fn estimate(&self) -> f32 {
        if let Some(hint) = self.hint {
            return hint;
        }

        let (sum, count) = self.measured;
        if count == 0 {
            ESTIMATE
        } else {
            (sum / f64::from(count)) as f32
        }
    }

    /// Records item `index`'s measured height.
    fn record(&mut self, index: usize, height: f32) {
        let previous = std::mem::replace(&mut self.heights[index], height);
        if !previous.is_nan() {
            self.measured.0 -= f64::from(previous);
            self.measured.1 -= 1;
        }
        self.measured.0 += f64::from(height);
        self.measured.1 += 1;
    }

    /// Whether the visible span has scrolled past what the built items
    /// cover, so the list must be laid out (and items built) again.
    fn uncovered(&self) -> bool {
        let (top, bottom) = self.covered.get();
        let offset = self.scroller.offset();
        let total = self.scroller.max_offset() + self.viewport.height;

        (offset < top && top > 0.0) || (offset + self.viewport.height > bottom && bottom < total)
    }
}

impl View for List {
    fn sizing(&self) -> Sizing {
        Sizing::FILL
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let outer = constraints.with_sizing(Sizing::FILL);
        let width = if outer.max.width.is_finite() {
            outer.max.width
        } else {
            0.0
        };
        // An unbounded height cannot be virtualized; show a screenful.
        let height = if outer.max.height.is_finite() {
            outer.max.height
        } else {
            800.0
        };
        self.viewport = Size::new(width, height);

        let count = self.count.get();
        if count < self.heights.len() {
            self.heights.truncate(count);
            self.measured = self
                .heights
                .iter()
                .filter(|height| !height.is_nan())
                .fold((0.0, 0), |(sum, n), height| {
                    (sum + f64::from(*height), n + 1)
                });
        }
        self.heights.resize(count, f32::NAN);
        self.built.retain(|(index, _)| *index < count);

        let estimate = self.estimate();
        let gap = self.gap;
        let height_of = |heights: &[f32], index: usize| {
            let height = heights[index];
            if height.is_nan() { estimate } else { height }
        };

        // Build a screen above and below what shows, so a fling has items
        // ready before they arrive.
        let offset = self.scroller.offset();
        let low = offset - height;
        let high = offset + 2.0 * height;

        let mut first = None;
        let mut last = None;
        let mut top = 0.0_f32;
        for index in 0..count {
            let bottom = top + height_of(&self.heights, index);
            if bottom >= low && top <= high {
                let _ = first.get_or_insert(index);
                last = Some(index);
            } else if top > high {
                break;
            }
            top = bottom + gap;
        }

        let mut previous = std::mem::take(&mut self.built).into_iter().peekable();
        let mut built = Vec::new();

        if let (Some(first), Some(last)) = (first, last) {
            let room = Constraints::loose(Size::new(width, f32::INFINITY));

            for index in first..=last {
                // Reuse the item if it was built; drop ones before it.
                while previous.peek().is_some_and(|(built, _)| *built < index) {
                    let _ = previous.next();
                }
                let mut child = match previous.peek() {
                    Some((built, _)) if *built == index => {
                        previous.next().map(|(_, child)| child).expect("peeked")
                    }
                    _ => Child::new(state::rebuilding(self.window.as_ref(), || {
                        hero::restaged(self.stage.as_ref(), || (self.item)(index))
                    })),
                };

                let size = child.measure(cx, room);
                self.record(index, size.height);
                built.push((index, child));
            }
        }
        drop(previous);

        // Place with the heights just measured.
        let estimate = self.estimate();
        let height_of = |heights: &[f32], index: usize| {
            let height = heights[index];
            if height.is_nan() { estimate } else { height }
        };
        let mut top = 0.0_f32;
        let mut next = built.iter_mut().peekable();
        let mut covered: Option<(f32, f32)> = None;
        for index in 0..count {
            let item = height_of(&self.heights, index);
            if let Some((built, child)) = next.peek_mut()
                && *built == index
            {
                child.place(Point::new(0.0, top));
                let start = covered.map_or(top, |(start, _)| start);
                covered = Some((start, top + item));
                let _ = next.next();
            }
            top += item + if index + 1 < count { gap } else { 0.0 };
        }

        self.built = built;
        self.covered.set(covered.unwrap_or((0.0, 0.0)));
        self.scroller.set_extent(top, height);
        self.viewport
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        // A fling moves the list between layouts; when it reaches items
        // that were not built, ask for a layout to build them.
        if self.scroller.advance(cx.now())
            && self.uncovered()
            && let Some(window) = &self.window
        {
            window.mark(Invalidation::LAYOUT);
        }
    }

    fn paint_over(&self, cx: &mut PaintCx<'_>) {
        self.scroller.paint_bar(cx);
    }

    fn intercept(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        self.scroller.intercept(cx, event)
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        self.scroller.wheel(cx, event)
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        Some(semantics::Node::new(
            self.id.clone(),
            cx.bounds(),
            semantics::Role::ScrollView,
        ))
    }

    fn clips(&self) -> bool {
        true
    }

    fn scroll_offset(&self) -> Vector {
        Vector::new(0.0, self.scroller.shown_offset())
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.scroller.is_moving(now)
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        for (_, child) in &mut self.built {
            visit(child);
        }
    }

    fn children_rev(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        for (_, child) in self.built.iter_mut().rev() {
            visit(child);
        }
    }
}

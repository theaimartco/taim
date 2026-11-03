//! A stack of pages: push to go deeper, pop (or press back) to return.
use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::{Constraints, Sizing};
use crate::state::{self, IntoProp, Invalidation, Prop, Scope, State};
use crate::views::look;
use crate::{
    AnyView, Border, Child, Event, Handled, Instant, Point, Size, Vector, View, semantics,
};

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

/// How long a page takes to slide in or out.
const TRANSITION: f32 = 0.3;
/// How far the page underneath moves while the top one slides, as a
/// fraction of the width (the parallax Android and iOS both use).
const UNDERLAY: f32 = 0.3;

/// The pages of a [`navigator`], from the first to the one showing.
///
/// A copyable handle: keep it in closures and call [`Navigator::push`] or
/// [`Navigator::pop`] from event handlers anywhere in the tree.
pub struct Navigator<R> {
    stack: State<Vec<R>>,
}

impl<R> Clone for Navigator<R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<R> Copy for Navigator<R> {}

impl<R: Clone + PartialEq + Send + Sync + 'static> Navigator<R> {
    /// A navigator showing `home`.
    pub fn new(cx: &mut Scope, home: R) -> Self {
        Self {
            stack: cx.signal(vec![home]),
        }
    }

    /// Shows `route` on top of the current page.
    pub fn push(&self, route: R) {
        self.stack.update(|stack| stack.push(route));
    }

    /// Returns to the previous page. `false`, and nothing happens, on the
    /// first page.
    pub fn pop(&self) -> bool {
        self.stack.update(|stack| {
            if stack.len() > 1 {
                let _ = stack.pop();
                true
            } else {
                false
            }
        })
    }

    /// Replaces the current page with `route`.
    pub fn replace(&self, route: R) {
        self.stack.update(|stack| {
            let _ = stack.pop();
            stack.push(route);
        });
    }

    /// Returns to the first page.
    pub fn pop_to_root(&self) {
        self.stack.update(|stack| stack.truncate(1));
    }

    /// The page showing.
    pub fn current(&self) -> R {
        self.stack
            .with(|stack| stack.last().cloned())
            .expect("a navigator always has a page")
    }

    /// How many pages are stacked; 1 on the first page.
    pub fn depth(&self) -> usize {
        self.stack.with(Vec::len)
    }

    /// Whether [`Navigator::pop`] would go anywhere.
    pub fn can_pop(&self) -> bool {
        self.depth() > 1
    }
}

/// Shows the top page of `navigator`, building each page with `page` the
/// first time it is shown.
///
/// Every page gets a scope of its own: state it creates lives as long as
/// the page is on the stack, so a page you go back to is as you left it,
/// and a popped page's state is freed. New pages slide in from the right;
/// back slides them out. The back button or gesture (and Escape) pops,
/// after any dialog, sheet or drawer on the page has closed.
///
/// ```ignore
/// #[derive(Clone, PartialEq)]
/// enum Route { Inbox, Message(u32) }
///
/// let nav = Navigator::new(cx, Route::Inbox);
/// navigator(nav, move |cx, route| match route {
///     Route::Inbox => button("Open").on_press(move || nav.push(Route::Message(7))).into(),
///     Route::Message(id) => text(format!("Message {id}")).into(),
/// })
/// ```
pub fn navigator<R>(
    navigator: Navigator<R>,
    page: impl Fn(&mut Scope, &R) -> AnyView + 'static,
) -> NavigatorView<R>
where
    R: Clone + PartialEq + Send + Sync + 'static,
{
    NavigatorView {
        navigator,
        stack: navigator.stack.into_prop(),
        page: Box::new(page),
        pages: Vec::new(),
        leaving: None,
        transition: Cell::new(None),
        width: 0.0,
        window: state::building(),
    }
}

/// One page and the state it owns. The page is dropped before its scope,
/// so no view outlives the state it reads.
struct Page<R> {
    route: R,
    child: Child,
    /// How far the page is shifted right, in pixels, for transitions.
    shift: Rc<Cell<f32>>,
    _scope: Scope,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Direction {
    Push,
    Pop,
}

#[derive(Debug, Clone, Copy)]
struct Transition {
    direction: Direction,
    /// Set at the first frame painted after the change.
    started: Option<Instant>,
}

/// See [`navigator`].
pub struct NavigatorView<R> {
    navigator: Navigator<R>,
    stack: Prop<Vec<R>>,
    page: Box<dyn Fn(&mut Scope, &R) -> AnyView>,
    pages: Vec<Page<R>>,
    /// The page sliding out after a pop.
    leaving: Option<Page<R>>,
    transition: Cell<Option<Transition>>,
    width: f32,
    window: Option<Arc<Invalidation>>,
}

impl<R: Clone + PartialEq + Send + Sync + 'static> NavigatorView<R> {
    fn build(&self, route: &R) -> Page<R> {
        let invalidation = self
            .window
            .clone()
            .unwrap_or_else(|| Arc::new(Invalidation::default()));
        let mut scope = Scope::new(invalidation);
        let shift = Rc::new(Cell::new(0.0));
        let view = state::rebuilding(self.window.as_ref(), || {
            scope.build(|scope| (self.page)(scope, route))
        });

        Page {
            route: route.clone(),
            child: Child::new(Slide {
                page: Child::new(view),
                shift: shift.clone(),
            }),
            shift,
            _scope: scope,
        }
    }

    /// Brings the pages in line with the stack: pages still on it are
    /// kept with their state, the rest are dropped, new ones are built.
    fn refresh(&mut self) {
        let stack = self.stack.get();
        let kept = self
            .pages
            .iter()
            .zip(&stack)
            .take_while(|(page, route)| page.route == **route)
            .count();

        if kept == self.pages.len() && kept == stack.len() {
            return;
        }

        let popped_one = kept + 1 == self.pages.len() && kept == stack.len();
        let mut removed = self.pages.split_off(kept);
        let pushed = stack.len() > kept && removed.is_empty();

        for route in &stack[kept..] {
            let page = self.build(route);
            self.pages.push(page);
        }

        let direction = if popped_one {
            self.leaving = removed.pop();
            Some(Direction::Pop)
        } else if pushed && self.pages.len() > 1 {
            self.leaving = None;
            Some(Direction::Push)
        } else {
            // A replacement or a jump of several pages: no animation.
            self.leaving = None;
            None
        };

        self.transition.set(direction.map(|direction| Transition {
            direction,
            started: None,
        }));
    }

    /// How far along the transition is at `now`, eased, if one runs.
    fn progress(&self, now: Instant) -> Option<(Direction, f32)> {
        let mut transition = self.transition.get()?;
        let started = *transition.started.get_or_insert(now);
        self.transition.set(Some(transition));

        let duration = if state::motion_reduced() {
            0.0
        } else {
            TRANSITION
        };
        let elapsed = now.saturating_duration_since(started).as_secs_f32();
        let linear = if duration <= 0.0 {
            1.0
        } else {
            (elapsed / duration).clamp(0.0, 1.0)
        };
        // Ease out: fast start, gentle landing.
        let eased = 1.0 - (1.0 - linear).powi(3);

        if linear >= 1.0 {
            self.transition.set(None);
        }
        Some((transition.direction, eased))
    }

    fn transitioning(&self) -> bool {
        self.transition.get().is_some()
    }
}

impl<R: Clone + PartialEq + Send + Sync + 'static> View for NavigatorView<R> {
    fn sizing(&self) -> Sizing {
        Sizing::FILL
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        self.refresh();

        if !self.transitioning() {
            self.leaving = None;
        }

        let outer = constraints.with_sizing(Sizing::FILL);
        let size = outer.max;
        self.width = size.width;

        let tight = Constraints::tight(size);
        for page in self.pages.iter_mut().rev().take(2) {
            let _ = page.child.measure(cx, tight);
            page.child.place(Point::ORIGIN);
        }
        if let Some(page) = &mut self.leaving {
            let _ = page.child.measure(cx, tight);
            page.child.place(Point::ORIGIN);
        }

        size
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let width = self.width;
        let top = self.pages.len().saturating_sub(1);

        for page in &self.pages {
            page.shift.set(0.0);
        }

        let shift = |index: Option<usize>, by: f32| {
            if let Some(page) = index.and_then(|index| self.pages.get(index)) {
                page.shift.set(by);
            }
        };

        match self.progress(cx.now()) {
            Some((Direction::Push, p)) => {
                shift(Some(top), width * (1.0 - p));
                shift(top.checked_sub(1), -width * UNDERLAY * p);
            }
            Some((Direction::Pop, p)) => {
                if let Some(leaving) = &self.leaving {
                    leaving.shift.set(width * p);
                }
                shift(Some(top), -width * UNDERLAY * (1.0 - p));
            }
            None => {}
        }
    }

    fn intercept(&mut self, _cx: &mut EventCx<'_>, event: &Event) -> Handled {
        // Pages do not take input while they move.
        Handled::from(self.transitioning() && look::is_pointer(event))
    }

    fn event(&mut self, _cx: &mut EventCx<'_>, event: &Event) -> Handled {
        // Back reaches the navigator only if nothing on the page (a
        // dialog, a sheet, a drawer) used it first.
        if look::is_back(event) && self.navigator.pop() {
            return Handled::Yes;
        }
        Handled::No
    }

    fn describe(&self, _cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }

    fn clips(&self) -> bool {
        true
    }

    fn is_animating(&self, _now: Instant) -> bool {
        self.transitioning()
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        let transitioning = self.transitioning();
        let count = self.pages.len();

        // The page underneath shows only while the top one slides.
        for (index, page) in self.pages.iter_mut().enumerate() {
            let top = index + 1 == count;
            let underneath = index + 2 == count;
            if top || (underneath && transitioning) {
                visit(&mut page.child);
            }
        }
        if transitioning && let Some(page) = &mut self.leaving {
            visit(&mut page.child);
        }
    }
}

/// A page, shifted sideways during transitions and painted opaque so the
/// page underneath does not show through.
struct Slide {
    page: Child,
    shift: Rc<Cell<f32>>,
}

impl View for Slide {
    fn sizing(&self) -> Sizing {
        Sizing::FILL
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let size = constraints.max;
        let _ = self.page.measure(cx, Constraints::tight(size));
        self.page.place(Point::ORIGIN);
        size
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let shift = self.shift.get();
        let mut bounds = cx.bounds();
        bounds.x += shift;
        let color = cx.theme().palette().background.base.color;
        cx.fill(bounds, color, Border::default());
    }

    fn describe(&self, _cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }

    fn scroll_offset(&self) -> Vector {
        Vector::new(-self.shift.get(), 0.0)
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        visit(&mut self.page);
    }
}

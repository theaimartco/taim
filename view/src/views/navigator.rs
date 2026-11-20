//! A stack of pages: push to go deeper, pop (or press back) to return.
use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::{Constraints, Sizing};
use crate::state::{self, IntoProp, Invalidation, Prop, Scope, State};
use crate::views::hero::{self, Board, Flight, PageLook, Stage};
use crate::views::look::{self, Gesture};
use crate::{
    AnyView, Border, Child, Color, Event, Handled, Instant, Point, Size, Vector, View, Visual,
    VisualCx, semantics,
};

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

/// How long a page takes to come in or go.
const TRANSITION: f32 = 0.3;
/// The quickest a page finishes moving after a swipe lets go.
const SHORTEST: f32 = 0.12;
/// How far the page underneath moves while the top one slides, as a
/// fraction of the width (the parallax Android and iOS both use).
const UNDERLAY: f32 = 0.3;
/// A swipe back starts this close to the leading edge, in pixels.
const EDGE: f32 = 24.0;
/// How far a finger moves before a swipe is a swipe, in pixels.
const SLOP: f32 = 8.0;
/// A swipe let go past this fraction of the width goes back...
const COMMIT: f32 = 0.4;
/// ...as does one let go faster than this, in pixels per second.
const FLICK: f32 = 500.0;

/// How pages move when a [`navigator`] goes deeper or back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageTransition {
    /// The new page slides in from the right over the old one, which
    /// drifts left and darkens; swiping from the left edge drags the page
    /// back. The iOS push.
    #[default]
    Slide,
    /// The new page grows in from slightly smaller while fading in, and the
    /// old one swells a little behind it. Material's zoom.
    Zoom,
    /// The pages cross-fade.
    Fade,
    /// The new page rises from the bottom while the old one recedes and
    /// darkens, like a full-screen sheet.
    Rise,
    /// Pages change at once.
    None,
}

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
/// and a popped page's state is freed. Pages move in and out as
/// [`NavigatorView::transition`] says, sliding by default; then a swipe
/// from the left edge drags the top page back, following the finger, and
/// letting go past the middle (or flicking) goes back. The back button or
/// gesture (and Escape) pops, after any dialog, sheet or drawer on the page
/// has closed. [`hero`](super::hero)es with the same tag on the two pages
/// fly between them while they move. With reduced motion pages change at
/// once.
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
        style: PageTransition::Slide,
        board: Rc::new(Board::default()),
        next_page: Cell::new(0),
        swipe: None,
        resume: None,
        size: Size::ZERO,
        window: state::building(),
    }
}

/// One page and the state it owns. The page is dropped before its scope,
/// so no view outlives the state it reads.
struct Page<R> {
    route: R,
    child: Child,
    /// How the page is drawn, shared with the heroes on it.
    look: Rc<PageLook>,
    _scope: Scope,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    /// A new page comes in over the old one.
    Push,
    /// The top page goes, uncovering the one underneath.
    Pop,
    /// A swipe let go too early: the top page returns.
    Stay,
}

/// Pages moving. `amount` is how far the covering page is from in place:
/// 0 when it covers everything, 1 when it is gone.
#[derive(Debug, Clone, Copy)]
struct Transition {
    kind: Kind,
    from: f32,
    to: f32,
    /// Seconds.
    duration: f32,
    /// Set at the first frame painted after the change.
    started: Option<Instant>,
}

/// A finger that started at the leading edge.
#[derive(Debug, Clone)]
struct Swipe {
    start: Point,
    /// It has moved far enough sideways to be dragging the page.
    dragging: bool,
    /// How far the page is dragged, as a fraction of the width.
    amount: f32,
    /// Recent `(time, x)`, for the speed it is let go at.
    samples: Vec<(Instant, f32)>,
}

impl Swipe {
    /// Pixels per second, positive towards going back.
    fn speed(&self) -> f32 {
        let Some(&(last, x)) = self.samples.last() else {
            return 0.0;
        };
        let Some(&(first, from)) = self
            .samples
            .iter()
            .find(|(at, _)| last.saturating_duration_since(*at).as_secs_f32() <= 0.1)
        else {
            return 0.0;
        };
        let elapsed = last.saturating_duration_since(first).as_secs_f32();
        if elapsed <= f32::EPSILON {
            0.0
        } else {
            (x - from) / elapsed
        }
    }
}

/// See [`navigator`].
pub struct NavigatorView<R> {
    navigator: Navigator<R>,
    stack: Prop<Vec<R>>,
    page: Box<dyn Fn(&mut Scope, &R) -> AnyView>,
    pages: Vec<Page<R>>,
    /// The page going away after a pop.
    leaving: Option<Page<R>>,
    transition: Cell<Option<Transition>>,
    style: PageTransition,
    board: Rc<Board>,
    next_page: Cell<u64>,
    swipe: Option<Swipe>,
    /// Where a pop that a swipe asked for picks up, and how long it takes.
    resume: Option<(f32, f32)>,
    size: Size,
    window: Option<Arc<Invalidation>>,
}

impl<R> NavigatorView<R> {
    /// How pages move between each other: [`PageTransition::Slide`] unless
    /// set.
    pub fn transition(mut self, style: PageTransition) -> Self {
        self.style = style;
        self
    }

    /// The transition as shown: none with reduced motion, where the
    /// selection change itself says the page changed.
    fn style(&self) -> PageTransition {
        if state::motion_reduced() {
            PageTransition::None
        } else {
            self.style
        }
    }

    fn duration(&self) -> f32 {
        TRANSITION
    }
}

/// How the covering (`upper`) and covered (`lower`) pages look when the
/// covering one is `amount` of the way from in place to gone.
fn poses(style: PageTransition, amount: f32, size: Size) -> [(Visual, f32, f32); 2] {
    let a = amount.clamp(0.0, 1.0);
    let covered = 1.0 - a;
    let moved = |x: f32, y: f32| Visual {
        offset: Vector::new(x, y),
        ..Visual::IDENTITY
    };
    // Each is (visual, dim, lift).
    match style {
        PageTransition::Slide => [
            (moved(size.width * a, 0.0), 0.0, covered.min(a * 4.0)),
            (
                moved(-size.width * UNDERLAY * covered, 0.0),
                0.18 * covered,
                0.0,
            ),
        ],
        PageTransition::Zoom => [
            (
                Visual {
                    scale: 1.0 - 0.1 * a,
                    opacity: covered,
                    ..Visual::IDENTITY
                },
                0.0,
                0.0,
            ),
            (
                Visual {
                    scale: 1.0 + 0.06 * covered,
                    ..Visual::IDENTITY
                },
                0.0,
                0.0,
            ),
        ],
        PageTransition::Fade => [
            (
                Visual {
                    opacity: covered,
                    ..Visual::IDENTITY
                },
                0.0,
                0.0,
            ),
            (Visual::IDENTITY, 0.0, 0.0),
        ],
        PageTransition::Rise => [
            (moved(0.0, size.height * a), 0.0, covered.min(a * 4.0)),
            (
                Visual {
                    scale: 1.0 - 0.05 * covered,
                    ..Visual::IDENTITY
                },
                0.3 * covered,
                0.0,
            ),
        ],
        PageTransition::None => [(Visual::IDENTITY, 0.0, 0.0), (Visual::IDENTITY, 0.0, 0.0)],
    }
}

impl<R: Clone + PartialEq + Send + Sync + 'static> NavigatorView<R> {
    fn build(&self, route: &R) -> Page<R> {
        let invalidation = self
            .window
            .clone()
            .unwrap_or_else(|| Arc::new(Invalidation::default()));
        let mut scope = Scope::new(invalidation);
        let id = self.next_page.get();
        self.next_page.set(id.wrapping_add(1));
        let look = PageLook::new(id);
        let stage = Stage {
            board: self.board.clone(),
            page: look.clone(),
        };
        let view = hero::on_stage(stage, || {
            state::rebuilding(self.window.as_ref(), || {
                scope.build(|scope| (self.page)(scope, route))
            })
        });

        Page {
            route: route.clone(),
            child: Child::new(Slide {
                page: Child::new(view),
                look: look.clone(),
            }),
            look,
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

        let animated = self.style() != PageTransition::None;
        let resume = self.resume.take();
        let transition = if popped_one {
            self.leaving = removed.pop();
            let (from, duration) = resume.unwrap_or((0.0, self.duration()));
            Some(Transition {
                kind: Kind::Pop,
                from,
                to: 1.0,
                duration,
                started: None,
            })
        } else if pushed && self.pages.len() > 1 {
            self.leaving = None;
            Some(Transition {
                kind: Kind::Push,
                from: 1.0,
                to: 0.0,
                duration: self.duration(),
                started: None,
            })
        } else {
            // A replacement or a jump of several pages: no animation.
            self.leaving = None;
            None
        };
        self.swipe = None;
        self.transition.set(transition.filter(|_| animated));
        if !animated {
            self.leaving = None;
        }

        let ids: Vec<u64> = self
            .pages
            .iter()
            .chain(&self.leaving)
            .map(|page| page.look.id)
            .collect();
        self.board.keep_only(&ids);
    }

    /// The transition at `now`: its kind and how far the covering page is
    /// from in place, eased.
    fn amount(&self, now: Instant) -> Option<(Kind, f32)> {
        let mut transition = self.transition.get()?;
        let started = *transition.started.get_or_insert(now);
        self.transition.set(Some(transition));

        let elapsed = now.saturating_duration_since(started).as_secs_f32();
        let linear = if transition.duration <= 0.0 {
            1.0
        } else {
            (elapsed / transition.duration).clamp(0.0, 1.0)
        };
        // Ease out: fast start, gentle landing.
        let eased = 1.0 - (1.0 - linear).powi(3);
        let amount = transition.from + (transition.to - transition.from) * eased;

        if linear >= 1.0 {
            self.transition.set(None);
            // The shell paints animation frames without laying out. Ask for
            // one final layout so the outgoing page's scope and tasks are
            // released even if no further input or model change occurs.
            if transition.kind == Kind::Pop
                && let Some(window) = &self.window
            {
                window.mark(Invalidation::LAYOUT);
            }
        }
        Some((transition.kind, amount))
    }

    fn transitioning(&self) -> bool {
        self.transition.get().is_some()
    }

    fn dragging(&self) -> bool {
        self.swipe.as_ref().is_some_and(|swipe| swipe.dragging)
    }

    /// Whether a swipe from the edge may drag the top page back.
    fn swipeable(&self) -> bool {
        self.style() == PageTransition::Slide && self.pages.len() > 1 && !self.transitioning()
    }

    /// A swipe let go: back if it went far or fast enough, else the page
    /// returns.
    fn let_go(&mut self, swipe: &Swipe) {
        let speed = swipe.speed();
        let amount = swipe.amount;
        let width = self.size.width.max(1.0);
        let goes = amount > COMMIT && speed > -FLICK || speed > FLICK;
        // Keep the finger's speed: an ease-out curve starts at three times
        // its average speed.
        let timed = |distance: f32, towards: f32| {
            let towards = towards.max(1.0);
            (3.0 * distance * width / towards).clamp(SHORTEST, TRANSITION * distance.max(0.4))
        };
        if goes {
            self.resume = Some((amount, timed(1.0 - amount, speed)));
            let _ = self.navigator.pop();
        } else {
            self.transition.set(Some(Transition {
                kind: Kind::Stay,
                from: amount,
                to: 0.0,
                duration: timed(amount, -speed),
                started: None,
            }));
        }
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
        self.size = size;

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
        for page in self.pages.iter().chain(&self.leaving) {
            page.look.visual.set(Visual::IDENTITY);
            page.look.dim.set(0.0);
            page.look.lift.set(0.0);
        }
        self.board.flight.set(None);

        let top = self.pages.len().checked_sub(1);
        let under = top.and_then(|top| top.checked_sub(1));
        let page = |index: Option<usize>| index.and_then(|index| self.pages.get(index));

        let pose = match self.amount(cx.now()) {
            Some((Kind::Pop, amount)) => self
                .leaving
                .as_ref()
                .zip(page(top))
                .map(|pair| (pair, amount)),
            Some((Kind::Push | Kind::Stay, amount)) => {
                page(top).zip(page(under)).map(|pair| (pair, amount))
            }
            None => self
                .swipe
                .as_ref()
                .filter(|swipe| swipe.dragging)
                .and_then(|swipe| page(top).zip(page(under)).map(|pair| (pair, swipe.amount))),
        };

        if let Some(((upper, lower), amount)) = pose {
            let [above, below] = poses(self.style(), amount, self.size);
            for (page, (visual, dim, lift)) in [(upper, above), (lower, below)] {
                page.look.visual.set(visual);
                page.look.dim.set(dim);
                page.look.lift.set(lift);
            }
            self.board.flight.set(Some(Flight {
                upper: upper.look.id,
                lower: lower.look.id,
                travel: amount,
            }));
        }
    }

    fn intercept(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        if crate::tree::is_cancellation(event) {
            if let Some(swipe) = self.swipe.take()
                && swipe.dragging
            {
                self.let_go(&Swipe {
                    samples: Vec::new(),
                    ..swipe
                });
                cx.request_animation();
            }
            return Handled::No;
        }

        if cx.is_touch() && (self.swipe.is_some() || self.swipeable()) {
            let bounds = cx.bounds();
            let now = cx.now();
            match look::gesture(event, cx.pointer()) {
                Some(Gesture::Down(at))
                    if self.swipeable() && bounds.contains(at) && at.x - bounds.x <= EDGE =>
                {
                    self.swipe = Some(Swipe {
                        start: at,
                        dragging: false,
                        amount: 0.0,
                        samples: vec![(now, at.x)],
                    });
                    // The page still gets the touch until it is a swipe.
                    return Handled::No;
                }
                Some(Gesture::Move(at)) => {
                    if let Some(swipe) = &mut self.swipe {
                        let (dx, dy) = (at.x - swipe.start.x, at.y - swipe.start.y);
                        if !swipe.dragging {
                            if dy.abs() > SLOP && dy.abs() > dx {
                                // Scrolling, not swiping.
                                self.swipe = None;
                                return Handled::No;
                            }
                            if dx <= SLOP {
                                return Handled::No;
                            }
                            swipe.dragging = true;
                            swipe.start.x = at.x;
                        }
                        let width = bounds.width.max(1.0);
                        swipe.amount = ((at.x - swipe.start.x) / width).clamp(0.0, 1.0);
                        swipe.samples.push((now, at.x));
                        if swipe.samples.len() > 32 {
                            let _ = swipe.samples.drain(..16);
                        }
                        cx.request_paint();
                        return Handled::Yes;
                    }
                }
                Some(Gesture::Up(_)) => {
                    if let Some(mut swipe) = self.swipe.take() {
                        if !swipe.dragging {
                            return Handled::No;
                        }
                        if let Some(&(_, x)) = swipe.samples.last() {
                            swipe.samples.push((now, x));
                        }
                        self.let_go(&swipe);
                        cx.request_animation();
                        return Handled::Yes;
                    }
                }
                _ => {}
            }
        }

        // Pages do not take input while they move.
        Handled::from(self.transitioning() && look::is_pointer(event))
    }

    fn event(&mut self, _cx: &mut EventCx<'_>, event: &Event) -> Handled {
        // Back reaches the navigator only if nothing on the page (a
        // dialog, a sheet, a drawer) used it first.
        if look::is_back(event) && !self.dragging() && self.navigator.pop() {
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
        let popping = self
            .transition
            .get()
            .is_some_and(|transition| transition.kind == Kind::Pop);
        // The page underneath shows only while the top one moves.
        let uncovered = !popping && (self.transitioning() || self.dragging());
        let count = self.pages.len();

        for (index, page) in self.pages.iter_mut().enumerate() {
            let top = index + 1 == count;
            let underneath = index + 2 == count;
            if top || (underneath && uncovered) {
                visit(&mut page.child);
            }
        }
        if popping && let Some(page) = &mut self.leaving {
            visit(&mut page.child);
        }
    }
}

/// A page, drawn as its navigator poses it and painted opaque so the page
/// underneath does not show through.
struct Slide {
    page: Child,
    look: Rc<PageLook>,
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
        let bounds = cx.bounds();
        let color = cx.theme().palette().background.base.color;
        let lift = self.look.lift.get();
        if lift > 0.0 {
            // A page moving over another casts a soft shadow on it.
            cx.fill_shadowed(
                bounds,
                color,
                Border::default(),
                crate::Shadow {
                    color: Color::BLACK.scale_alpha(0.25 * lift),
                    offset: Vector::ZERO,
                    blur_radius: 18.0,
                },
            );
        } else {
            cx.fill(bounds, color, Border::default());
        }
    }

    fn paint_over(&self, cx: &mut PaintCx<'_>) {
        let dim = self.look.dim.get();
        if dim > 0.0 {
            cx.fill(
                cx.bounds(),
                Color::BLACK.scale_alpha(dim),
                Border::default(),
            );
        }
    }

    fn describe(&self, _cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }

    fn visual(&self, cx: &VisualCx) -> Visual {
        self.look.centre.set(cx.bounds().center());
        self.look.visual.get()
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        visit(&mut self.page);
    }
}

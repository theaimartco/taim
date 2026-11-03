//! Views that slide over the rest of the screen.
use super::look::{self, Gesture, Label, Tween};
use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::{Constraints, Sizing};
use crate::state::{IntoProp, Prop, State, spawn};
use crate::{AnyView, Border, Child, Color, Duration, Event, Handled, Instant, Point, Rectangle};
use crate::{Size, Vector, View, semantics};

use std::cell::Cell;
use taim_core::widget;

/// How far from the left edge a swipe may start to open a drawer.
const EDGE: f32 = 16.0;

/// A panel that slides in from the left over the screen while `open` is
/// true. Swiping in from the left edge opens it; tapping outside or
/// swiping it back closes it.
pub fn drawer(content: impl Into<AnyView>, open: State<bool>) -> Drawer {
    Drawer {
        content: Child::new(content.into()),
        open,
        shown: open.into_prop(),
        followed: open.get(),
        width: 300.0,
        progress: Tween::new(if open.get() { 1.0 } else { 0.0 }),
        drag: None,
        painted: Cell::new(0.0),
        id: widget::Id::unique(),
    }
}

/// See [`drawer`].
pub struct Drawer {
    content: Child,
    open: State<bool>,
    shown: Prop<bool>,
    /// The `open` value the animation last started towards.
    followed: bool,
    width: f32,
    progress: Tween,
    /// While dragging: where the drag started and the progress it started
    /// from, plus the latest progress.
    drag: Option<(f32, f32, f32)>,
    /// Progress at the last paint; decides offset and hit-testing.
    painted: Cell<f32>,
    id: widget::Id,
}

impl Drawer {
    /// Panel width in logical pixels.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(1.0);
        self
    }

    fn progress(&self, now: Instant) -> f32 {
        self.drag
            .map_or_else(|| self.progress.value(now), |(_, _, current)| current)
            .clamp(0.0, 1.0)
    }

    fn settle(&mut self, open: bool, now: Instant) {
        self.followed = open;
        self.progress.go(if open { 1.0 } else { 0.0 }, now);
        let _ = self.open.set(open);
    }
}

impl View for Drawer {
    fn sizing(&self) -> Sizing {
        Sizing::FILL
    }

    fn is_layer(&self) -> bool {
        true
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let open = self.shown.get();
        if open != self.followed {
            self.followed = open;
            self.progress
                .go(if open { 1.0 } else { 0.0 }, Instant::now());
        }

        let bounds = constraints.with_sizing(Sizing::FILL).max;
        let room = Size::new(self.width - 32.0, bounds.height - 32.0);
        let _ = self.content.measure(cx, Constraints::loose(room));
        self.content.place(Point::new(16.0, 16.0));
        bounds
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let progress = self.progress(cx.now());
        self.painted.set(progress);
        if progress <= 0.0 {
            return;
        }

        let bounds = cx.bounds();
        let palette = *cx.theme().palette();
        cx.fill(
            bounds,
            Color::BLACK.scale_alpha(0.4 * progress),
            Border::default(),
        );

        let panel = Rectangle::new(
            Point::new(bounds.x - self.width * (1.0 - progress), bounds.y),
            Size::new(self.width, bounds.height),
        );
        cx.fill(panel, palette.background.base.color, Border::default());
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let bounds = cx.bounds();
        let now = cx.now();
        let progress = self.progress(now);

        if progress > 0.0 && look::is_back(event) {
            self.settle(false, now);
            cx.request_animation();
            return Handled::Yes;
        }

        match look::gesture(event, cx.pointer()) {
            Some(Gesture::Down(at)) if bounds.contains(at) => {
                let on_panel = at.x < bounds.x + self.width * progress;
                let from_edge = progress == 0.0 && at.x < bounds.x + EDGE;

                if from_edge || (progress > 0.0 && on_panel) {
                    self.drag = Some((at.x, progress, progress));
                    return Handled::Yes;
                }
                if progress > 0.0 {
                    // Tap on the scrim.
                    self.settle(false, now);
                    cx.request_animation();
                    return Handled::Yes;
                }
                Handled::No
            }
            Some(Gesture::Move(at)) => {
                let Some((start, from, _)) = self.drag else {
                    return Handled::No;
                };
                let current = (from + (at.x - start) / self.width).clamp(0.0, 1.0);
                self.drag = Some((start, from, current));
                cx.request_paint();
                Handled::Yes
            }
            Some(Gesture::Up(_)) => {
                let Some((start, from, current)) = self.drag.take() else {
                    return Handled::No;
                };
                // A drag of a few pixels is a tap: leave the drawer as it was.
                let moved = ((current - from) * self.width).abs() > 4.0;
                let open = if moved { current > 0.5 } else { from > 0.5 };
                let _ = start;
                self.progress = Tween::new(current);
                self.settle(open, now);
                cx.request_animation();
                Handled::Yes
            }
            _ if progress > 0.0 => Handled::from(look::is_pointer(event)),
            _ => Handled::No,
        }
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        (self.painted.get() > 0.0).then(|| {
            semantics::Node::new(self.id.clone(), cx.bounds(), semantics::Role::NamedGroup)
                .with_name("Navigation drawer")
        })
    }

    fn scroll_offset(&self) -> Vector {
        Vector::new(self.width * (1.0 - self.painted.get()), 0.0)
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.progress.is_moving(now)
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        if self.painted.get() > 0.0 {
            visit(&mut self.content);
        }
    }
}

/// A panel that slides up from the bottom while `open` is true. Its handle
/// can be dragged between `detents` (fractions of the screen height) or
/// flung down to dismiss.
pub fn sheet(content: impl Into<AnyView>, open: State<bool>) -> Sheet {
    Sheet {
        content: Child::new(content.into()),
        open,
        shown: open.into_prop(),
        followed: open.get(),
        detents: vec![0.4],
        detent: 0,
        height: Tween::new(0.0),
        drag: None,
        painted: Cell::new(0.0),
        screen: Cell::new(0.0),
        on_dismiss: None,
        id: widget::Id::unique(),
    }
}

/// See [`sheet`].
pub struct Sheet {
    content: Child,
    open: State<bool>,
    shown: Prop<bool>,
    followed: bool,
    detents: Vec<f32>,
    detent: usize,
    /// Visible height as a fraction of the screen.
    height: Tween,
    drag: Option<(f32, f32, f32)>,
    painted: Cell<f32>,
    screen: Cell<f32>,
    on_dismiss: Option<Box<dyn Fn()>>,
    id: widget::Id,
}

const HANDLE: f32 = 28.0;

impl Sheet {
    /// Heights the sheet rests at, as fractions of the screen height.
    pub fn detents(mut self, detents: impl IntoIterator<Item = f32>) -> Self {
        let mut detents: Vec<f32> = detents
            .into_iter()
            .map(|detent| detent.clamp(0.1, 1.0))
            .collect();
        detents.sort_by(f32::total_cmp);
        if !detents.is_empty() {
            self.detents = detents;
        }
        self
    }

    /// Runs `action` when the user dismisses the sheet (scrim tap or
    /// fling down).
    pub fn on_dismiss(mut self, action: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Box::new(action));
        self
    }

    fn target(&self) -> f32 {
        self.detents[self.detent.min(self.detents.len() - 1)]
    }

    fn dismiss(&mut self, now: Instant) {
        self.followed = false;
        self.height.go(0.0, now);
        let _ = self.open.set(false);
        if let Some(dismissed) = &self.on_dismiss {
            dismissed();
        }
    }

    fn current(&self, now: Instant) -> f32 {
        self.drag
            .map_or_else(|| self.height.value(now), |(_, _, current)| current)
    }
}

impl View for Sheet {
    fn sizing(&self) -> Sizing {
        Sizing::FILL
    }

    fn is_layer(&self) -> bool {
        true
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let open = self.shown.get();
        if open != self.followed {
            self.followed = open;
            let target = if open { self.target() } else { 0.0 };
            self.height.go(target, Instant::now());
        }

        let bounds = constraints.with_sizing(Sizing::FILL).max;
        self.screen.set(bounds.height);
        let tallest = self.detents.last().copied().unwrap_or(0.5) * bounds.height;
        let room = Size::new(bounds.width - 32.0, (tallest - HANDLE - 16.0).max(0.0));
        let _ = self.content.measure(cx, Constraints::loose(room));
        self.content.place(Point::new(16.0, HANDLE));
        bounds
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let fraction = self.current(cx.now()).max(0.0);
        self.painted.set(fraction);
        if fraction <= 0.0 {
            return;
        }

        let bounds = cx.bounds();
        let palette = *cx.theme().palette();
        let scrim = (fraction / self.detents[0]).min(1.0);
        cx.fill(
            bounds,
            Color::BLACK.scale_alpha(0.3 * scrim),
            Border::default(),
        );

        let top = bounds.y + bounds.height * (1.0 - fraction);
        let panel = Rectangle::new(
            Point::new(bounds.x, top),
            Size::new(bounds.width, bounds.height * fraction + 16.0),
        );
        // Liquid glass rising over the dimmed page.
        super::glass::pane(
            cx,
            panel,
            28.0,
            taim_core::material::Material::LIQUID,
            None,
            1.5,
        );

        let grip = Rectangle::new(
            Point::new(bounds.x + bounds.width / 2.0 - 18.0, top + 10.0),
            Size::new(36.0, 4.0),
        );
        cx.fill(
            grip,
            palette.background.strong.color,
            Border::default().rounded(2.0),
        );
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let bounds = cx.bounds();
        let fraction = self.current(cx.now()).max(0.0);
        if fraction <= 0.0 && self.drag.is_none() {
            return Handled::No;
        }

        let top = bounds.y + bounds.height * (1.0 - fraction);
        let now = cx.now();

        if look::is_back(event) {
            self.dismiss(now);
            cx.request_animation();
            return Handled::Yes;
        }

        match look::gesture(event, cx.pointer()) {
            Some(Gesture::Down(at)) if at.y < top => {
                // Tap on the scrim dismisses.
                self.dismiss(now);
                cx.request_animation();
                Handled::Yes
            }
            Some(Gesture::Down(at)) if at.y < top + HANDLE => {
                self.drag = Some((at.y, fraction, fraction));
                Handled::Yes
            }
            Some(Gesture::Move(at)) => {
                let Some((start, from, _)) = self.drag else {
                    return Handled::from(fraction > 0.0);
                };
                let current = (from - (at.y - start) / bounds.height.max(1.0)).clamp(0.0, 1.0);
                self.drag = Some((start, from, current));
                cx.request_paint();
                Handled::Yes
            }
            Some(Gesture::Up(_)) => {
                let Some((_, _, current)) = self.drag.take() else {
                    return Handled::from(fraction > 0.0);
                };
                self.height = Tween::new(current);

                if current < self.detents[0] * 0.5 {
                    self.dismiss(now);
                } else {
                    self.detent = self
                        .detents
                        .iter()
                        .enumerate()
                        .min_by(|(_, a), (_, b)| {
                            (*a - current).abs().total_cmp(&(*b - current).abs())
                        })
                        .map_or(0, |(index, _)| index);
                    self.height.go(self.target(), now);
                }
                cx.request_animation();
                Handled::Yes
            }
            _ => Handled::from(look::is_pointer(event)),
        }
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        (self.painted.get() > 0.0).then(|| {
            semantics::Node::new(self.id.clone(), cx.bounds(), semantics::Role::NamedGroup)
                .with_name("Sheet")
        })
    }

    fn scroll_offset(&self) -> Vector {
        let screen = self.screen.get();
        Vector::new(0.0, -(screen * (1.0 - self.painted.get())))
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.height.is_moving(now)
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        if self.painted.get() > 0.0 {
            visit(&mut self.content);
        }
    }
}

/// A short notice at the bottom of the screen showing `message` while it
/// is `Some`. It clears `message` itself after a few seconds.
pub fn snackbar(message: State<Option<String>>) -> Snackbar {
    Snackbar {
        message,
        shown: message.into_prop(),
        label: Label::new("", None),
        action: None,
        last: None,
        visible: Tween::new(0.0),
        card: Cell::new(Rectangle::default()),
        action_area: Cell::new(Rectangle::default()),
        id: widget::Id::unique(),
    }
}

/// See [`snackbar`].
pub struct Snackbar {
    message: State<Option<String>>,
    shown: Prop<Option<String>>,
    label: Label,
    action: Option<(Label, Box<dyn Fn()>)>,
    last: Option<String>,
    visible: Tween,
    card: Cell<Rectangle>,
    action_area: Cell<Rectangle>,
    id: widget::Id,
}

/// How long a notice stays up.
const LINGER: Duration = Duration::from_secs(4);

impl Snackbar {
    /// Adds an action button; pressing it runs `action` and hides the
    /// notice.
    pub fn action(mut self, label: impl Into<String>, action: impl Fn() + 'static) -> Self {
        self.action = Some((Label::new(label, None), Box::new(action)));
        self
    }
}

impl View for Snackbar {
    fn sizing(&self) -> Sizing {
        Sizing::FILL
    }

    fn is_layer(&self) -> bool {
        true
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let message = self.shown.get();

        if message != self.last {
            let now = Instant::now();
            match &message {
                Some(text) => {
                    self.label.set(text);
                    self.visible.go(1.0, now);

                    // Hide this notice later, unless another replaced it.
                    let state = self.message;
                    let expected = message.clone();
                    spawn(async move {
                        std::thread::sleep(LINGER);
                        if state.get() == expected {
                            let _ = state.set(None);
                        }
                    });
                }
                None => self.visible.go(0.0, now),
            }
            self.last = message;
        }

        let _ = self.label.measure(cx);
        if let Some((label, _)) = &mut self.action {
            let _ = label.measure(cx);
        }
        constraints.with_sizing(Sizing::FILL).max
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let visible = self.visible.value(cx.now());
        if visible <= 0.0 {
            self.card.set(Rectangle::default());
            return;
        }

        let bounds = cx.bounds();
        let palette = *cx.theme().palette();
        let width = (bounds.width - 32.0).min(480.0);
        let height = 48.0;
        let card = Rectangle::new(
            Point::new(
                bounds.x + (bounds.width - width) / 2.0,
                bounds.y + bounds.height - (height + 88.0) * visible,
            ),
            Size::new(width, height),
        );
        self.card.set(card);

        // Notices invert the theme: a dark card on light themes and a light
        // card on dark ones.
        let (card_color, ink, accent) = if palette.is_dark {
            (
                Color::from_rgb8(0xEC, 0xEC, 0xF0),
                Color::from_rgb8(0x1C, 0x1C, 0x22),
                palette.primary.strong.color,
            )
        } else {
            (
                Color::from_rgb8(0x2A, 0x2A, 0x32),
                Color::from_rgb8(0xF4, 0xF4, 0xF8),
                palette.primary.weak.color,
            )
        };
        cx.fill(card, card_color, Border::default().rounded(8.0));
        let text_area = Rectangle::new(
            Point::new(card.x + 16.0, card.y),
            Size::new(card.width - 120.0, card.height),
        );
        self.label
            .paint(cx, Point::new(text_area.x, card.y + 14.0), ink);

        if let Some((label, _)) = &self.action {
            let area = Rectangle::new(
                Point::new(card.x + card.width - 96.0, card.y),
                Size::new(96.0, card.height),
            );
            self.action_area.set(area);
            label.paint_centered(cx, area, accent);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let card = self.card.get();
        if card.width == 0.0 {
            return Handled::No;
        }

        match look::gesture(event, cx.pointer()) {
            Some(Gesture::Down(at)) if self.action_area.get().contains(at) => {
                if let Some((_, action)) = &self.action {
                    action();
                }
                let _ = self.message.set(None);
                Handled::Yes
            }
            Some(Gesture::Down(at)) if card.contains(at) => Handled::Yes,
            _ => Handled::No,
        }
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        self.last.as_ref().map(|message| {
            semantics::Node::new(self.id.clone(), cx.bounds(), semantics::Role::StaticText)
                .with_name(message.clone())
                .with_live(semantics::Live::Polite)
        })
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.visible.is_moving(now)
    }
}

/// Lets the user pull `content` down to refresh. Releasing past the
/// threshold sets `refreshing` and runs the refresh action.
pub fn pull_to_refresh(content: impl Into<AnyView>, refreshing: State<bool>) -> PullToRefresh {
    PullToRefresh {
        content: Child::new(content.into()),
        refreshing,
        busy: refreshing.into_prop(),
        on_refresh: None,
        drag: None,
        finger: None,
        settle: Tween::new(0.0),
        painted: Cell::new(0.0),
    }
}

/// See [`pull_to_refresh`].
pub struct PullToRefresh {
    content: Child,
    refreshing: State<bool>,
    busy: Prop<bool>,
    on_refresh: Option<Box<dyn Fn()>>,
    /// Pointer y where the pull started, and the current pull distance.
    drag: Option<(f32, f32)>,
    /// Where a finger landed, before it is known to be pulling.
    finger: Option<f32>,
    settle: Tween,
    painted: Cell<f32>,
}

const THRESHOLD: f32 = 70.0;
const HOLD: f32 = 56.0;
/// How far a finger moves down before it is pulling rather than tapping.
const PULL_SLOP: f32 = 8.0;

/// Whether anything inside `child` is scrolled away from its top, so a
/// downward drag should scroll it back rather than pull to refresh.
fn scrolled_down(child: &mut Child) -> bool {
    if child.view().scroll_offset().y > 0.0 {
        return true;
    }

    let mut scrolled = false;
    child.view_mut().children(&mut |grandchild| {
        scrolled = scrolled || scrolled_down(grandchild);
    });
    scrolled
}

impl PullToRefresh {
    /// Follows the pointer to `y`, returning the pull distance.
    fn pull_to(&mut self, y: f32, cx: &mut EventCx<'_>) -> f32 {
        let Some((start, _)) = self.drag else {
            return 0.0;
        };
        // Resistance: the content follows at half the finger speed.
        let pull = ((y - start) * 0.5).clamp(0.0, 120.0);
        self.drag = Some((start, pull));
        cx.request_paint();
        pull
    }

    /// Ends a pull: refreshes past the threshold, springs back either way.
    fn release(&mut self, cx: &mut EventCx<'_>) -> Handled {
        let Some((_, pull)) = self.drag.take() else {
            return Handled::No;
        };
        self.settle = Tween::new(pull);
        if pull >= THRESHOLD {
            let _ = self.refreshing.set(true);
            if let Some(refresh) = &self.on_refresh {
                refresh();
            }
            self.settle.go(HOLD, cx.now());
        } else {
            self.settle.go(0.0, cx.now());
        }
        cx.request_animation();
        Handled::from(pull > 0.0)
    }

    /// Runs `action` when the user pulls past the threshold.
    pub fn on_refresh(mut self, action: impl Fn() + 'static) -> Self {
        self.on_refresh = Some(Box::new(action));
        self
    }
}

impl View for PullToRefresh {
    fn sizing(&self) -> Sizing {
        Sizing::FILL
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let busy = self.busy.get();
        let rest = if busy { HOLD } else { 0.0 };
        if self.drag.is_none() && (self.settle.target() - rest).abs() > f32::EPSILON {
            self.settle.go(rest, Instant::now());
        }

        let size = self
            .content
            .measure(cx, constraints.with_sizing(Sizing::FILL));
        self.content.place(Point::ORIGIN);
        size
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let pull = self
            .drag
            .map_or_else(|| self.settle.value(cx.now()), |(_, pull)| pull);
        self.painted.set(pull);
    }

    fn paint_over(&self, cx: &mut PaintCx<'_>) {
        let pull = self.painted.get();
        if pull <= 1.0 {
            return;
        }

        let bounds = cx.bounds();
        let palette = *cx.theme().palette();
        let ready = (pull / THRESHOLD).min(1.0);
        let dot = Rectangle::new(
            Point::new(
                bounds.x + bounds.width / 2.0 - 14.0,
                bounds.y + pull / 2.0 - 14.0,
            ),
            Size::new(28.0, 28.0),
        );

        cx.fill(
            dot,
            palette.background.base.color,
            Border::default()
                .width(3.0)
                .color(palette.primary.base.color.scale_alpha(ready))
                .rounded(14.0),
        );

        if self.busy.get() {
            // A dot orbiting the ring while the refresh runs.
            let angle = cx.now().elapsed().as_secs_f32() * 6.0;
            let center = dot.center();
            let orbit = Rectangle::new(
                Point::new(
                    center.x + 8.0 * angle.cos() - 3.0,
                    center.y + 8.0 * angle.sin() - 3.0,
                ),
                Size::new(6.0, 6.0),
            );
            cx.fill(
                orbit,
                palette.primary.base.color,
                Border::default().rounded(3.0),
            );
        }
    }

    /// A finger pulls before the content sees it, as long as the content
    /// is at its top: then a downward drag is a pull, even one that starts
    /// on a button. Otherwise the content scrolls back first.
    fn intercept(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        if !cx.is_touch() {
            return Handled::No;
        }

        match look::gesture(event, cx.pointer()) {
            Some(Gesture::Down(at)) if cx.bounds().contains(at) && !self.busy.get() => {
                self.finger = Some(at.y);
                Handled::No
            }
            Some(Gesture::Move(at)) if self.drag.is_none() => {
                let Some(start) = self.finger else {
                    return Handled::No;
                };
                let moved = at.y - start;

                if moved < -PULL_SLOP || (moved > PULL_SLOP && scrolled_down(&mut self.content)) {
                    // Up, or down a list that can scroll back: not a pull.
                    self.finger = None;
                    return Handled::No;
                }
                if moved <= PULL_SLOP {
                    return Handled::No;
                }

                self.finger = None;
                self.drag = Some((start + PULL_SLOP, 0.0));
                let _ = self.pull_to(at.y, cx);
                Handled::Yes
            }
            Some(Gesture::Move(at)) => {
                let _ = self.pull_to(at.y, cx);
                Handled::Yes
            }
            Some(Gesture::Up(_)) => {
                self.finger = None;
                self.release(cx)
            }
            _ => Handled::No,
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let bounds = cx.bounds();

        // A finger pulls through `intercept`.
        if cx.is_touch() {
            return Handled::No;
        }

        match look::gesture(event, cx.pointer()) {
            Some(Gesture::Down(at)) if bounds.contains(at) && !self.busy.get() => {
                self.drag = Some((at.y, 0.0));
                Handled::No
            }
            Some(Gesture::Move(at)) => {
                if self.drag.is_none() {
                    return Handled::No;
                }
                let pull = self.pull_to(at.y, cx);
                Handled::from(pull > 0.0)
            }
            Some(Gesture::Up(_)) => self.release(cx),
            _ => Handled::No,
        }
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        self.busy.get().then(|| {
            semantics::Node::new(widget::Id::unique(), cx.bounds(), semantics::Role::Group)
                .with_state(semantics::State::BUSY)
        })
    }

    fn scroll_offset(&self) -> Vector {
        Vector::new(0.0, -self.painted.get())
    }

    fn clips(&self) -> bool {
        true
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.settle.is_moving(now) || self.busy.get()
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        visit(&mut self.content);
    }
}

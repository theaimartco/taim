use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::{Constraints, Sizing};
use crate::views::look::{self, Gesture};
use crate::{
    AnyView, Border, Child, Event, Handled, Instant, Point, Rectangle, Size, Vector, View,
};
use crate::{mouse, semantics};

use taim_core::widget;

use std::cell::Cell;

/// Shows `content` in a vertically scrollable area that fills its parent.
///
/// A mouse wheel or trackpad scrolls it. A finger drags it, and a finger
/// lifted while moving flings it, slowing to a stop. A drag that starts on
/// a button scrolls instead of pressing the button once the finger has
/// moved further than the touch slop.
pub fn scroll(content: impl Into<AnyView>) -> Scroll {
    Scroll {
        content: Child::new(content.into()),
        scroller: Scroller::default(),
        viewport: Size::ZERO,
        id: widget::Id::unique(),
    }
}

/// See [`scroll`].
pub struct Scroll {
    content: Child,
    scroller: Scroller,
    viewport: Size,
    id: widget::Id,
}

/// Pixels per wheel "line".
const LINE: f32 = 40.0;
const BAR: f32 = 6.0;

/// How far a finger moves before it is scrolling rather than tapping, in
/// logical pixels. Android uses 8dp.
const SLOP: f32 = 8.0;
/// Slower flings than this (pixels per second) just stop.
const MIN_FLING: f32 = 50.0;
/// Android's default maximum fling velocity, 8000dp/s.
const MAX_FLING: f32 = 8_000.0;
/// How quickly a fling loses speed: velocity decays as `e^(-FRICTION t)`.
const FRICTION: f32 = 4.0;
/// Velocity is measured over the last this many seconds of the drag.
const VELOCITY_WINDOW: f32 = 0.1;
/// How far content can be pulled past an end, in logical pixels: the
/// stretch approaches this but never reaches it.
const STRETCH: f32 = 140.0;
/// How much of a fling's remaining speed carries past the end it hits.
const BOUNCE_CARRY: f32 = 0.5;
/// The furthest past an end a fling may carry, in pixels per second.
const MAX_BOUNCE: f32 = 2_400.0;

/// Pulling past an end: critically damped, so it eases back without
/// wobbling, like a rubber band.
const SPRING_BACK: look::Tuning = look::Tuning {
    omega: 13.0,
    zeta: 1.0,
};

/// Moves a stretch of `over` past an end by `pull` pixels, with more
/// resistance the further it already is.
fn stretch_by(over: f32, pull: f32) -> f32 {
    // Work in the unresisted distance `d`, where `over = STRETCH * d / (d + STRETCH)`.
    let magnitude = over.abs().min(STRETCH * 0.999);
    let raw = magnitude * STRETCH / (STRETCH - magnitude);
    let raw = (raw * over.signum() + pull * 0.55).clamp(-1.0e6, 1.0e6);
    let resisted = STRETCH * raw.abs() / (raw.abs() + STRETCH);
    resisted * raw.signum()
}

#[derive(Debug, Clone)]
struct Drag {
    start: f32,
    last: f32,
    /// The finger has moved past the slop and owns the gesture.
    scrolling: bool,
    /// Recent `(time, finger y)` samples for the release velocity.
    samples: Vec<(Instant, f32)>,
}

#[derive(Debug, Clone, Copy)]
struct Fling {
    started: Instant,
    from: f32,
    /// Pixels per second, positive towards the end of the content.
    velocity: f32,
}

impl Fling {
    /// How far the fling has travelled after `elapsed` seconds, and whether
    /// it is still moving.
    fn travel(&self, elapsed: f32) -> (f32, bool) {
        let decay = (-FRICTION * elapsed).exp();
        let distance = self.velocity / FRICTION * (1.0 - decay);
        let moving = (self.velocity * decay).abs() >= MIN_FLING / 4.0;

        (distance, moving)
    }
}

/// Vertical scrolling: the offset, wheel input, touch dragging with slop
/// and flings. Shared by [`scroll`] and [`list`](super::list).
#[derive(Debug)]
pub(crate) struct Scroller {
    /// A cell so a fling can advance while painting, which only borrows.
    offset: Cell<f32>,
    /// The scrolled content's height and the visible height.
    content: f32,
    viewport: f32,
    /// A finger on the area that may become, or has become, a scroll.
    drag: Option<Drag>,
    /// Momentum after the finger left.
    fling: Cell<Option<Fling>>,
    /// How far the content is pulled past an end, positive past the end
    /// and negative above the start; springs back to zero.
    over: Cell<look::Spring>,
    /// The stretch at the last [`Scroller::advance`], for
    /// [`Scroller::shown_offset`], which has no clock.
    stretched: Cell<f32>,
    /// Scrolling changes which children exist (a lazy list), so it needs a
    /// layout rather than just a paint.
    relayout: bool,
}

impl Default for Scroller {
    fn default() -> Self {
        Self {
            offset: Cell::new(0.0),
            content: 0.0,
            viewport: 0.0,
            drag: None,
            fling: Cell::new(None),
            over: Cell::new(look::Spring::new(0.0, SPRING_BACK)),
            stretched: Cell::new(0.0),
            relayout: false,
        }
    }
}

impl Scroller {
    /// A scroller whose content changes as it scrolls (a lazy list), so
    /// every move asks for a layout.
    pub fn relayouting() -> Self {
        Self {
            relayout: true,
            ..Self::default()
        }
    }

    /// Asks for what a moved offset needs.
    fn moved(&self, cx: &mut EventCx<'_>) {
        if self.relayout {
            cx.request_layout();
        } else {
            cx.request_paint();
        }
    }

    pub fn offset(&self) -> f32 {
        self.offset.get()
    }

    /// The offset content is drawn at: [`Scroller::offset`] plus any
    /// stretch past an end.
    pub fn shown_offset(&self) -> f32 {
        self.offset.get() + self.stretched.get()
    }

    /// Whether a fling or a stretch is still moving.
    pub fn is_moving(&self, now: Instant) -> bool {
        self.is_flinging() || self.over.get().is_moving(now)
    }

    /// Follows a dragging finger `delta` pixels (positive downwards): the
    /// content scrolls, and past an end it stretches with resistance.
    fn drag_by(&mut self, delta: f32, now: Instant) -> bool {
        let mut spring = self.over.get();
        let mut over = spring.value(now);
        // Positive moves towards the end of the content.
        let mut want = -delta;

        // Pulling back from a stretch eases it first.
        if over != 0.0 && want.signum() != over.signum() {
            let back = want.abs().min(over.abs());
            over -= back * over.signum();
            want -= back * want.signum();
        }
        let before = self.offset.get();
        let moved = self.scroll_by(-want);
        want -= self.offset.get() - before;
        if want.abs() > f32::EPSILON {
            over = stretch_by(over, want);
        }
        let stretched = over != spring.value(now);
        spring.set(over);
        self.over.set(spring);
        self.stretched.set(over);
        moved || stretched
    }

    /// Lets a stretch spring back.
    fn let_go(&self, now: Instant) {
        let mut spring = self.over.get();
        if spring.value(now) != 0.0 || spring.target() != 0.0 {
            spring.go(0.0, now);
            self.over.set(spring);
        }
    }

    /// Records the content and visible heights, keeping the offset in range.
    pub fn set_extent(&mut self, content: f32, viewport: f32) {
        self.content = content;
        self.viewport = viewport;
        self.offset
            .set(self.offset.get().clamp(0.0, self.max_offset()));
    }

    pub fn max_offset(&self) -> f32 {
        (self.content - self.viewport).max(0.0)
    }

    fn scroll_by(&mut self, delta: f32) -> bool {
        let current = self.offset.get();
        let next = (current - delta).clamp(0.0, self.max_offset());
        self.offset.set(next);
        next != current
    }

    /// Advances a running fling to `now`. Returns whether the offset moved.
    pub fn advance(&self, now: Instant) -> bool {
        self.stretched.set(self.over.get().value(now));
        let Some(fling) = self.fling.get() else {
            return false;
        };

        let elapsed = now.saturating_duration_since(fling.started).as_secs_f32();
        let (distance, moving) = fling.travel(elapsed);
        let max = self.max_offset();
        let next = (fling.from + distance).clamp(0.0, max);
        let moved = next != self.offset.get();
        self.offset.set(next);

        // A fling that reaches either end stops there.
        let at_end = (next <= 0.0 && fling.velocity < 0.0) || (next >= max && fling.velocity > 0.0);
        if !moving || at_end {
            self.fling.set(None);
        }
        // Hitting an end at speed carries the content a little past it,
        // and it springs back.
        if at_end && moving {
            let left = fling.velocity * (-FRICTION * elapsed).exp();
            let mut spring = self.over.get();
            spring.kick((left * BOUNCE_CARRY).clamp(-MAX_BOUNCE, MAX_BOUNCE), now);
            self.over.set(spring);
        }
        moved
    }

    pub fn is_flinging(&self) -> bool {
        self.fling.get().is_some()
    }

    /// The finger's speed over the last moments of the drag, in pixels per
    /// second, positive when the content should move towards its end.
    fn release_velocity(samples: &[(Instant, f32)]) -> f32 {
        let (Some(&(last_at, last_y)), true) = (samples.last(), samples.len() > 1) else {
            return 0.0;
        };

        let Some(&(first_at, first_y)) = samples.iter().find(|(at, _)| {
            last_at.saturating_duration_since(*at).as_secs_f32() <= VELOCITY_WINDOW
        }) else {
            return 0.0;
        };

        let elapsed = last_at.saturating_duration_since(first_at).as_secs_f32();
        if elapsed <= f32::EPSILON {
            return 0.0;
        }

        // A finger moving up scrolls towards the end.
        ((first_y - last_y) / elapsed).clamp(-MAX_FLING, MAX_FLING)
    }

    /// Takes a finger drag once it is clearly a scroll; see
    /// [`View::intercept`].
    pub fn intercept(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        if !cx.is_touch() {
            return Handled::No;
        }
        // The system took the gesture: drop the drag, never fling from it.
        if crate::tree::is_cancellation(event) {
            self.drag = None;
            self.let_go(cx.now());
            return Handled::No;
        }
        let now = cx.now();

        match look::gesture(event, cx.pointer()) {
            Some(Gesture::Down(at)) if cx.is_hovered() => {
                // A finger landing on a moving list stops it, and only that:
                // it must not press whatever happened to be underneath.
                let caught = self.fling.get().is_some() || self.over.get().is_moving(now);
                let _ = self.advance(now);
                self.fling.set(None);
                // Hold a springing stretch where the finger caught it.
                let mut spring = self.over.get();
                spring.set(spring.value(now));
                self.over.set(spring);

                self.drag = Some(Drag {
                    start: at.y,
                    last: at.y,
                    scrolling: caught,
                    samples: vec![(now, at.y)],
                });

                Handled::from(caught)
            }
            Some(Gesture::Move(at)) => {
                let max = self.max_offset();
                let Some(drag) = &mut self.drag else {
                    return Handled::No;
                };

                if !drag.scrolling {
                    if (at.y - drag.start).abs() <= SLOP {
                        return Handled::No;
                    }

                    // Only a drag this area can follow is a scroll; one it
                    // cannot (down at the top, up at the end) is left to an
                    // ancestor, such as pull-to-refresh.
                    let offset = self.offset.get();
                    let towards_end = at.y < drag.start;
                    let movable = if towards_end {
                        offset < max
                    } else {
                        offset > 0.0
                    };
                    if !movable {
                        self.drag = None;
                        return Handled::No;
                    }
                    drag.scrolling = true;
                    // Start from where the finger crossed the slop, so the
                    // content does not jump by the slop distance.
                    drag.last = at.y;
                }

                let delta = at.y - drag.last;
                drag.last = at.y;
                drag.samples.push((now, at.y));
                if drag.samples.len() > 32 {
                    let _ = drag.samples.drain(..16);
                }

                if self.drag_by(delta, now) {
                    self.moved(cx);
                }
                Handled::Yes
            }
            Some(Gesture::Up(_)) => {
                let Some(mut drag) = self.drag.take() else {
                    return Handled::No;
                };
                if !drag.scrolling {
                    self.let_go(now);
                    return Handled::No;
                }
                self.let_go(now);
                if self.over.get().is_moving(now) {
                    cx.request_animation();
                }

                // The finger is still where it last moved to; a finger that
                // rested before lifting has no speed left to fling with.
                drag.samples.push((now, drag.last));
                let velocity = Self::release_velocity(&drag.samples);
                // Let go while stretched, it only springs back.
                if velocity.abs() >= MIN_FLING && self.stretched.get() == 0.0 {
                    self.fling.set(Some(Fling {
                        started: now,
                        from: self.offset.get(),
                        velocity,
                    }));
                    cx.request_animation();
                }
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    /// Scrolls for a mouse wheel or trackpad over the area.
    pub fn wheel(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event else {
            return Handled::No;
        };
        if !cx.is_hovered() {
            return Handled::No;
        }

        // The wheel takes over from any fling.
        let _ = self.advance(cx.now());
        self.fling.set(None);

        let pixels = match *delta {
            mouse::ScrollDelta::Lines { y, .. } => y * LINE,
            mouse::ScrollDelta::Pixels { y, .. } => y,
        };

        if self.scroll_by(pixels) {
            self.moved(cx);
        }
        Handled::Yes
    }

    /// Draws the scroll bar over `cx`'s bounds.
    pub fn paint_bar(&self, cx: &mut PaintCx<'_>) {
        let total = self.content;
        if total <= self.viewport || total <= 0.0 {
            return;
        }

        let bounds = cx.bounds();
        let length = (bounds.height * bounds.height / total).max(24.0);
        let travel = bounds.height - length;
        let top = bounds.y + travel * (self.offset.get() / self.max_offset());
        let bar = Rectangle::new(
            Point::new(bounds.x + bounds.width - BAR - 2.0, top),
            Size::new(BAR, length),
        );

        let color = cx
            .theme()
            .palette()
            .background
            .strongest
            .color
            .scale_alpha(0.5);
        cx.fill(bar, color, Border::default().rounded(BAR / 2.0));
    }
}

impl View for Scroll {
    fn sizing(&self) -> Sizing {
        Sizing::FILL
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let outer = constraints.with_sizing(Sizing::FILL);
        let room = Size::new(outer.max.width, f32::INFINITY);
        let content = self.content.measure(cx, Constraints::loose(room));

        self.viewport = Size::new(
            if outer.max.width.is_finite() {
                outer.max.width
            } else {
                content.width
            },
            if outer.max.height.is_finite() {
                outer.max.height
            } else {
                content.height
            },
        );
        self.content.place(Point::ORIGIN);
        self.scroller
            .set_extent(self.content.size().height, self.viewport.height);
        self.viewport
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let _ = self.scroller.advance(cx.now());
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
        visit(&mut self.content);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Duration;

    #[test]
    fn a_stretch_resists_more_the_further_it_goes_and_never_runs_away() {
        let first = stretch_by(0.0, 40.0);
        let second = stretch_by(first, 40.0) - first;
        assert!(first > 0.0 && second > 0.0 && second < first);
        assert!(stretch_by(0.0, 1.0e7) < STRETCH);
        assert!(stretch_by(0.0, -40.0) < 0.0);
    }

    #[test]
    fn a_fling_into_the_top_bounces_past_it_and_springs_back() {
        let mut scroller = Scroller::default();
        scroller.set_extent(2_000.0, 400.0);
        scroller.offset.set(20.0);
        let start = Instant::now();
        scroller.fling.set(Some(Fling {
            started: start,
            from: 20.0,
            velocity: -3_000.0,
        }));

        // It reaches the top within a few frames and carries on past it.
        let mut now = start;
        for _ in 0..4 {
            now += Duration::from_millis(16);
            let _ = scroller.advance(now);
        }
        assert!(!scroller.is_flinging() && scroller.offset() == 0.0);
        assert!(scroller.is_moving(now));
        let _ = scroller.advance(now + Duration::from_millis(60));
        let past = scroller.shown_offset();
        assert!(past < -5.0, "the content shows past the top: {past}");

        let later = now + Duration::from_secs(2);
        let _ = scroller.advance(later);
        assert!(scroller.shown_offset().abs() < 0.01 && !scroller.is_moving(later));
    }

    #[test]
    fn dragging_past_the_end_stretches_and_dragging_back_undoes_it_first() {
        let mut scroller = Scroller::default();
        scroller.set_extent(500.0, 400.0);
        let now = Instant::now();
        // Up by 150: 100 of scrolling, then a stretch past the end.
        assert!(scroller.drag_by(-150.0, now));
        assert_eq!(scroller.offset(), 100.0);
        let stretched = scroller.shown_offset() - 100.0;
        assert!(stretched > 0.0 && stretched < 50.0);

        // Back down: the stretch goes before the content scrolls.
        let _ = scroller.drag_by(stretched, now);
        assert_eq!(scroller.offset(), 100.0);
        assert!((scroller.shown_offset() - 100.0).abs() < 1.0e-3);
    }
}

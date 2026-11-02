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
        offset: Cell::new(0.0),
        viewport: Size::ZERO,
        id: widget::Id::unique(),
        drag: None,
        fling: Cell::new(None),
    }
}

/// See [`scroll`].
pub struct Scroll {
    content: Child,
    /// A cell so a fling can advance while painting, which only borrows.
    offset: Cell<f32>,
    viewport: Size,
    id: widget::Id,
    /// A finger on the area that may become, or has become, a scroll.
    drag: Option<Drag>,
    /// Momentum after the finger left.
    fling: Cell<Option<Fling>>,
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

impl Scroll {
    fn max_offset(&self) -> f32 {
        (self.content.size().height - self.viewport.height).max(0.0)
    }

    fn scroll_by(&mut self, delta: f32) -> bool {
        let current = self.offset.get();
        let next = (current - delta).clamp(0.0, self.max_offset());
        self.offset.set(next);
        next != current
    }

    /// Advances a running fling to `now`.
    fn advance(&self, now: Instant) {
        let Some(fling) = self.fling.get() else {
            return;
        };

        let elapsed = now.saturating_duration_since(fling.started).as_secs_f32();
        let (distance, moving) = fling.travel(elapsed);
        let max = self.max_offset();
        let next = (fling.from + distance).clamp(0.0, max);
        self.offset.set(next);

        // A fling that reaches either end stops there.
        let at_end = (next <= 0.0 && fling.velocity < 0.0) || (next >= max && fling.velocity > 0.0);
        if !moving || at_end {
            self.fling.set(None);
        }
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
        self.offset.set(self.offset.get().min(self.max_offset()));
        self.viewport
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        self.advance(cx.now());
    }

    fn paint_over(&self, cx: &mut PaintCx<'_>) {
        let total = self.content.size().height;
        if total <= self.viewport.height || total <= 0.0 {
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

    fn intercept(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        if !cx.is_touch() {
            return Handled::No;
        }
        let now = cx.now();

        match look::gesture(event, cx.pointer()) {
            Some(Gesture::Down(at)) if cx.is_hovered() => {
                // A finger landing on a moving list stops it, and only that:
                // it must not press whatever happened to be underneath.
                let caught = self.fling.get().is_some();
                self.advance(now);
                self.fling.set(None);

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

                if self.scroll_by(delta) {
                    cx.request_paint();
                }
                Handled::Yes
            }
            Some(Gesture::Up(_)) => {
                let Some(mut drag) = self.drag.take() else {
                    return Handled::No;
                };
                if !drag.scrolling {
                    return Handled::No;
                }

                // The finger is still where it last moved to; a finger that
                // rested before lifting has no speed left to fling with.
                drag.samples.push((now, drag.last));
                let velocity = Self::release_velocity(&drag.samples);
                if velocity.abs() >= MIN_FLING {
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

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event else {
            return Handled::No;
        };
        if !cx.is_hovered() {
            return Handled::No;
        }

        // The wheel takes over from any fling.
        self.advance(cx.now());
        self.fling.set(None);

        let pixels = match *delta {
            mouse::ScrollDelta::Lines { y, .. } => y * LINE,
            mouse::ScrollDelta::Pixels { y, .. } => y,
        };

        if self.scroll_by(pixels) {
            cx.request_paint();
        }
        Handled::Yes
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
        Vector::new(0.0, self.offset.get())
    }

    fn is_animating(&self, _now: Instant) -> bool {
        self.fling.get().is_some()
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        visit(&mut self.content);
    }
}

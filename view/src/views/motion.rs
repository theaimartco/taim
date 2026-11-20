//! Motion for any view: [`animated`] glides content to wherever layout moves
//! it, and [`appear`] brings content in with a fade and a rise.
//!
//! Both only change how content is drawn, never its layout, and both
//! respect reduced motion: with it, moved content is simply where it
//! belongs and appearing content only fades in, briefly.

use std::cell::Cell;

use super::look::{GLIDE, Spring, Tuning};
use crate::cx::{DescribeCx, MeasureCx, PaintCx};
use crate::layout::{Constraints, Sizing};
use crate::{AnyView, Child, Duration, Instant, Point, Size, Vector, View, Visual, VisualCx};
use taim_core::semantics;

/// A rise with a soft landing, a touch livelier than [`GLIDE`].
const RISE: Tuning = Tuning {
    omega: 15.0,
    zeta: 0.78,
};

/// Glides `content` to its new place whenever layout moves it (a row
/// reordered, an item above removed, a panel opening) instead of jumping
/// there. A move during a glide continues from where the content is drawn,
/// at the speed it had.
pub fn animated(content: impl Into<AnyView>) -> Animated {
    Animated {
        content: Child::new(content),
        x: Cell::new(Spring::new(0.0, GLIDE)),
        y: Cell::new(Spring::new(0.0, GLIDE)),
        placed: Cell::new(None),
    }
}

/// See [`animated`].
pub struct Animated {
    content: Child,
    /// How far the content is drawn from where layout put it.
    x: Cell<Spring>,
    y: Cell<Spring>,
    /// Where layout last put it.
    placed: Cell<Option<Point>>,
}

impl Animated {
    /// Restarts the glide on one axis from `from` pixels away, keeping its
    /// speed.
    fn glide(axis: &Cell<Spring>, from: f32, now: Instant) {
        let mut spring = axis.get();
        let velocity = spring.velocity(now);
        spring.set(from);
        spring.go(0.0, now);
        spring.kick(velocity, now);
        axis.set(spring);
    }
}

impl View for Animated {
    fn sizing(&self) -> Sizing {
        self.content.sizing()
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let size = self.content.measure(cx, constraints);
        self.content.place(Point::ORIGIN);
        size
    }

    fn paint(&self, _cx: &mut PaintCx<'_>) {}

    fn describe(&self, _cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.x.get().is_moving(now) || self.y.get().is_moving(now)
    }

    fn visual(&self, cx: &VisualCx) -> Visual {
        let now = cx.now();
        let origin = cx.frame().position();
        if let Some(last) = self.placed.get()
            && last != origin
        {
            // Drawn where it was, then glide home.
            Self::glide(&self.x, last.x + self.x.get().value(now) - origin.x, now);
            Self::glide(&self.y, last.y + self.y.get().value(now) - origin.y, now);
        }
        self.placed.set(Some(origin));
        Visual {
            offset: Vector::new(self.x.get().value(now), self.y.get().value(now)),
            ..Visual::IDENTITY
        }
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        visit(&mut self.content);
    }
}

/// Brings `content` in the first time it is drawn: it fades in while
/// rising a few pixels and settling to full size. Inside
/// [`when`](super::when) it plays each time the content is shown; give a
/// list's items growing [`delay`](Appear::delay)s to cascade them.
pub fn appear(content: impl Into<AnyView>) -> Appear {
    Appear {
        content: Child::new(content),
        delay: Duration::ZERO,
        rise: 14.0,
        progress: Cell::new(Spring::new(0.0, RISE)),
        started: Cell::new(None),
    }
}

/// See [`appear`].
pub struct Appear {
    content: Child,
    delay: Duration,
    rise: f32,
    progress: Cell<Spring>,
    /// When it starts coming in: first drawn plus the delay.
    started: Cell<Option<Instant>>,
}

/// With reduced motion, content only fades in, this quickly.
const REDUCED_FADE: Duration = Duration::from_millis(150);

impl Appear {
    /// Waits `delay` after it is first drawn before coming in.
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// Rises `pixels` into place; negative drops in from above, zero only
    /// fades and scales.
    pub fn rise(mut self, pixels: f32) -> Self {
        self.rise = if pixels.is_finite() { pixels } else { 0.0 };
        self
    }
}

impl View for Appear {
    fn sizing(&self) -> Sizing {
        self.content.sizing()
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let size = self.content.measure(cx, constraints);
        self.content.place(Point::ORIGIN);
        size
    }

    fn paint(&self, _cx: &mut PaintCx<'_>) {}

    fn describe(&self, _cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }

    fn is_animating(&self, now: Instant) -> bool {
        match self.started.get() {
            None => true,
            Some(start) if crate::state::motion_reduced() => now < start + REDUCED_FADE,
            Some(_) => self.progress.get().is_moving(now),
        }
    }

    fn visual(&self, cx: &VisualCx) -> Visual {
        let now = cx.now();
        let start = *self.started.get().get_or_insert_with(|| {
            let start = now + self.delay;
            let mut progress = self.progress.get();
            // A spring started in the future holds still until then.
            progress.go(1.0, start);
            self.progress.set(progress);
            start
        });
        self.started.set(Some(start));
        // Reduced motion: no movement, just a short fade so the change
        // still reads.
        if crate::state::motion_reduced() {
            let faded =
                now.saturating_duration_since(start).as_secs_f32() / REDUCED_FADE.as_secs_f32();
            return Visual {
                opacity: faded.clamp(0.0, 1.0),
                ..Visual::IDENTITY
            };
        }
        let shown = self.progress.get().value(now);
        let left = 1.0 - shown;
        Visual {
            offset: Vector::new(0.0, self.rise * left),
            scale: 1.0 - 0.04 * left,
            opacity: shown.clamp(0.0, 1.0),
        }
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        visit(&mut self.content);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rectangle;
    use crate::views::space;

    fn at(y: f32) -> Rectangle {
        Rectangle::new(Point::new(0.0, y), Size::new(10.0, 10.0))
    }

    #[test]
    fn moved_content_is_drawn_where_it_was_then_glides_home() {
        let view = animated(space());
        let start = Instant::now();
        assert!(
            view.visual(&VisualCx::new(start, at(0.0), at(0.0)))
                .is_identity()
        );

        // Layout moves it 100 pixels down: it is still drawn at the top.
        let jumped = view.visual(&VisualCx::new(start, at(100.0), at(100.0)));
        assert!((jumped.offset.y + 100.0).abs() < 1.0e-3);
        assert!(view.is_animating(start));

        let midway = view.visual(&VisualCx::new(
            start + Duration::from_millis(80),
            at(100.0),
            at(100.0),
        ));
        assert!(midway.offset.y > -100.0 && midway.offset.y < 0.0);

        let later = start + Duration::from_secs(2);
        assert!(
            view.visual(&VisualCx::new(later, at(100.0), at(100.0)))
                .offset
                .y
                .abs()
                < 0.01
        );
        assert!(!view.is_animating(later));
    }

    #[test]
    fn a_move_during_a_glide_continues_from_where_it_is_drawn() {
        let view = animated(space());
        let start = Instant::now();
        let _ = view.visual(&VisualCx::new(start, at(0.0), at(0.0)));
        let _ = view.visual(&VisualCx::new(start, at(100.0), at(100.0)));
        let soon = start + Duration::from_millis(60);
        let drawn = 100.0
            + view
                .visual(&VisualCx::new(soon, at(100.0), at(100.0)))
                .offset
                .y;

        // Moved back: no jump on screen.
        let back = view.visual(&VisualCx::new(soon, at(0.0), at(0.0)));
        assert!((back.offset.y - drawn).abs() < 1.0e-2);
    }

    #[test]
    fn appearing_content_fades_in_rises_and_settles() {
        let view = appear(space()).delay(Duration::from_millis(100));
        let start = Instant::now();
        let first = view.visual(&VisualCx::new(start, at(0.0), at(0.0)));
        assert_eq!(first.opacity, 0.0);
        assert!(first.offset.y > 10.0);
        assert!(view.is_animating(start));

        // Still waiting out the delay.
        let waiting = view.visual(&VisualCx::new(
            start + Duration::from_millis(50),
            at(0.0),
            at(0.0),
        ));
        assert_eq!(waiting.opacity, 0.0);

        let coming = view.visual(&VisualCx::new(
            start + Duration::from_millis(220),
            at(0.0),
            at(0.0),
        ));
        assert!(coming.opacity > 0.3 && coming.offset.y < first.offset.y);

        let later = start + Duration::from_secs(2);
        let settled = view.visual(&VisualCx::new(later, at(0.0), at(0.0)));
        assert!((settled.opacity - 1.0).abs() < 1.0e-3);
        assert!(settled.offset.y.abs() < 0.05 && (settled.scale - 1.0).abs() < 1.0e-3);
        assert!(!view.is_animating(later));
    }

    #[test]
    fn with_reduced_motion_content_is_simply_there() {
        let _reduced = crate::state::MotionGuard::enter(true);
        let start = Instant::now();
        let view = appear(space());
        let first = view.visual(&VisualCx::new(start, at(0.0), at(0.0)));
        assert!(first.offset == Vector::ZERO && first.scale == 1.0 && first.opacity == 0.0);
        let later = start + Duration::from_millis(200);
        assert!(
            view.visual(&VisualCx::new(later, at(0.0), at(0.0)))
                .is_identity()
        );
        assert!(!view.is_animating(later));

        let moving = animated(space());
        let _ = moving.visual(&VisualCx::new(start, at(0.0), at(0.0)));
        assert!(
            moving
                .visual(&VisualCx::new(start, at(50.0), at(50.0)))
                .is_identity()
        );
    }
}

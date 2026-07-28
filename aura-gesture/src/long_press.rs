//! Long-press recognition with a deadline/movement policy.
use crate::recognizer::{Disposition, Recognizer, Verdict};
use aura_core::Point;
use aura_core::pointer::{Id as PointerId, Phase, Pointer};
use aura_core::time::{Duration, Instant};

/// The default hold duration a pointer must remain down for, before a
/// [`LongPress`] accepts.
pub const DEFAULT_DURATION: Duration = Duration::from_millis(500);

/// The default movement tolerance, in logical pixels, a [`LongPress`]
/// allows before rejecting -- deliberately tighter than
/// [`crate::tap::DEFAULT_SLOP`], since a long-press is a stationary
/// gesture by definition and should lose readily to a
/// [`crate::pan`]/[`crate::swipe`] that starts moving.
pub const DEFAULT_SLOP: f32 = 8.0;

/// Recognizes a long-press: one pointer, held down without moving more
/// than [`LongPress::slop`] for at least [`LongPress::duration`].
///
/// Unlike [`crate::tap::Tap`] (which only ever resolves in response to a
/// real [`Pointer`] event), [`LongPress`] typically accepts via
/// [`Recognizer::poll`]: the deadline elapsing is not, itself, a pointer
/// event, so [`crate::arena::Arena::tick`] is what actually lets this
/// recognizer win once its hold duration has passed with the pointer
/// still perfectly still.
pub struct LongPress {
    duration: Duration,
    slop: f32,
    tracked: Option<PointerId>,
    origin: Option<Point>,
    started_at: Option<Instant>,
    rejected: bool,
}

impl Default for LongPress {
    fn default() -> Self {
        Self::new()
    }
}

impl LongPress {
    /// Creates a [`LongPress`] recognizer using the default duration and
    /// slop.
    pub fn new() -> Self {
        Self {
            duration: DEFAULT_DURATION,
            slop: DEFAULT_SLOP,
            tracked: None,
            origin: None,
            started_at: None,
            rejected: false,
        }
    }

    /// Sets the required hold duration.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Sets the movement tolerance, in logical pixels.
    pub fn slop(mut self, slop: f32) -> Self {
        self.slop = slop;
        self
    }

    fn check(&self, position: Point, now: Instant) -> Disposition {
        let Some(origin) = self.origin else {
            return Disposition::Rejected;
        };
        let Some(started_at) = self.started_at else {
            return Disposition::Rejected;
        };

        if position.distance(origin) > self.slop {
            return Disposition::Rejected;
        }

        if now.saturating_duration_since(started_at) >= self.duration {
            Disposition::Accepted
        } else {
            Disposition::Pending
        }
    }
}

impl Recognizer for LongPress {
    fn name(&self) -> &'static str {
        "long_press"
    }

    fn add_pointer(&mut self, pointer: Pointer) {
        if self.tracked.is_some() {
            // A long-press is single-pointer, like a tap; a second
            // concurrent contact means this is some other gesture
            // (a pinch/rotate) instead.
            self.rejected = true;
            return;
        }

        self.tracked = Some(pointer.id);
        self.origin = Some(pointer.position);
        self.started_at = Some(pointer.timestamp);
    }

    fn remove_pointer(&mut self, id: PointerId) {
        if self.tracked == Some(id) {
            self.tracked = None;
        }
    }

    fn on_event(&mut self, pointer: Pointer) -> Disposition {
        if self.rejected || self.tracked != Some(pointer.id) {
            return Disposition::Rejected;
        }

        if pointer.phase == Phase::Cancel {
            return Disposition::Rejected;
        }

        if pointer.phase == Phase::Up {
            // Released before the deadline: a long-press never accepts
            // on an `Up` event itself, only via `poll` once its
            // duration has actually elapsed. `Up` before that point is
            // always too early.
            return Disposition::Rejected;
        }

        self.check(pointer.position, pointer.timestamp)
    }

    fn poll(&mut self, now: Instant) -> Disposition {
        if self.rejected {
            return Disposition::Rejected;
        }

        let Some(origin) = self.origin else {
            return Disposition::Pending;
        };

        self.check(origin, now)
    }

    fn on_verdict(&mut self, _verdict: Verdict) {}

    fn reset(&mut self) {
        self.tracked = None;
        self.origin = None;
        self.started_at = None;
        self.rejected = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aura_core::pointer::{DeviceKind, Metadata};

    fn epoch() -> Instant {
        static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        *EPOCH.get_or_init(Instant::now)
    }

    fn t(millis: u64) -> Instant {
        epoch() + Duration::from_millis(millis)
    }

    fn pointer(phase: Phase, position: Point, millis: u64) -> Pointer {
        Pointer {
            id: PointerId::MOUSE,
            device: DeviceKind::Mouse,
            phase,
            position,
            timestamp: t(millis),
            metadata: Metadata::NONE,
            captured: false,
        }
    }

    #[test]
    fn accepts_via_poll_once_the_duration_elapses_with_no_movement() {
        let mut long_press = LongPress::new().duration(Duration::from_millis(500));
        long_press.add_pointer(pointer(Phase::Down, Point::new(10.0, 10.0), 0));

        assert_eq!(long_press.poll(t(400)), Disposition::Pending);
        assert_eq!(long_press.poll(t(500)), Disposition::Accepted);
    }

    #[test]
    fn movement_beyond_slop_rejects() {
        let mut long_press = LongPress::new();
        long_press.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));

        let disposition = long_press.on_event(pointer(Phase::Move, Point::new(50.0, 0.0), 10));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn a_release_before_the_deadline_rejects() {
        let mut long_press = LongPress::new().duration(Duration::from_millis(500));
        long_press.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));

        let disposition = long_press.on_event(pointer(Phase::Up, Point::new(0.0, 0.0), 100));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn cancel_rejects() {
        let mut long_press = LongPress::new();
        long_press.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));

        let disposition = long_press.on_event(pointer(Phase::Cancel, Point::new(0.0, 0.0), 10));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn a_second_concurrent_pointer_rejects() {
        let mut long_press = LongPress::new();
        long_press.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));
        long_press.add_pointer(Pointer {
            id: PointerId::from_raw(2),
            ..pointer(Phase::Down, Point::new(5.0, 5.0), 1)
        });

        assert_eq!(long_press.poll(t(1000)), Disposition::Rejected);
    }

    #[test]
    fn small_movement_within_slop_still_accepts() {
        let mut long_press = LongPress::new().duration(Duration::from_millis(200));
        long_press.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));

        let disposition = long_press.on_event(pointer(Phase::Move, Point::new(2.0, 0.0), 50));
        assert_eq!(disposition, Disposition::Pending);

        assert_eq!(long_press.poll(t(200)), Disposition::Accepted);
    }

    #[test]
    fn reset_allows_reuse_for_a_new_contact() {
        let mut long_press = LongPress::new().duration(Duration::from_millis(100));
        long_press.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));
        let _ = long_press.on_event(pointer(Phase::Move, Point::new(50.0, 0.0), 10));

        long_press.reset();
        long_press.add_pointer(pointer(Phase::Down, Point::new(100.0, 100.0), 100));

        assert_eq!(long_press.poll(t(300)), Disposition::Accepted);
    }
}

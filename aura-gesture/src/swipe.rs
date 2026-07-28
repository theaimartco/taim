//! Swipe recognition: direction, distance, and velocity rules.
use crate::recognizer::{Disposition, Recognizer, Verdict};
use crate::velocity::Estimator;
use aura_core::pointer::{Id as PointerId, Phase, Pointer};
use aura_core::{Point, Vector};

/// The minimum total distance a pointer must travel before its release
/// can be recognized as a swipe, in logical pixels.
///
/// Distinguishes a swipe from a fast, short flick that a
/// [`crate::tap`] should win instead -- a swipe is a deliberate,
/// sustained motion, not just quick.
pub const DEFAULT_MIN_DISTANCE: f32 = 40.0;

/// The minimum speed, in logical pixels per second, a pointer must be
/// moving at release for a [`Swipe`] to accept.
///
/// Distinguishes a swipe from a slow pan-and-stop covering the same
/// distance -- the release velocity is what actually signals "the user
/// intended a fling," not just the total distance traveled.
pub const DEFAULT_MIN_VELOCITY: f32 = 300.0;

/// The four cardinal directions a [`Swipe`] can resolve to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Predominantly rightward (+X) movement.
    Right,
    /// Predominantly leftward (-X) movement.
    Left,
    /// Predominantly downward (+Y) movement.
    Down,
    /// Predominantly upward (-Y) movement.
    Up,
}

impl Direction {
    /// Resolves the dominant cardinal [`Direction`] of `vector`, used by
    /// both [`Swipe`] and [`crate::edge_swipe::EdgeSwipe`] to classify a
    /// release displacement.
    pub(crate) fn from_vector(vector: Vector) -> Self {
        if vector.x.abs() >= vector.y.abs() {
            if vector.x >= 0.0 {
                Direction::Right
            } else {
                Direction::Left
            }
        } else if vector.y >= 0.0 {
            Direction::Down
        } else {
            Direction::Up
        }
    }
}

/// Recognizes a swipe: one pointer released with both a minimum total
/// distance and a minimum release velocity, resolving to one of the
/// four cardinal [`Direction`]s.
///
/// Unlike [`crate::pan`] (which accepts the moment slop is crossed, so a
/// scrollable can start tracking the drag live), a [`Swipe`] only ever
/// resolves on release -- there is no such thing as a swipe still in
/// progress, only one that already happened or didn't.
pub struct Swipe {
    min_distance: f32,
    min_velocity: f32,
    tracked: Option<PointerId>,
    origin: Option<Point>,
    estimator: Estimator,
    direction: Option<Direction>,
    rejected: bool,
}

impl Default for Swipe {
    fn default() -> Self {
        Self::new()
    }
}

impl Swipe {
    /// Creates a [`Swipe`] recognizer using the default minimum
    /// distance and velocity.
    pub fn new() -> Self {
        Self {
            min_distance: DEFAULT_MIN_DISTANCE,
            min_velocity: DEFAULT_MIN_VELOCITY,
            tracked: None,
            origin: None,
            estimator: Estimator::new(),
            direction: None,
            rejected: false,
        }
    }

    /// Sets the minimum total distance.
    pub fn min_distance(mut self, distance: f32) -> Self {
        self.min_distance = distance;
        self
    }

    /// Sets the minimum release velocity.
    pub fn min_velocity(mut self, velocity: f32) -> Self {
        self.min_velocity = velocity;
        self
    }

    /// Returns the resolved [`Direction`], once this [`Swipe`] has
    /// accepted; `None` before then.
    pub fn direction(&self) -> Option<Direction> {
        self.direction
    }
}

impl Recognizer for Swipe {
    fn name(&self) -> &'static str {
        "swipe"
    }

    fn add_pointer(&mut self, pointer: Pointer) {
        if self.tracked.is_some() {
            self.rejected = true;
            return;
        }

        self.tracked = Some(pointer.id);
        self.origin = Some(pointer.position);
        self.estimator.push(pointer.position, pointer.timestamp);
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

        self.estimator.push(pointer.position, pointer.timestamp);

        if pointer.phase != Phase::Up {
            return Disposition::Pending;
        }

        let Some(origin) = self.origin else {
            return Disposition::Rejected;
        };

        let displacement = pointer.position - origin;
        let distance = pointer.position.distance(origin);

        if distance < self.min_distance {
            return Disposition::Rejected;
        }

        let Some(velocity) = self.estimator.estimate() else {
            return Disposition::Rejected;
        };

        let speed = (velocity.x * velocity.x + velocity.y * velocity.y).sqrt();

        if speed < self.min_velocity {
            return Disposition::Rejected;
        }

        self.direction = Some(Direction::from_vector(displacement));
        Disposition::Accepted
    }

    fn on_verdict(&mut self, _verdict: Verdict) {}

    fn reset(&mut self) {
        self.tracked = None;
        self.origin = None;
        self.estimator.reset();
        self.direction = None;
        self.rejected = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aura_core::pointer::{DeviceKind, Metadata};
    use aura_core::time::{Duration, Instant};

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
    fn a_fast_far_rightward_release_accepts_as_right() {
        let mut swipe = Swipe::new();
        swipe.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));
        let _ = swipe.on_event(pointer(Phase::Move, Point::new(50.0, 0.0), 20));

        let disposition = swipe.on_event(pointer(Phase::Up, Point::new(100.0, 0.0), 40));

        assert_eq!(disposition, Disposition::Accepted);
        assert_eq!(swipe.direction(), Some(Direction::Right));
    }

    #[test]
    fn a_slow_far_release_rejects_for_insufficient_velocity() {
        let mut swipe = Swipe::new();
        swipe.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));
        let _ = swipe.on_event(pointer(Phase::Move, Point::new(50.0, 0.0), 500));

        let disposition = swipe.on_event(pointer(Phase::Up, Point::new(100.0, 0.0), 1000));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn a_fast_short_release_rejects_for_insufficient_distance() {
        let mut swipe = Swipe::new();
        swipe.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));

        let disposition = swipe.on_event(pointer(Phase::Up, Point::new(5.0, 0.0), 5));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn cancel_rejects() {
        let mut swipe = Swipe::new();
        swipe.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));

        let disposition = swipe.on_event(pointer(Phase::Cancel, Point::new(0.0, 0.0), 10));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn direction_resolves_to_the_dominant_axis() {
        let mut swipe = Swipe::new();
        swipe.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));

        let _ = swipe.on_event(pointer(Phase::Up, Point::new(0.0, 100.0), 20));
        assert_eq!(swipe.direction(), Some(Direction::Down));
    }

    #[test]
    fn upward_and_leftward_swipes_resolve_correctly() {
        let mut up = Swipe::new();
        up.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));
        let _ = up.on_event(pointer(Phase::Up, Point::new(0.0, -100.0), 20));
        assert_eq!(up.direction(), Some(Direction::Up));

        let mut left = Swipe::new();
        left.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));
        let _ = left.on_event(pointer(Phase::Up, Point::new(-100.0, 0.0), 20));
        assert_eq!(left.direction(), Some(Direction::Left));
    }

    #[test]
    fn a_second_concurrent_pointer_rejects() {
        let mut swipe = Swipe::new();
        swipe.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));
        swipe.add_pointer(Pointer {
            id: PointerId::from_raw(2),
            ..pointer(Phase::Down, Point::new(5.0, 5.0), 1)
        });

        let disposition = swipe.on_event(pointer(Phase::Up, Point::new(100.0, 0.0), 20));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn reset_allows_reuse_for_a_new_contact() {
        let mut swipe = Swipe::new();
        swipe.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));
        let _ = swipe.on_event(pointer(Phase::Up, Point::new(5.0, 0.0), 5));

        swipe.reset();
        swipe.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 100));

        let disposition = swipe.on_event(pointer(Phase::Up, Point::new(100.0, 0.0), 120));
        assert_eq!(disposition, Disposition::Accepted);
    }
}

//! Reserved-edge swipe recognition with system priority.
//!
//! A real touch platform reserves its screen edges for system-level
//! gestures (e.g. "swipe up from the bottom to go home") that must win
//! against ordinary content gestures starting at the same position --
//! an app's own edge-adjacent pan/scroll should never be able to
//! intercept a system back-swipe. [`EdgeSwipe`] is the recognizer half
//! of that policy; the "must win" half is the caller's responsibility,
//! by registering it into the [`crate::arena::Arena`] with
//! [`crate::arena::Priority::SYSTEM`] (see this module's own
//! [`EdgeSwipe`] documentation for the full contract).
use crate::recognizer::{Disposition, Recognizer, Verdict};
use crate::swipe::{DEFAULT_MIN_DISTANCE, DEFAULT_MIN_VELOCITY, Direction};
use crate::velocity::Estimator;
use taim_core::pointer::{Id as PointerId, Phase, Pointer};
use taim_core::{Point, Size};

/// Which edge of a surface an [`EdgeSwipe`] watches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    /// The left edge (x near 0).
    Left,
    /// The right edge (x near the surface's width).
    Right,
    /// The top edge (y near 0).
    Top,
    /// The bottom edge (y near the surface's height).
    Bottom,
}

impl Edge {
    /// Returns whether `position` is within `threshold` logical pixels
    /// of this [`Edge`] of a `size`-sized surface.
    fn contains(self, position: Point, size: Size, threshold: f32) -> bool {
        match self {
            Edge::Left => position.x <= threshold,
            Edge::Right => position.x >= size.width - threshold,
            Edge::Top => position.y <= threshold,
            Edge::Bottom => position.y >= size.height - threshold,
        }
    }

    /// Returns whether `direction` swipes inward, away from this
    /// [`Edge`] -- the only direction a reserved edge-swipe should ever
    /// recognize (a swipe running *along* the edge, or further off-
    /// screen, is not the "pull this edge's system surface in" gesture
    /// the edge is reserved for).
    fn accepts(self, direction: Direction) -> bool {
        matches!(
            (self, direction),
            (Edge::Left, Direction::Right)
                | (Edge::Right, Direction::Left)
                | (Edge::Top, Direction::Down)
                | (Edge::Bottom, Direction::Up)
        )
    }
}

/// How close to an [`Edge`] a pointer's initial down position must be to
/// count as starting an edge swipe, in logical pixels.
pub const DEFAULT_EDGE_THRESHOLD: f32 = 20.0;

/// Recognizes a swipe starting within [`EdgeSwipe::threshold`] of a
/// reserved [`Edge`] of a `size`-sized surface, moving inward.
///
/// Otherwise follows the same distance/velocity rules as
/// [`crate::swipe::Swipe`] (indeed, [`EdgeSwipe`] rejects immediately,
/// before any of that matters, if the pointer did not even start near
/// the reserved edge) -- the two differ only in *where* they may start
/// and *which* direction they accept, not in what "fast enough and far
/// enough" means.
///
/// # System priority
///
/// [`EdgeSwipe`] does not set its own arena priority -- priority is an
/// [`crate::arena::Arena::register`]-time parameter, not a property of
/// the recognizer itself. A caller wiring up reserved system edges is
/// expected to register this recognizer with
/// [`crate::arena::Priority::SYSTEM`], so it wins any same-pass tie
/// against an ordinary content gesture (e.g. a [`crate::pan`]) that also
/// accepted starting from the same edge-adjacent position -- registering
/// it at [`crate::arena::Priority::NORMAL`] would defeat the entire
/// point of a *reserved* edge.
pub struct EdgeSwipe {
    edge: Edge,
    size: Size,
    threshold: f32,
    min_distance: f32,
    min_velocity: f32,
    tracked: Option<PointerId>,
    origin: Option<Point>,
    estimator: Estimator,
    rejected: bool,
}

impl EdgeSwipe {
    /// Creates an [`EdgeSwipe`] watching `edge` of a `size`-sized
    /// surface, using the default threshold, minimum distance, and
    /// minimum velocity.
    pub fn new(edge: Edge, size: Size) -> Self {
        Self {
            edge,
            size,
            threshold: DEFAULT_EDGE_THRESHOLD,
            min_distance: DEFAULT_MIN_DISTANCE,
            min_velocity: DEFAULT_MIN_VELOCITY,
            tracked: None,
            origin: None,
            estimator: Estimator::new(),
            rejected: false,
        }
    }

    /// Sets how close to the edge a pointer must start, in logical
    /// pixels.
    pub fn threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold;
        self
    }

    /// Sets the minimum total distance, mirroring
    /// [`crate::swipe::Swipe::min_distance`].
    pub fn min_distance(mut self, distance: f32) -> Self {
        self.min_distance = distance;
        self
    }

    /// Sets the minimum release velocity, mirroring
    /// [`crate::swipe::Swipe::min_velocity`].
    pub fn min_velocity(mut self, velocity: f32) -> Self {
        self.min_velocity = velocity;
        self
    }
}

impl Recognizer for EdgeSwipe {
    fn name(&self) -> &'static str {
        "edge_swipe"
    }

    fn add_pointer(&mut self, pointer: Pointer) {
        if self.tracked.is_some() {
            self.rejected = true;
            return;
        }

        if !self
            .edge
            .contains(pointer.position, self.size, self.threshold)
        {
            self.rejected = true;
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

        let direction = Direction::from_vector(displacement);

        if self.edge.accepts(direction) {
            Disposition::Accepted
        } else {
            Disposition::Rejected
        }
    }

    fn on_verdict(&mut self, _verdict: Verdict) {}

    fn reset(&mut self) {
        self.tracked = None;
        self.origin = None;
        self.estimator.reset();
        self.rejected = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taim_core::pointer::{DeviceKind, Metadata};
    use taim_core::time::{Duration, Instant};

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

    const SURFACE: Size = Size::new(400.0, 800.0);

    #[test]
    fn a_swipe_starting_at_the_left_edge_moving_right_accepts() {
        let mut edge_swipe = EdgeSwipe::new(Edge::Left, SURFACE);
        edge_swipe.add_pointer(pointer(Phase::Down, Point::new(5.0, 400.0), 0));

        let disposition = edge_swipe.on_event(pointer(Phase::Up, Point::new(100.0, 400.0), 20));

        assert_eq!(disposition, Disposition::Accepted);
    }

    #[test]
    fn a_swipe_not_starting_near_the_edge_rejects() {
        let mut edge_swipe = EdgeSwipe::new(Edge::Left, SURFACE);
        edge_swipe.add_pointer(pointer(Phase::Down, Point::new(200.0, 400.0), 0));

        let disposition = edge_swipe.on_event(pointer(Phase::Up, Point::new(300.0, 400.0), 20));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn a_swipe_at_the_edge_moving_the_wrong_direction_rejects() {
        let mut edge_swipe = EdgeSwipe::new(Edge::Left, SURFACE);
        // Starting at the left edge but swiping further left (off
        // screen) rather than inward.
        edge_swipe.add_pointer(pointer(Phase::Down, Point::new(10.0, 400.0), 0));

        let disposition = edge_swipe.on_event(pointer(Phase::Up, Point::new(10.0, 500.0), 20));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn every_edge_only_accepts_its_own_inward_direction() {
        let cases = [
            (Edge::Left, Point::new(5.0, 400.0), Point::new(100.0, 400.0)),
            (
                Edge::Right,
                Point::new(395.0, 400.0),
                Point::new(300.0, 400.0),
            ),
            (Edge::Top, Point::new(200.0, 5.0), Point::new(200.0, 100.0)),
            (
                Edge::Bottom,
                Point::new(200.0, 795.0),
                Point::new(200.0, 700.0),
            ),
        ];

        for (edge, origin, release) in cases {
            let mut edge_swipe = EdgeSwipe::new(edge, SURFACE);
            edge_swipe.add_pointer(pointer(Phase::Down, origin, 0));

            let disposition = edge_swipe.on_event(pointer(Phase::Up, release, 20));

            assert_eq!(disposition, Disposition::Accepted, "edge {edge:?} failed");
        }
    }

    #[test]
    fn cancel_rejects() {
        let mut edge_swipe = EdgeSwipe::new(Edge::Left, SURFACE);
        edge_swipe.add_pointer(pointer(Phase::Down, Point::new(5.0, 400.0), 0));

        let disposition = edge_swipe.on_event(pointer(Phase::Cancel, Point::new(5.0, 400.0), 10));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn a_second_concurrent_pointer_rejects() {
        let mut edge_swipe = EdgeSwipe::new(Edge::Left, SURFACE);
        edge_swipe.add_pointer(pointer(Phase::Down, Point::new(5.0, 400.0), 0));
        edge_swipe.add_pointer(Pointer {
            id: PointerId::from_raw(2),
            ..pointer(Phase::Down, Point::new(5.0, 405.0), 1)
        });

        let disposition = edge_swipe.on_event(pointer(Phase::Up, Point::new(100.0, 400.0), 20));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn reset_allows_reuse_for_a_new_contact() {
        let mut edge_swipe = EdgeSwipe::new(Edge::Left, SURFACE);
        edge_swipe.add_pointer(pointer(Phase::Down, Point::new(200.0, 400.0), 0));
        let _ = edge_swipe.on_event(pointer(Phase::Up, Point::new(300.0, 400.0), 20));

        edge_swipe.reset();
        edge_swipe.add_pointer(pointer(Phase::Down, Point::new(5.0, 400.0), 100));

        let disposition = edge_swipe.on_event(pointer(Phase::Up, Point::new(100.0, 400.0), 120));
        assert_eq!(disposition, Disposition::Accepted);
    }
}

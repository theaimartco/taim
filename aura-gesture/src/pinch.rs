//! Pinch (scale) recognition with focal-point tracking.
use crate::recognizer::{Disposition, Recognizer, Verdict};
use aura_core::Point;
use aura_core::pointer::{Id as PointerId, Phase, Pointer};

/// The minimum relative change in inter-pointer distance, as a fraction
/// of the initial distance, before a [`Pinch`] accepts.
///
/// E.g. `0.05` requires the two pointers to have moved at least 5%
/// closer together or further apart than where they started, filtering
/// out the same unavoidable jitter a single-pointer recognizer's slop
/// tolerance exists for.
pub const DEFAULT_MIN_SCALE_DELTA: f32 = 0.05;

/// Recognizes a pinch: exactly two pointers whose distance from each
/// other changes by at least [`Pinch::min_scale_delta`] (relative to
/// where they started), reporting a running [`Pinch::scale`] and
/// [`Pinch::focal_point`] once accepted.
///
/// Requires *exactly* two concurrent pointers -- fewer can't pinch at
/// all, and a third joining mid-gesture means the interaction is
/// something else (most platforms treat a third finger as ending the
/// pinch), so [`Pinch`] rejects rather than trying to guess which two of
/// three pointers were the "real" pinch.
pub struct Pinch {
    min_scale_delta: f32,
    tracked: [Option<(PointerId, Point)>; 2],
    initial_distance: Option<f32>,
    scale: f32,
    focal_point: Point,
    accepted: bool,
    rejected: bool,
}

impl Default for Pinch {
    fn default() -> Self {
        Self::new()
    }
}

impl Pinch {
    /// Creates a [`Pinch`] recognizer using the default minimum scale
    /// delta.
    pub fn new() -> Self {
        Self {
            min_scale_delta: DEFAULT_MIN_SCALE_DELTA,
            tracked: [None, None],
            initial_distance: None,
            scale: 1.0,
            focal_point: Point::ORIGIN,
            accepted: false,
            rejected: false,
        }
    }

    /// Sets the minimum relative scale change required to accept.
    pub fn min_scale_delta(mut self, delta: f32) -> Self {
        self.min_scale_delta = delta;
        self
    }

    /// Returns the current scale factor relative to the initial
    /// distance between the two pointers (`1.0` at the start, `>1.0`
    /// while spreading apart, `<1.0` while pinching together).
    ///
    /// Only meaningful once [`Pinch`] has accepted; reports `1.0`
    /// beforehand.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Returns the current midpoint between the two tracked pointers,
    /// updated on every event -- the point a scale transform should be
    /// applied around, so content under the fingers stays under them as
    /// the user pinches.
    pub fn focal_point(&self) -> Point {
        self.focal_point
    }

    fn slots_filled(&self) -> usize {
        self.tracked.iter().filter(|slot| slot.is_some()).count()
    }

    fn update_derived(&mut self) {
        let (Some((_, a)), Some((_, b))) = (self.tracked[0], self.tracked[1]) else {
            return;
        };

        let distance = a.distance(b);
        self.focal_point = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);

        if let Some(initial) = self.initial_distance
            && initial > 0.0
        {
            self.scale = distance / initial;
        }
    }
}

impl Recognizer for Pinch {
    fn name(&self) -> &'static str {
        "pinch"
    }

    fn add_pointer(&mut self, pointer: Pointer) {
        if self.slots_filled() >= 2 {
            self.rejected = true;
            return;
        }

        let slot = if self.tracked[0].is_none() { 0 } else { 1 };
        self.tracked[slot] = Some((pointer.id, pointer.position));

        if self.slots_filled() == 2 {
            let (Some((_, a)), Some((_, b))) = (self.tracked[0], self.tracked[1]) else {
                unreachable!("both slots were just confirmed filled");
            };

            self.initial_distance = Some(a.distance(b));
            self.update_derived();
        }
    }

    fn remove_pointer(&mut self, id: PointerId) {
        for slot in &mut self.tracked {
            if slot.map(|(tracked_id, _)| tracked_id) == Some(id) {
                *slot = None;
            }
        }
    }

    fn on_event(&mut self, pointer: Pointer) -> Disposition {
        if self.rejected {
            return Disposition::Rejected;
        }

        let is_tracked = self
            .tracked
            .iter()
            .any(|slot| slot.map(|(id, _)| id) == Some(pointer.id));

        if !is_tracked {
            return Disposition::Rejected;
        }

        if pointer.phase == Phase::Cancel {
            return Disposition::Rejected;
        }

        if pointer.phase == Phase::Up && self.slots_filled() < 2 {
            // One of the two fingers already lifted (or is lifting now)
            // before we ever had both down together long enough to
            // decide -- there is no pinch without two simultaneous
            // contacts.
            return Disposition::Rejected;
        }

        for slot in &mut self.tracked {
            if slot.map(|(id, _)| id) == Some(pointer.id) {
                *slot = Some((pointer.id, pointer.position));
            }
        }

        if self.slots_filled() < 2 {
            return Disposition::Pending;
        }

        self.update_derived();

        if self.accepted {
            return Disposition::Accepted;
        }

        let Some(initial) = self.initial_distance else {
            return Disposition::Pending;
        };

        if initial <= 0.0 {
            return Disposition::Pending;
        }

        if (self.scale - 1.0).abs() >= self.min_scale_delta {
            self.accepted = true;
            Disposition::Accepted
        } else {
            Disposition::Pending
        }
    }

    fn on_verdict(&mut self, _verdict: Verdict) {}

    fn reset(&mut self) {
        self.tracked = [None, None];
        self.initial_distance = None;
        self.scale = 1.0;
        self.focal_point = Point::ORIGIN;
        self.accepted = false;
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

    fn pointer(id: u64, phase: Phase, position: Point, millis: u64) -> Pointer {
        Pointer {
            id: PointerId::from_raw(id),
            device: DeviceKind::Touch,
            phase,
            position,
            timestamp: t(millis),
            metadata: Metadata::NONE,
            captured: false,
        }
    }

    #[test]
    fn spreading_two_fingers_apart_accepts_with_scale_above_one() {
        let mut pinch = Pinch::new();
        pinch.add_pointer(pointer(1, Phase::Down, Point::new(90.0, 100.0), 0));
        pinch.add_pointer(pointer(2, Phase::Down, Point::new(110.0, 100.0), 0));

        let disposition = pinch.on_event(pointer(1, Phase::Move, Point::new(50.0, 100.0), 10));
        let _ = pinch.on_event(pointer(2, Phase::Move, Point::new(150.0, 100.0), 10));

        assert!(pinch.scale() > 1.0);
        // Depending on exactly which move crosses the threshold, one of
        // the two calls may already report Accepted.
        let _ = disposition;
        assert_eq!(
            pinch.on_event(pointer(1, Phase::Move, Point::new(50.0, 100.0), 20)),
            Disposition::Accepted
        );
    }

    #[test]
    fn pinching_two_fingers_together_accepts_with_scale_below_one() {
        let mut pinch = Pinch::new();
        pinch.add_pointer(pointer(1, Phase::Down, Point::new(0.0, 100.0), 0));
        pinch.add_pointer(pointer(2, Phase::Down, Point::new(200.0, 100.0), 0));

        let _ = pinch.on_event(pointer(1, Phase::Move, Point::new(90.0, 100.0), 10));
        let disposition = pinch.on_event(pointer(2, Phase::Move, Point::new(110.0, 100.0), 10));

        assert_eq!(disposition, Disposition::Accepted);
        assert!(pinch.scale() < 1.0);
    }

    #[test]
    fn small_movement_below_the_threshold_stays_pending() {
        let mut pinch = Pinch::new();
        pinch.add_pointer(pointer(1, Phase::Down, Point::new(0.0, 100.0), 0));
        pinch.add_pointer(pointer(2, Phase::Down, Point::new(200.0, 100.0), 0));

        let disposition = pinch.on_event(pointer(1, Phase::Move, Point::new(1.0, 100.0), 10));

        assert_eq!(disposition, Disposition::Pending);
    }

    #[test]
    fn a_third_pointer_rejects() {
        let mut pinch = Pinch::new();
        pinch.add_pointer(pointer(1, Phase::Down, Point::new(0.0, 100.0), 0));
        pinch.add_pointer(pointer(2, Phase::Down, Point::new(200.0, 100.0), 0));
        pinch.add_pointer(pointer(3, Phase::Down, Point::new(100.0, 0.0), 1));

        let disposition = pinch.on_event(pointer(1, Phase::Move, Point::new(90.0, 100.0), 10));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn only_one_pointer_down_stays_pending() {
        let mut pinch = Pinch::new();
        pinch.add_pointer(pointer(1, Phase::Down, Point::new(0.0, 100.0), 0));

        let disposition = pinch.on_event(pointer(1, Phase::Move, Point::new(10.0, 100.0), 10));

        assert_eq!(disposition, Disposition::Pending);
    }

    #[test]
    fn one_finger_lifting_before_two_are_both_down_rejects() {
        let mut pinch = Pinch::new();
        pinch.add_pointer(pointer(1, Phase::Down, Point::new(0.0, 100.0), 0));

        let disposition = pinch.on_event(pointer(1, Phase::Up, Point::new(0.0, 100.0), 10));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn cancel_rejects() {
        let mut pinch = Pinch::new();
        pinch.add_pointer(pointer(1, Phase::Down, Point::new(0.0, 100.0), 0));
        pinch.add_pointer(pointer(2, Phase::Down, Point::new(200.0, 100.0), 0));

        let disposition = pinch.on_event(pointer(1, Phase::Cancel, Point::new(0.0, 100.0), 10));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn focal_point_tracks_the_midpoint_of_both_pointers() {
        let mut pinch = Pinch::new();
        pinch.add_pointer(pointer(1, Phase::Down, Point::new(0.0, 0.0), 0));
        pinch.add_pointer(pointer(2, Phase::Down, Point::new(100.0, 0.0), 0));

        assert_eq!(pinch.focal_point(), Point::new(50.0, 0.0));

        let _ = pinch.on_event(pointer(1, Phase::Move, Point::new(0.0, 100.0), 10));

        assert_eq!(pinch.focal_point(), Point::new(50.0, 50.0));
    }

    #[test]
    fn reset_allows_reuse_for_a_new_pair_of_contacts() {
        let mut pinch = Pinch::new();
        pinch.add_pointer(pointer(1, Phase::Down, Point::new(0.0, 100.0), 0));
        pinch.add_pointer(pointer(2, Phase::Down, Point::new(200.0, 100.0), 0));
        let _ = pinch.on_event(pointer(1, Phase::Move, Point::new(90.0, 100.0), 10));

        pinch.reset();
        pinch.add_pointer(pointer(3, Phase::Down, Point::new(0.0, 0.0), 100));
        pinch.add_pointer(pointer(4, Phase::Down, Point::new(200.0, 0.0), 100));

        let disposition = pinch.on_event(pointer(3, Phase::Move, Point::new(90.0, 0.0), 110));
        assert_eq!(disposition, Disposition::Accepted);
    }
}

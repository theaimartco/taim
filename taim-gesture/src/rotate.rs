//! Stable multi-pointer rotation-angle recognition.
use crate::recognizer::{Disposition, Recognizer, Verdict};
use taim_core::pointer::{Id as PointerId, Phase, Pointer};
use taim_core::{Point, Radians};

/// The minimum absolute angle change, in radians, before a [`Rotate`]
/// accepts.
///
/// `0.05` radians (~2.9 degrees) filters out the same unavoidable jitter
/// every other slop/threshold in this crate exists to tolerate.
pub const DEFAULT_MIN_ANGLE_DELTA: f32 = 0.05;

/// Normalizes an angle difference to `(-π, π]`, so a rotation that
/// crosses the raw `atan2` wraparound boundary (e.g. from just under +π
/// to just over -π) reports a small delta in the direction the fingers
/// actually moved, rather than jumping by nearly a full turn.
///
/// This is the "stable" in this module's own file-plan description:
/// without this normalization, a rotation gesture crossing the
/// discontinuity would report a wildly incorrect angle for exactly one
/// frame.
fn normalize(mut delta: f32) -> f32 {
    use std::f32::consts::PI;

    while delta > PI {
        delta -= 2.0 * PI;
    }

    while delta <= -PI {
        delta += 2.0 * PI;
    }

    delta
}

/// Recognizes a rotation: exactly two pointers whose angle relative to
/// each other changes by at least [`Rotate::min_angle_delta`] (relative
/// to where they started), reporting a running [`Rotate::angle`] once
/// accepted.
///
/// Like [`crate::pinch::Pinch`], [`Rotate`] requires *exactly* two
/// concurrent pointers and rejects if a third joins.
pub struct Rotate {
    min_angle_delta: f32,
    tracked: [Option<(PointerId, Point)>; 2],
    initial_angle: Option<f32>,
    angle: Radians,
    accepted: bool,
    rejected: bool,
}

impl Default for Rotate {
    fn default() -> Self {
        Self::new()
    }
}

impl Rotate {
    /// Creates a [`Rotate`] recognizer using the default minimum angle
    /// delta.
    pub fn new() -> Self {
        Self {
            min_angle_delta: DEFAULT_MIN_ANGLE_DELTA,
            tracked: [None, None],
            initial_angle: None,
            angle: Radians(0.0),
            accepted: false,
            rejected: false,
        }
    }

    /// Sets the minimum angle change required to accept, in radians.
    pub fn min_angle_delta(mut self, delta: f32) -> Self {
        self.min_angle_delta = delta;
        self
    }

    /// Returns the current rotation angle relative to the initial angle
    /// between the two pointers (positive is counter-clockwise, matching
    /// [`taim_core::Radians`]'s own convention).
    ///
    /// Only meaningful once [`Rotate`] has accepted; reports `0.0`
    /// beforehand.
    pub fn angle(&self) -> Radians {
        self.angle
    }

    fn slots_filled(&self) -> usize {
        self.tracked.iter().filter(|slot| slot.is_some()).count()
    }

    fn current_angle(&self) -> Option<f32> {
        let (Some((_, a)), Some((_, b))) = (self.tracked[0], self.tracked[1]) else {
            return None;
        };

        Some((b.y - a.y).atan2(b.x - a.x))
    }

    fn update_derived(&mut self) {
        let Some(current) = self.current_angle() else {
            return;
        };

        if let Some(initial) = self.initial_angle {
            self.angle = Radians(normalize(current - initial));
        }
    }
}

impl Recognizer for Rotate {
    fn name(&self) -> &'static str {
        "rotate"
    }

    fn add_pointer(&mut self, pointer: Pointer) {
        if self.slots_filled() >= 2 {
            self.rejected = true;
            return;
        }

        let slot = if self.tracked[0].is_none() { 0 } else { 1 };
        self.tracked[slot] = Some((pointer.id, pointer.position));

        if self.slots_filled() == 2 {
            self.initial_angle = self.current_angle();
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

        if self.initial_angle.is_none() {
            return Disposition::Pending;
        }

        if self.angle.0.abs() >= self.min_angle_delta {
            self.accepted = true;
            Disposition::Accepted
        } else {
            Disposition::Pending
        }
    }

    fn on_verdict(&mut self, _verdict: Verdict) {}

    fn reset(&mut self) {
        self.tracked = [None, None];
        self.initial_angle = None;
        self.angle = Radians(0.0);
        self.accepted = false;
        self.rejected = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taim_core::pointer::{DeviceKind, Metadata};
    use taim_core::time::{Duration, Instant};
    use std::f32::consts::PI;

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
    fn normalize_keeps_small_deltas_unchanged() {
        assert!((normalize(0.1) - 0.1).abs() < 1e-6);
        assert!((normalize(-0.1) - (-0.1)).abs() < 1e-6);
    }

    #[test]
    fn normalize_wraps_deltas_beyond_pi() {
        let wrapped = normalize(PI + 0.1);
        assert!((wrapped - (0.1 - PI)).abs() < 1e-4);
    }

    #[test]
    fn a_quarter_turn_is_recognized_and_reported() {
        let mut rotate = Rotate::new();
        // Horizontal line: (0,0) -- (100,0).
        rotate.add_pointer(pointer(1, Phase::Down, Point::new(0.0, 0.0), 0));
        rotate.add_pointer(pointer(2, Phase::Down, Point::new(100.0, 0.0), 0));

        // Rotate to vertical: (0,0) -- (0,100), a +90 degree turn.
        let disposition = rotate.on_event(pointer(2, Phase::Move, Point::new(0.0, 100.0), 10));

        assert_eq!(disposition, Disposition::Accepted);
        assert!((rotate.angle().0 - std::f32::consts::FRAC_PI_2).abs() < 0.01);
    }

    #[test]
    fn small_movement_below_the_threshold_stays_pending() {
        let mut rotate = Rotate::new();
        rotate.add_pointer(pointer(1, Phase::Down, Point::new(0.0, 0.0), 0));
        rotate.add_pointer(pointer(2, Phase::Down, Point::new(100.0, 0.0), 0));

        let disposition = rotate.on_event(pointer(2, Phase::Move, Point::new(100.0, 1.0), 10));

        assert_eq!(disposition, Disposition::Pending);
    }

    #[test]
    fn a_third_pointer_rejects() {
        let mut rotate = Rotate::new();
        rotate.add_pointer(pointer(1, Phase::Down, Point::new(0.0, 0.0), 0));
        rotate.add_pointer(pointer(2, Phase::Down, Point::new(100.0, 0.0), 0));
        rotate.add_pointer(pointer(3, Phase::Down, Point::new(0.0, 100.0), 1));

        let disposition = rotate.on_event(pointer(2, Phase::Move, Point::new(0.0, 100.0), 10));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn cancel_rejects() {
        let mut rotate = Rotate::new();
        rotate.add_pointer(pointer(1, Phase::Down, Point::new(0.0, 0.0), 0));
        rotate.add_pointer(pointer(2, Phase::Down, Point::new(100.0, 0.0), 0));

        let disposition = rotate.on_event(pointer(1, Phase::Cancel, Point::new(0.0, 0.0), 10));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn one_finger_lifting_before_two_are_both_down_rejects() {
        let mut rotate = Rotate::new();
        rotate.add_pointer(pointer(1, Phase::Down, Point::new(0.0, 0.0), 0));

        let disposition = rotate.on_event(pointer(1, Phase::Up, Point::new(0.0, 0.0), 10));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn reset_allows_reuse_for_a_new_pair_of_contacts() {
        let mut rotate = Rotate::new();
        rotate.add_pointer(pointer(1, Phase::Down, Point::new(0.0, 0.0), 0));
        rotate.add_pointer(pointer(2, Phase::Down, Point::new(100.0, 0.0), 0));
        let _ = rotate.on_event(pointer(2, Phase::Move, Point::new(0.0, 100.0), 10));

        rotate.reset();
        rotate.add_pointer(pointer(3, Phase::Down, Point::new(0.0, 0.0), 100));
        rotate.add_pointer(pointer(4, Phase::Down, Point::new(100.0, 0.0), 100));

        let disposition = rotate.on_event(pointer(4, Phase::Move, Point::new(0.0, 100.0), 110));
        assert_eq!(disposition, Disposition::Accepted);
    }
}

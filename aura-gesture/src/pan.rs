//! Pan recognition with axis lock, slop, and capture.
use crate::recognizer::{Disposition, Recognizer, Verdict};
use aura_core::pointer::{Id as PointerId, Phase, Pointer};
use aura_core::{Point, Vector};

/// How far a pointer must move from its initial down position before a
/// [`Pan`] accepts, in logical pixels.
///
/// Below this distance, movement is presumed to be the same
/// unavoidable jitter [`crate::tap::DEFAULT_SLOP`] already tolerates for
/// a tap; a [`Pan`] should not steal the arena from a tap/long-press
/// over sub-pixel-scale noise.
pub const DEFAULT_SLOP: f32 = 8.0;

/// Which axis, if any, a [`Pan`] has locked its reported movement to.
///
/// Locking is a purely reporting-side decision (see [`Pan::translation`]
/// zeroing the cross-axis component once locked) -- it never affects
/// whether the pan itself accepts or rejects, only how its
/// [`Pan::translation`] looks to whatever consumes it, matching how a
/// real single-direction scrollable wants a diagonal drag reported as
/// pure vertical or pure horizontal motion once the intent is clear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisLock {
    /// No axis lock requested; [`Pan::translation`] reports the full,
    /// unmodified displacement.
    Free,
    /// Once movement exceeds [`Pan::slop`] predominantly along the
    /// horizontal axis, lock to it and zero out vertical movement in
    /// [`Pan::translation`] from then on.
    Horizontal,
    /// The vertical counterpart of [`AxisLock::Horizontal`].
    Vertical,
}

/// Recognizes a pan: one pointer moving beyond [`Pan::slop`] from its
/// initial position, optionally constrained to a single reporting axis
/// via [`AxisLock`].
///
/// A [`Pan`] captures its pointer the moment it accepts (see
/// [`crate::arena`]'s module documentation: an accepted entrant's
/// pointer is handed to it for the remainder of the contact's
/// lifetime) -- from that point on, [`Pan::translation`] is the
/// authoritative source of the drag delta a consumer (a scrollable, a
/// draggable) should apply.
pub struct Pan {
    slop: f32,
    axis_lock: AxisLock,
    locked_axis: Option<AxisLock>,
    tracked: Option<PointerId>,
    origin: Option<Point>,
    current: Option<Point>,
    accepted: bool,
    rejected: bool,
}

impl Default for Pan {
    fn default() -> Self {
        Self::new()
    }
}

impl Pan {
    /// Creates a [`Pan`] recognizer with no axis lock and the default
    /// slop.
    pub fn new() -> Self {
        Self {
            slop: DEFAULT_SLOP,
            axis_lock: AxisLock::Free,
            locked_axis: None,
            tracked: None,
            origin: None,
            current: None,
            accepted: false,
            rejected: false,
        }
    }

    /// Sets the movement tolerance, in logical pixels.
    pub fn slop(mut self, slop: f32) -> Self {
        self.slop = slop;
        self
    }

    /// Requests axis locking once the pan accepts; see [`AxisLock`].
    pub fn axis_lock(mut self, axis_lock: AxisLock) -> Self {
        self.axis_lock = axis_lock;
        self
    }

    /// Returns the current translation from the initial down position,
    /// with the cross axis zeroed out if [`AxisLock`] has locked to one.
    ///
    /// Returns [`Vector::ZERO`] before any pointer has been registered.
    pub fn translation(&self) -> Vector {
        let (Some(origin), Some(current)) = (self.origin, self.current) else {
            return Vector::ZERO;
        };

        let raw = current - origin;

        match self.locked_axis {
            Some(AxisLock::Horizontal) => Vector::new(raw.x, 0.0),
            Some(AxisLock::Vertical) => Vector::new(0.0, raw.y),
            Some(AxisLock::Free) | None => raw,
        }
    }
}

impl Recognizer for Pan {
    fn name(&self) -> &'static str {
        "pan"
    }

    fn add_pointer(&mut self, pointer: Pointer) {
        if self.tracked.is_some() {
            self.rejected = true;
            return;
        }

        self.tracked = Some(pointer.id);
        self.origin = Some(pointer.position);
        self.current = Some(pointer.position);
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

        if matches!(pointer.phase, Phase::Cancel) {
            return Disposition::Rejected;
        }

        self.current = Some(pointer.position);

        if self.accepted {
            // Already won and capturing; every further event (short of
            // a cancel, handled above) keeps the pan accepted -- there
            // is no rejecting out of an already-accepted pan mid-drag.
            return Disposition::Accepted;
        }

        if matches!(pointer.phase, Phase::Up) {
            // Released before ever moving past the slop: this was a
            // tap-like interaction, not a pan.
            return Disposition::Rejected;
        }

        let Some(origin) = self.origin else {
            return Disposition::Rejected;
        };

        let displacement = pointer.position - origin;
        let distance = pointer.position.distance(origin);

        if distance < self.slop {
            return Disposition::Pending;
        }

        if self.locked_axis.is_none() {
            self.locked_axis = Some(match self.axis_lock {
                AxisLock::Free => AxisLock::Free,
                AxisLock::Horizontal | AxisLock::Vertical => {
                    if displacement.x.abs() >= displacement.y.abs() {
                        AxisLock::Horizontal
                    } else {
                        AxisLock::Vertical
                    }
                }
            });
        }

        self.accepted = true;
        Disposition::Accepted
    }

    fn on_verdict(&mut self, _verdict: Verdict) {}

    fn reset(&mut self) {
        self.tracked = None;
        self.origin = None;
        self.current = None;
        self.locked_axis = None;
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
    fn movement_within_slop_is_pending() {
        let mut pan = Pan::new();
        pan.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));

        let disposition = pan.on_event(pointer(Phase::Move, Point::new(2.0, 0.0), 10));

        assert_eq!(disposition, Disposition::Pending);
    }

    #[test]
    fn movement_beyond_slop_accepts() {
        let mut pan = Pan::new();
        pan.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));

        let disposition = pan.on_event(pointer(Phase::Move, Point::new(50.0, 0.0), 10));

        assert_eq!(disposition, Disposition::Accepted);
    }

    #[test]
    fn release_before_moving_past_slop_rejects() {
        let mut pan = Pan::new();
        pan.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));

        let disposition = pan.on_event(pointer(Phase::Up, Point::new(1.0, 0.0), 10));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn cancel_rejects() {
        let mut pan = Pan::new();
        pan.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));

        let disposition = pan.on_event(pointer(Phase::Cancel, Point::new(0.0, 0.0), 10));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn translation_reports_the_full_displacement_when_free() {
        let mut pan = Pan::new();
        pan.add_pointer(pointer(Phase::Down, Point::new(10.0, 10.0), 0));
        let _ = pan.on_event(pointer(Phase::Move, Point::new(60.0, 40.0), 10));

        assert_eq!(pan.translation(), Vector::new(50.0, 30.0));
    }

    #[test]
    fn horizontal_axis_lock_zeroes_vertical_movement() {
        let mut pan = Pan::new().axis_lock(AxisLock::Horizontal);
        pan.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));

        // Predominantly horizontal movement.
        let disposition = pan.on_event(pointer(Phase::Move, Point::new(50.0, 5.0), 10));
        assert_eq!(disposition, Disposition::Accepted);

        assert_eq!(pan.translation(), Vector::new(50.0, 0.0));
    }

    #[test]
    fn axis_lock_still_accepts_when_movement_is_predominantly_cross_axis() {
        let mut pan = Pan::new().axis_lock(AxisLock::Horizontal);
        pan.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));

        // Predominantly vertical movement still accepts -- the
        // recognizer never rejects based on direction, it only decides
        // which axis to report as locked once the slop is crossed.
        let disposition = pan.on_event(pointer(Phase::Move, Point::new(5.0, 50.0), 10));
        assert_eq!(disposition, Disposition::Accepted);

        assert_eq!(pan.translation(), Vector::new(0.0, 50.0));
    }

    #[test]
    fn axis_lock_is_decided_once_and_stays_fixed() {
        let mut pan = Pan::new().axis_lock(AxisLock::Horizontal);
        pan.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));

        let _ = pan.on_event(pointer(Phase::Move, Point::new(50.0, 5.0), 10));
        assert_eq!(pan.translation(), Vector::new(50.0, 0.0));

        // Later movement swings mostly vertical, but the axis lock was
        // already decided on first crossing the slop and must not
        // change.
        let _ = pan.on_event(pointer(Phase::Move, Point::new(55.0, 80.0), 20));
        assert_eq!(pan.translation(), Vector::new(55.0, 0.0));
    }

    #[test]
    fn a_second_concurrent_pointer_rejects() {
        let mut pan = Pan::new();
        pan.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));
        pan.add_pointer(Pointer {
            id: PointerId::from_raw(2),
            ..pointer(Phase::Down, Point::new(5.0, 5.0), 1)
        });

        let disposition = pan.on_event(pointer(Phase::Move, Point::new(50.0, 0.0), 10));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn once_accepted_stays_accepted_across_further_moves() {
        let mut pan = Pan::new();
        pan.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));
        let _ = pan.on_event(pointer(Phase::Move, Point::new(50.0, 0.0), 10));

        let disposition = pan.on_event(pointer(Phase::Move, Point::new(60.0, 0.0), 20));

        assert_eq!(disposition, Disposition::Accepted);
    }

    #[test]
    fn reset_allows_reuse_for_a_new_contact() {
        let mut pan = Pan::new();
        pan.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));
        let _ = pan.on_event(pointer(Phase::Move, Point::new(50.0, 0.0), 10));

        pan.reset();
        pan.add_pointer(pointer(Phase::Down, Point::new(100.0, 100.0), 100));

        assert_eq!(pan.translation(), Vector::ZERO);
    }
}

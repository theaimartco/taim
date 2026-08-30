//! Tap and multi-tap recognition.
use crate::recognizer::{Disposition, Recognizer, Verdict};
use taim_core::Point;
use taim_core::pointer::{Id as PointerId, Phase, Pointer};
use taim_core::time::{Duration, Instant};

/// How far a pointer may move from its initial down position and still
/// count as a tap, in logical pixels.
///
/// Matches common platform tap-slop conventions (a real finger/mouse is
/// never perfectly still); movement beyond this rejects the tap in
/// favor of whatever gesture (a [`crate::pan`], most commonly) actually
/// matches sustained movement.
pub const DEFAULT_SLOP: f32 = 8.0;

/// How long a pointer may remain down and still count as a tap, rather
/// than a [`crate::long_press`].
pub const DEFAULT_TIMEOUT: Duration = Duration::from_millis(500);

/// How long after one tap's release a second tap's press must begin to
/// count as part of the same multi-tap sequence.
pub const DEFAULT_MULTI_TAP_INTERVAL: Duration = Duration::from_millis(300);

/// How far a subsequent tap's down position may drift from the first
/// tap's, and still count as part of the same multi-tap sequence.
pub const DEFAULT_MULTI_TAP_SLOP: f32 = 24.0;

/// Recognizes a single tap: one pointer, pressed and released within
/// [`Tap::timeout`], having never moved more than [`Tap::slop`] from its
/// initial position.
///
/// [`Tap`] only ever tracks one pointer at a time; a second concurrent
/// pointer immediately rejects (a tap is, by definition, a single-finger
/// gesture -- a second finger joining mid-gesture means the user is
/// doing something else, most likely a [`crate::pinch`] or
/// [`crate::rotate`]).
pub struct Tap {
    slop: f32,
    timeout: Duration,
    tracked: Option<PointerId>,
    origin: Option<Point>,
    started_at: Option<Instant>,
    rejected: bool,
}

impl Default for Tap {
    fn default() -> Self {
        Self::new()
    }
}

impl Tap {
    /// Creates a [`Tap`] recognizer using the default slop and timeout.
    pub fn new() -> Self {
        Self {
            slop: DEFAULT_SLOP,
            timeout: DEFAULT_TIMEOUT,
            tracked: None,
            origin: None,
            started_at: None,
            rejected: false,
        }
    }

    /// Sets the movement tolerance, in logical pixels.
    pub fn slop(mut self, slop: f32) -> Self {
        self.slop = slop;
        self
    }

    /// Sets the maximum hold duration.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl Recognizer for Tap {
    fn name(&self) -> &'static str {
        "tap"
    }

    fn add_pointer(&mut self, pointer: Pointer) {
        if self.tracked.is_some() {
            // A second concurrent pointer immediately disqualifies this
            // as a tap; `rejected` is checked on the next `on_event`
            // call, since `add_pointer` itself has no `Disposition` to
            // return.
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

        let Some(origin) = self.origin else {
            return Disposition::Rejected;
        };
        let Some(started_at) = self.started_at else {
            return Disposition::Rejected;
        };

        let elapsed = pointer.timestamp.saturating_duration_since(started_at);

        if elapsed > self.timeout {
            return Disposition::Rejected;
        }

        let distance = pointer.position.distance(origin);

        if distance > self.slop {
            return Disposition::Rejected;
        }

        match pointer.phase {
            Phase::Up => Disposition::Accepted,
            Phase::Cancel => Disposition::Rejected,
            Phase::Down | Phase::Move => Disposition::Pending,
        }
    }

    fn on_verdict(&mut self, _verdict: Verdict) {}

    fn reset(&mut self) {
        self.tracked = None;
        self.origin = None;
        self.started_at = None;
        self.rejected = false;
    }
}

/// Tracks a run of consecutive [`Tap`]s (double-tap, triple-tap, ...),
/// each one required to begin within [`MultiTap::interval`] of the
/// previous tap's release and within [`MultiTap::slop`] of its position.
///
/// [`MultiTap`] does not itself implement [`Recognizer`] -- a
/// [`crate::arena::Arena`] resolves one contact's competition at a time,
/// while a multi-tap spans several *separate*, sequential contacts (each
/// its own arena competition). Instead, [`MultiTap`] is fed each
/// resolved [`Tap`]'s outcome via [`MultiTap::record_tap`], and reports
/// the running count itself.
pub struct MultiTap {
    interval: Duration,
    slop: f32,
    count: u32,
    last_position: Option<Point>,
    last_release: Option<Instant>,
}

impl Default for MultiTap {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiTap {
    /// Creates a [`MultiTap`] tracker using the default interval and
    /// slop.
    pub fn new() -> Self {
        Self {
            interval: DEFAULT_MULTI_TAP_INTERVAL,
            slop: DEFAULT_MULTI_TAP_SLOP,
            count: 0,
            last_position: None,
            last_release: None,
        }
    }

    /// Sets the maximum interval between consecutive taps.
    pub fn interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// Sets the maximum position drift between consecutive taps.
    pub fn slop(mut self, slop: f32) -> Self {
        self.slop = slop;
        self
    }

    /// Records that a [`Tap`] resolved at `position`/`released_at`,
    /// returning the new running count: `1` for a first tap or one that
    /// broke the chain (too slow, moved too far), `2` for a double-tap,
    /// and so on.
    pub fn record_tap(&mut self, position: Point, released_at: Instant) -> u32 {
        let continues_chain = match (self.last_position, self.last_release) {
            (Some(last_position), Some(last_release)) => {
                let within_time =
                    released_at.saturating_duration_since(last_release) <= self.interval;
                let within_slop = position.distance(last_position) <= self.slop;

                within_time && within_slop
            }
            _ => false,
        };

        self.count = if continues_chain { self.count + 1 } else { 1 };

        self.last_position = Some(position);
        self.last_release = Some(released_at);

        self.count
    }

    /// Resets the chain, as if no tap had ever been recorded.
    pub fn reset(&mut self) {
        self.count = 0;
        self.last_position = None;
        self.last_release = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taim_core::pointer::{DeviceKind, Metadata};

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
    fn a_quick_small_press_release_is_accepted() {
        let mut tap = Tap::new();
        tap.add_pointer(pointer(Phase::Down, Point::new(10.0, 10.0), 0));

        let disposition = tap.on_event(pointer(Phase::Up, Point::new(11.0, 11.0), 50));

        assert_eq!(disposition, Disposition::Accepted);
    }

    #[test]
    fn movement_beyond_slop_rejects() {
        let mut tap = Tap::new();
        tap.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));

        let disposition = tap.on_event(pointer(Phase::Move, Point::new(50.0, 0.0), 10));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn holding_past_the_timeout_rejects() {
        let mut tap = Tap::new().timeout(Duration::from_millis(100));
        tap.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));

        let disposition = tap.on_event(pointer(Phase::Up, Point::new(0.0, 0.0), 200));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn a_second_concurrent_pointer_rejects() {
        let mut tap = Tap::new();
        tap.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));
        tap.add_pointer(Pointer {
            id: PointerId::from_raw(2),
            ..pointer(Phase::Down, Point::new(5.0, 5.0), 1)
        });

        let disposition = tap.on_event(pointer(Phase::Up, Point::new(0.0, 0.0), 10));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn cancel_rejects() {
        let mut tap = Tap::new();
        tap.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));

        let disposition = tap.on_event(pointer(Phase::Cancel, Point::new(0.0, 0.0), 10));

        assert_eq!(disposition, Disposition::Rejected);
    }

    #[test]
    fn reset_allows_reuse_for_a_new_contact() {
        let mut tap = Tap::new();
        tap.add_pointer(pointer(Phase::Down, Point::new(0.0, 0.0), 0));
        let _ = tap.on_event(pointer(Phase::Move, Point::new(50.0, 0.0), 10));

        tap.reset();
        tap.add_pointer(pointer(Phase::Down, Point::new(100.0, 100.0), 100));

        let disposition = tap.on_event(pointer(Phase::Up, Point::new(101.0, 100.0), 110));

        assert_eq!(disposition, Disposition::Accepted);
    }

    #[test]
    fn multi_tap_counts_a_quick_double_tap() {
        let mut multi = MultiTap::new();

        assert_eq!(multi.record_tap(Point::new(0.0, 0.0), t(100)), 1);
        assert_eq!(multi.record_tap(Point::new(2.0, 2.0), t(250)), 2);
    }

    #[test]
    fn multi_tap_resets_the_chain_after_the_interval_elapses() {
        let mut multi = MultiTap::new().interval(Duration::from_millis(300));

        assert_eq!(multi.record_tap(Point::new(0.0, 0.0), t(0)), 1);
        assert_eq!(multi.record_tap(Point::new(0.0, 0.0), t(1000)), 1);
    }

    #[test]
    fn multi_tap_resets_the_chain_when_position_drifts_too_far() {
        let mut multi = MultiTap::new().slop(10.0);

        assert_eq!(multi.record_tap(Point::new(0.0, 0.0), t(0)), 1);
        assert_eq!(multi.record_tap(Point::new(100.0, 0.0), t(50)), 1);
    }

    #[test]
    fn multi_tap_reset_clears_the_chain() {
        let mut multi = MultiTap::new();
        let _ = multi.record_tap(Point::new(0.0, 0.0), t(0));

        multi.reset();

        assert_eq!(multi.record_tap(Point::new(0.0, 0.0), t(10)), 1);
    }
}

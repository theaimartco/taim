use taim_core::animation::{Motion, MotionClock};
use taim_core::time::{Duration, Instant};
use taim_motion::{Easing, Timestamp, Transition};

#[test]
fn rapid_reverse_preserves_the_sampled_position() {
    let start = Timestamp::from_seconds(0.0);
    let midpoint = Timestamp::from_seconds(0.3);
    let mut transition = Transition::new(0.0_f32, start);
    transition.retarget(1.0, start, 1.0, Easing::Linear);

    let before_reverse = transition.value_at(midpoint);
    transition.retarget(0.0, midpoint, 0.3, Easing::EaseOut);
    let at_reverse = transition.value_at(midpoint);

    assert!((before_reverse - at_reverse).abs() < f32::EPSILON);
    assert!(transition.is_running());
}

#[test]
fn immediate_transition_does_not_keep_idle_frames_alive() {
    let now = Timestamp::from_seconds(42.0);
    let mut transition = Transition::new(0.0_f32, now);
    transition.retarget(1.0, now, 0.0, Easing::Linear);

    assert_eq!(transition.value_at(now), 1.0);
    assert!(!transition.is_running());
}

#[test]
fn taim_compatibility_motion_uses_redraw_timestamps() {
    let mut motion = Motion::new(0.0_f32).duration(Duration::from_millis(200));
    let now = Instant::now();
    motion.go_mut(1.0, now);

    let midpoint = now + Duration::from_millis(100);
    assert!((motion.value_at(midpoint) - 0.5).abs() < 0.0001);
    assert!(motion.is_animating(midpoint));
    assert!(!motion.is_animating(now + Duration::from_millis(200)));

    // The adapter's origin is arbitrary; conversion remains monotonic.
    let clock = MotionClock::new(now);
    assert!(clock.at(midpoint).as_seconds() > 0.0);
}

//! Fixed-time motion oracles used in place of renderer-specific pixels.

use taim_motion::{Easing, Timestamp, Transition};

#[test]
fn fixed_time_transition_has_the_same_pixels_at_every_refresh_rate() {
    fn render_at(rate: u32) -> f32 {
        let start = Timestamp::from_seconds(0.0);
        let mut transition = Transition::new(0.0_f32, start);
        transition.retarget(1.0, start, 0.4, Easing::EaseInOut);

        let frame = (0.2 * rate as f64).round() / rate as f64;
        transition.value_at(Timestamp::from_seconds(frame))
    }

    let expected = 0.5;
    for rate in [60, 90, 120] {
        assert!((render_at(rate) - expected).abs() < 0.0001, "{rate} Hz");
    }
}

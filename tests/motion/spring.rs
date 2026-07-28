use aura_motion::{Bounds, Decay, Spring, SpringConfig};

#[test]
fn spring_matches_across_60_90_and_120_hz() {
    fn sample(step: f32) -> (f32, f32) {
        let mut spring = Spring::with_config(
            0.0,
            SpringConfig {
                angular_frequency: 14.0,
                damping_ratio: 0.82,
                epsilon: 0.000_01,
            },
        );
        spring.set_target(1.0);

        let mut elapsed = 0.0_f32;
        while elapsed < 1.0_f32 {
            let delta = (1.0_f32 - elapsed).min(step);
            let _ = spring.advance(delta);
            elapsed += delta;
        }
        (spring.value(), spring.velocity())
    }

    let sixty = sample(1.0 / 60.0);
    let ninety = sample(1.0 / 90.0);
    let one_twenty = sample(1.0 / 120.0);

    for actual in [ninety, one_twenty] {
        assert!((sixty.0 - actual.0).abs() < 0.0001);
        assert!((sixty.1 - actual.1).abs() < 0.0001);
    }
}

#[test]
fn decay_hands_off_at_a_bound_without_overshoot() {
    let mut decay = Decay::new(5.0, 600.0, 3.0).bounds(Bounds::new(0.0, 10.0));
    let result = decay.advance(0.1);

    assert_eq!(result.value, 10.0);
    assert_eq!(result.hit_bound, Some(10.0));
    assert_eq!(result.velocity, 0.0);
}

use aura_motion::{
    Effect, Keyframe, Keyframes, KeyframesError, MotionPolicy, Segment, SharedElementId,
    SharedElements, Snapshot, Timeline,
};

#[test]
fn keyframes_require_strict_normalized_offsets() {
    assert_eq!(
        Keyframes::<f32>::new(Vec::new()),
        Err(KeyframesError::Empty)
    );
    assert_eq!(
        Keyframes::new(vec![
            Keyframe {
                offset: 0.5,
                value: 1.0_f32,
            },
            Keyframe {
                offset: 0.5,
                value: 2.0,
            },
        ]),
        Err(KeyframesError::Unordered)
    );

    let frames = Keyframes::new(vec![
        Keyframe {
            offset: 0.0,
            value: 0.0_f32,
        },
        Keyframe {
            offset: 1.0,
            value: 10.0,
        },
    ])
    .expect("ordered normalized keyframes are valid");
    assert_eq!(frames.sample(0.3), 3.0);
}

#[test]
fn timeline_pause_reverse_seek_and_stagger_are_elapsed_time_based() {
    let mut timeline = Timeline::new(1.0);
    timeline.advance(0.3);
    assert!((timeline.progress() - 0.3).abs() < 0.000_001);

    timeline.pause();
    timeline.advance(2.0);
    assert!((timeline.progress() - 0.3).abs() < 0.000_001);

    timeline.resume();
    timeline.reverse();
    timeline.advance(0.1);
    assert!((timeline.progress() - 0.2).abs() < 0.000_001);

    let staggered = Segment {
        start: 0.2,
        duration: 0.4,
    };
    assert!((staggered.progress(0.4) - 0.5).abs() < 0.000_001);
}

#[test]
fn shared_element_registry_returns_every_layer_on_capacity_and_cancel() {
    let mut shared = SharedElements::new(1);
    let first = Snapshot {
        layer: 10_u32,
        width: 20.0,
        height: 30.0,
    };
    let second = Snapshot {
        layer: 20_u32,
        width: 20.0,
        height: 30.0,
    };

    assert_eq!(shared.register(SharedElementId(1), first), None);
    assert_eq!(shared.register(SharedElementId(2), second), Some(20));
    assert_eq!(shared.clear(), vec![10]);
    assert!(shared.is_empty());
}

#[test]
fn reduced_motion_preserves_a_short_semantic_cue() {
    assert_eq!(
        MotionPolicy::Reduced.effect(Effect::Transform),
        Effect::Fade
    );
    assert_eq!(MotionPolicy::Reduced.duration(Effect::Transform, 0.5), 0.15);
    assert_eq!(MotionPolicy::Reduced.duration(Effect::Instant, 0.5), 0.0);
}

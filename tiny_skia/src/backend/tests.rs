use crate::Renderer;
use crate::core::{self, Color, Point, Rectangle, Size, Transformation, renderer};

#[test]
fn retained_statistics_distinguish_hits_from_stale_cache_evictions() {
    let mut renderer = Renderer::new(renderer::Settings::default());
    let bounds = Rectangle::with_size(Size::new(100.0, 100.0));

    core::Renderer::reset(&mut renderer, bounds);
    core::Renderer::start_retained_layer(&mut renderer, 1, 1, bounds);
    core::Renderer::end_retained_layer(&mut renderer);
    core::Renderer::reset(&mut renderer, bounds);

    assert!(core::Renderer::replay_retained_layer(
        &mut renderer,
        1,
        1,
        bounds
    ));
    assert_eq!(core::Renderer::retained_layer_statistics(&renderer).hits, 1);

    core::Renderer::reset(&mut renderer, bounds);
    assert!(!core::Renderer::replay_retained_layer(
        &mut renderer,
        1,
        2,
        bounds
    ));

    let statistics = core::Renderer::retained_layer_statistics(&renderer);
    assert_eq!(statistics.misses, 1);
    assert_eq!(statistics.evictions, 1);
}

#[test]
fn nested_retained_recordings_balance_and_replay_the_innermost_list() {
    let mut renderer = Renderer::new(renderer::Settings::default());
    let outer = Rectangle::with_size(Size::new(100.0, 100.0));
    let inner = Rectangle::new(Point::new(10.0, 10.0), Size::new(20.0, 20.0));

    core::Renderer::reset(&mut renderer, outer);
    core::Renderer::start_retained_layer(&mut renderer, 1, 1, outer);
    core::Renderer::start_retained_layer(&mut renderer, 2, 1, inner);
    core::Renderer::fill_quad(
        &mut renderer,
        renderer::Quad {
            bounds: inner,
            ..renderer::Quad::default()
        },
        Color::BLACK,
    );
    core::Renderer::end_retained_layer(&mut renderer);
    core::Renderer::end_retained_layer(&mut renderer);

    assert!(renderer.recording_retained_frames.is_empty());
    assert_eq!(renderer.active_retained_frames.len(), 2);

    core::Renderer::reset(&mut renderer, outer);
    assert!(core::Renderer::replay_retained_layer(
        &mut renderer,
        2,
        1,
        inner
    ));
    assert_eq!(core::Renderer::retained_layer_statistics(&renderer).hits, 1);
}

#[test]
fn retained_replay_rejects_changed_ancestor_transform_and_opacity() {
    let mut renderer = Renderer::new(renderer::Settings::default());
    let bounds = Rectangle::with_size(Size::new(100.0, 100.0));

    core::Renderer::reset(&mut renderer, bounds);
    core::Renderer::start_transformation(&mut renderer, Transformation::translate(10.0, 5.0));
    core::Renderer::start_retained_layer(&mut renderer, 1, 1, bounds);
    core::Renderer::end_retained_layer(&mut renderer);
    core::Renderer::end_transformation(&mut renderer);
    core::Renderer::reset(&mut renderer, bounds);

    assert!(!core::Renderer::replay_retained_layer(
        &mut renderer,
        1,
        1,
        bounds
    ));

    core::Renderer::start_opacity(&mut renderer, 0.5);
    core::Renderer::start_retained_layer(&mut renderer, 2, 1, bounds);
    core::Renderer::end_retained_layer(&mut renderer);
    core::Renderer::end_opacity(&mut renderer);
    core::Renderer::reset(&mut renderer, bounds);

    assert!(!core::Renderer::replay_retained_layer(
        &mut renderer,
        2,
        1,
        bounds
    ));

    let statistics = core::Renderer::retained_layer_statistics(&renderer);
    assert_eq!(statistics.misses, 2);
    assert_eq!(statistics.evictions, 2);
}

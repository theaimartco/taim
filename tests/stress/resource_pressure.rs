use taim_core::widget::tree::Revision;
use taim_core::{Rectangle, Size, Transformation};
use taim_graphics::display_list::{Command, DisplayListBuilder, Generation};
use taim_graphics::retained_layer::{RetainedLayer, RetainedLayerCache, RetainedLayerKey};

#[test]
fn retained_layers_remain_bounded_and_drop_stale_resources_under_churn() {
    let budget = 4_096;
    let mut cache = RetainedLayerCache::with_budget(budget);
    let bounds = Rectangle::with_size(Size::new(320.0, 240.0));
    let generation = Generation::next();
    for _ in 0..10_000 {
        cache.advance_frame();
        let mut commands = DisplayListBuilder::new();
        commands.push(Command::Text {
            position: taim_core::Point::ORIGIN,
            color: taim_core::Color::WHITE,
            bounds,
            cache_generation: generation,
        });
        cache.insert(
            RetainedLayerKey::unique(),
            RetainedLayer::new(
                Revision::INITIAL,
                bounds,
                Transformation::IDENTITY,
                commands.build(),
            ),
        );
        assert!(cache.current_bytes() <= budget);
        assert_eq!(cache.telemetry().total_bytes, cache.current_bytes() as u64);
    }
    assert!(cache.telemetry().evictions > 9_000);
    assert!(!cache.is_empty());
    cache.purge_stale_generations(Generation::next());
    assert!(cache.is_empty());
    assert_eq!(cache.current_bytes(), 0);
    assert_eq!(cache.telemetry().total_bytes, 0);
}

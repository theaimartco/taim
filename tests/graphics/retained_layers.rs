//! Integration tests for the retained layer cache system.
//!
//! These tests verify cache behaviour from a consumer's perspective:
//! hits, misses, revision invalidation, resource generation staleness,
//! LRU eviction under budget pressure, and telemetry reporting.

use aura_graphics::core::widget::tree::Revision;
use aura_graphics::core::{Point, Rectangle, Size, Transformation};
use aura_graphics::display_list::{Command, DisplayListBuilder, Generation};
use aura_graphics::retained_layer::{
    CacheTelemetry, RetainedLayer, RetainedLayerCache, RetainedLayerKey,
};

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rectangle {
    Rectangle::new(Point::new(x, y), Size::new(w, h))
}

fn surface() -> Rectangle {
    rect(0.0, 0.0, 1920.0, 1080.0)
}

fn make_layer(revision: Revision) -> RetainedLayer {
    let display_list = DisplayListBuilder::new().build();
    RetainedLayer::new(revision, surface(), Transformation::IDENTITY, display_list)
}

fn make_layer_with_resource(revision: Revision, resource_gen: Generation) -> RetainedLayer {
    let mut builder = DisplayListBuilder::new();
    builder.push(Command::Text {
        position: Point::ORIGIN,
        color: aura_graphics::core::Color::WHITE,
        bounds: rect(0.0, 0.0, 100.0, 20.0),
        cache_generation: resource_gen,
    });
    let display_list = builder.build();
    RetainedLayer::new(revision, surface(), Transformation::IDENTITY, display_list)
}

// ── Cache hit/miss ─────────────────────────────────────────────────

#[test]
fn cache_hit_on_unchanged_revision() {
    let mut cache = RetainedLayerCache::new();
    let key = RetainedLayerKey::unique();
    let resource_gen = Generation::next();
    let revision = Revision::INITIAL;

    cache.insert(key, make_layer_with_resource(revision, resource_gen));

    let result = cache.get(
        key,
        revision,
        surface(),
        Transformation::IDENTITY,
        resource_gen,
    );
    assert!(
        result.is_some(),
        "layer with matching revision should be a cache hit"
    );
    assert_eq!(cache.telemetry().hits, 1);
}

#[test]
fn cache_miss_on_changed_revision() {
    let mut cache = RetainedLayerCache::new();
    let key = RetainedLayerKey::unique();
    let resource_gen = Generation::next();
    let old_revision = Revision::INITIAL;
    let new_revision = old_revision.next();

    cache.insert(key, make_layer_with_resource(old_revision, resource_gen));

    let result = cache.get(
        key,
        new_revision,
        surface(),
        Transformation::IDENTITY,
        resource_gen,
    );
    assert!(
        result.is_none(),
        "different revision should be a cache miss"
    );
    assert_eq!(cache.telemetry().misses, 1);
}

#[test]
fn cache_miss_on_missing_key() {
    let mut cache = RetainedLayerCache::new();
    let key = RetainedLayerKey::unique();
    let resource_gen = Generation::next();

    let result = cache.get(
        key,
        Revision::INITIAL,
        surface(),
        Transformation::IDENTITY,
        resource_gen,
    );
    assert!(result.is_none(), "missing key should be a cache miss");
}

// ── Revision invalidation ──────────────────────────────────────────

#[test]
fn revision_invalidation_removes_stale_layer() {
    let mut cache = RetainedLayerCache::new();
    let key = RetainedLayerKey::unique();
    let resource_gen = Generation::next();

    cache.insert(
        key,
        make_layer_with_resource(Revision::INITIAL, resource_gen),
    );
    assert_eq!(cache.len(), 1);

    // Query with bumped revision — stale layer is evicted
    let _ = cache.get(
        key,
        Revision::INITIAL.next(),
        surface(),
        Transformation::IDENTITY,
        resource_gen,
    );
    assert!(cache.is_empty(), "stale layer should be removed on miss");
}

#[test]
fn clip_change_invalidates_layer() {
    let mut cache = RetainedLayerCache::new();
    let key = RetainedLayerKey::unique();
    let resource_gen = Generation::next();

    cache.insert(
        key,
        make_layer_with_resource(Revision::INITIAL, resource_gen),
    );

    let different_clip = rect(10.0, 10.0, 100.0, 100.0);
    let result = cache.get(
        key,
        Revision::INITIAL,
        different_clip,
        Transformation::IDENTITY,
        resource_gen,
    );
    assert!(result.is_none(), "different clip should invalidate");
}

// ── Resource generation ────────────────────────────────────────────

#[test]
fn stale_resource_generation_evicts_layer() {
    let mut cache = RetainedLayerCache::new();
    let key = RetainedLayerKey::unique();
    let old_gen = Generation::next();
    let new_gen = Generation::next();

    cache.insert(key, make_layer_with_resource(Revision::INITIAL, old_gen));

    let result = cache.get(
        key,
        Revision::INITIAL,
        surface(),
        Transformation::IDENTITY,
        new_gen,
    );
    assert!(result.is_none(), "stale generation should evict layer");
}

#[test]
fn purge_stale_generations_removes_all_old_layers() {
    let mut cache = RetainedLayerCache::new();
    let old_gen = Generation::next();

    let key1 = RetainedLayerKey::unique();
    let key2 = RetainedLayerKey::unique();
    cache.insert(key1, make_layer_with_resource(Revision::INITIAL, old_gen));
    cache.insert(key2, make_layer_with_resource(Revision::INITIAL, old_gen));
    assert_eq!(cache.len(), 2);

    let new_gen = Generation::next();
    cache.purge_stale_generations(new_gen);
    assert!(
        cache.is_empty(),
        "all layers from old generation should be purged"
    );
    assert!(cache.telemetry().evictions >= 2);
}

// ── LRU eviction ───────────────────────────────────────────────────

#[test]
fn lru_eviction_under_budget_pressure() {
    // Each empty layer is estimated at 16 bytes (ARC_OVERHEAD only).
    // Budget of 32 allows 2 layers; inserting a 3rd forces eviction.
    let mut cache = RetainedLayerCache::with_budget(32);

    let key1 = RetainedLayerKey::unique();
    let key2 = RetainedLayerKey::unique();
    let key3 = RetainedLayerKey::unique();

    cache.insert(key1, make_layer(Revision::INITIAL));
    cache.advance_frame();
    cache.insert(key2, make_layer(Revision::INITIAL));
    cache.advance_frame();

    // key1 was inserted first and never accessed → least recently used
    cache.insert(key3, make_layer(Revision::INITIAL));

    assert!(
        cache.telemetry().evictions > 0,
        "eviction should have occurred"
    );
}

#[test]
fn recently_accessed_layers_survive_eviction() {
    // Sized to hold exactly the two resource-carrying layers below, so
    // the third insert is what forces an eviction.
    //
    // `make_layer_with_resource` produces a one-command display list,
    // which `estimate_display_list_bytes` puts at 16 + 96 = 112 bytes;
    // `make_layer` is an empty list at 16. A smaller budget would now be
    // rejected outright by `insert`'s oversized-entry guard -- an entry
    // larger than the whole budget cannot be retained without making the
    // cache unbounded -- and the eviction this test exists to observe
    // would never be reached.
    let mut cache = RetainedLayerCache::with_budget(224);
    let resource_gen = Generation::next();

    let key1 = RetainedLayerKey::unique();
    let key2 = RetainedLayerKey::unique();

    cache.insert(
        key1,
        make_layer_with_resource(Revision::INITIAL, resource_gen),
    );
    cache.advance_frame();
    cache.insert(
        key2,
        make_layer_with_resource(Revision::INITIAL, resource_gen),
    );
    cache.advance_frame();

    // Access key1 to make it recently used
    let _ = cache.get(
        key1,
        Revision::INITIAL,
        surface(),
        Transformation::IDENTITY,
        resource_gen,
    );
    cache.advance_frame();

    // Insert a third layer — should evict key2 (LRU), not key1
    let key3 = RetainedLayerKey::unique();
    cache.insert(key3, make_layer(Revision::INITIAL));

    assert!(
        cache.telemetry().evictions > 0,
        "the third insert should not have fit within the budget"
    );

    // The point of the test: recency, not insertion order, decides who
    // goes. key1 was touched after key2 was inserted, so key2 is the
    // least recently used and the one that should have been dropped.
    assert!(
        cache
            .get(
                key1,
                Revision::INITIAL,
                surface(),
                Transformation::IDENTITY,
                resource_gen,
            )
            .is_some(),
        "the recently accessed layer should have survived eviction"
    );

    assert!(cache.current_bytes() <= cache.budget_bytes());
}

// ── Purge ──────────────────────────────────────────────────────────

#[test]
fn purge_clears_all_layers_and_resets_bytes() {
    let mut cache = RetainedLayerCache::new();
    cache.insert(RetainedLayerKey::unique(), make_layer(Revision::INITIAL));
    cache.insert(RetainedLayerKey::unique(), make_layer(Revision::INITIAL));

    cache.purge();
    assert!(cache.is_empty());
    assert_eq!(cache.current_bytes(), 0);
}

#[test]
fn remove_specific_key() {
    let mut cache = RetainedLayerCache::new();
    let key = RetainedLayerKey::unique();
    cache.insert(key, make_layer(Revision::INITIAL));

    let removed = cache.remove(key);
    assert!(removed.is_some());
    assert!(cache.is_empty());
}

#[test]
fn remove_nonexistent_key_returns_none() {
    let mut cache = RetainedLayerCache::new();
    let result = cache.remove(RetainedLayerKey::unique());
    assert!(result.is_none());
}

// ── Telemetry ──────────────────────────────────────────────────────

#[test]
fn telemetry_hit_rate_calculation() {
    let mut telem = CacheTelemetry::default();
    assert_eq!(telem.hit_rate(), 0.0, "0/0 should be 0.0");

    telem.hits = 7;
    telem.misses = 3;
    assert!((telem.hit_rate() - 0.7).abs() < f32::EPSILON);
}

#[test]
fn telemetry_tracks_bytes_used() {
    let mut cache = RetainedLayerCache::new();
    assert_eq!(cache.current_bytes(), 0);

    cache.insert(RetainedLayerKey::unique(), make_layer(Revision::INITIAL));
    assert!(
        cache.current_bytes() > 0,
        "bytes should increase after insert"
    );
    assert_eq!(cache.telemetry().total_bytes, cache.current_bytes() as u64);
}

// ── Layer validity ─────────────────────────────────────────────────

#[test]
fn layer_validity_checks_all_four_conditions() {
    let resource_gen = Generation::next();
    let revision = Revision::INITIAL;
    let clip = surface();
    let transform = Transformation::IDENTITY;

    let layer = make_layer_with_resource(revision, resource_gen);

    // All match → valid
    assert!(layer.is_valid(revision, clip, transform, resource_gen));

    // Wrong revision → invalid
    assert!(!layer.is_valid(revision.next(), clip, transform, resource_gen));

    // Wrong clip → invalid
    assert!(!layer.is_valid(
        revision,
        rect(1.0, 1.0, 10.0, 10.0),
        transform,
        resource_gen
    ));

    // Wrong resource generation → invalid
    assert!(!layer.is_valid(revision, clip, transform, Generation::next()));
}

// ── Key uniqueness ─────────────────────────────────────────────────

#[test]
fn unique_keys_are_distinct() {
    let keys: Vec<RetainedLayerKey> = (0..100).map(|_| RetainedLayerKey::unique()).collect();
    let unique_count = keys.iter().collect::<std::collections::HashSet<_>>().len();
    assert_eq!(unique_count, 100);
}

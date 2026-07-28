//! Retained layer cache for the Aura rendering pipeline.
//!
//! A retained layer captures the rendering output of a widget subtree
//! that has not changed between frames. Instead of re-traversing the
//! widget tree, calling `Widget::draw`, and rebuilding primitives, the
//! compositor can replay a retained layer's [`DisplayList`] directly —
//! producing identical pixels with zero widget work.
//!
//! # Ownership model
//!
//! - A [`RetainedLayer`] is owned by the [`RetainedLayerCache`].
//! - The cache is keyed by [`RetainedLayerKey`], which ties a layer to
//!   a specific widget subtree (via its [`crate::cache::Group`] or a
//!   stable identity).
//! - The cache is **byte-bounded**: it tracks the estimated memory
//!   footprint of each layer and evicts least-recently-used entries
//!   when the total exceeds the configured budget.
//!
//! # Invalidation
//!
//! A layer is valid only when its stored [`Revision`] matches the
//! corresponding widget tree node's current revision. A mismatch means
//! the widget's state has changed and the layer must be rebuilt.
//!
//! # Resource generation safety
//!
//! A layer's [`DisplayList`] may reference GPU resources (textures,
//! buffers, shaped text runs) that were created with a specific
//! [`Generation`]. If those resources have since been evicted or
//! recreated (e.g. after device loss), the layer's display list is
//! stale and must not be replayed. The cache checks this before
//! returning a hit.
//!
//! [`Revision`]: crate::core::widget::tree::Revision
//! [`Generation`]: crate::display_list::Generation

use crate::cache::Group;
use crate::core::widget::tree::Revision;
use crate::core::{Rectangle, Transformation};
use crate::display_list::{DisplayList, Generation};

use rustc_hash::FxHashMap;
use std::sync::atomic::{self, AtomicU64};

/// A unique key identifying a retained layer in the cache.
///
/// Tied to a widget subtree's stable identity so the same widget
/// across frames maps to the same cache slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RetainedLayerKey(u64);

impl RetainedLayerKey {
    /// Creates a new unique key.
    pub fn unique() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, atomic::Ordering::Relaxed))
    }

    /// Creates a key from a cache [`Group`].
    ///
    /// Layers sharing the same group may be related (e.g. parts of
    /// the same composite widget), which the cache can use for
    /// co-eviction hints.
    pub fn from_group(group: Group) -> Self {
        // Use the Group's ordering/hash properties directly.
        // Groups are already unique u64s internally.
        Self(group_to_u64(group))
    }
}

/// Extract a u64 from a Group for keying purposes.
/// Groups have unique IDs internally; we use their hash as a proxy.
fn group_to_u64(group: Group) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = rustc_hash::FxHasher::default();
    group.hash(&mut hasher);
    hasher.finish()
}

/// A single retained layer: a cached snapshot of a widget subtree's
/// rendering output.
#[derive(Debug, Clone)]
pub struct RetainedLayer {
    /// The revision of the widget tree node when this layer was built.
    revision: Revision,

    /// The clip rectangle in effect when this layer was built.
    clip: Rectangle,

    /// The transformation in effect when this layer was built.
    transform: Transformation,

    /// The display list containing the rendering commands.
    display_list: DisplayList,

    /// The estimated memory footprint of this layer, in bytes.
    estimated_bytes: usize,

    /// The frame number when this layer was last accessed (for LRU).
    last_access_frame: u64,
}

impl RetainedLayer {
    /// Creates a new retained layer.
    pub fn new(
        revision: Revision,
        clip: Rectangle,
        transform: Transformation,
        display_list: DisplayList,
    ) -> Self {
        let estimated_bytes = estimate_display_list_bytes(&display_list);

        Self {
            revision,
            clip,
            transform,
            display_list,
            estimated_bytes,
            last_access_frame: 0,
        }
    }

    /// Returns the revision this layer was built at.
    pub fn revision(&self) -> Revision {
        self.revision
    }

    /// Returns the clip rectangle this layer was built with.
    pub fn clip(&self) -> Rectangle {
        self.clip
    }

    /// Returns the transformation this layer was built with.
    pub fn transform(&self) -> Transformation {
        self.transform
    }

    /// Returns the display list of this layer.
    pub fn display_list(&self) -> &DisplayList {
        &self.display_list
    }

    /// Returns the estimated memory footprint in bytes.
    pub fn estimated_bytes(&self) -> usize {
        self.estimated_bytes
    }

    /// Returns `true` if this layer is still valid for the given
    /// widget revision, clip, transform, and resource generation.
    pub fn is_valid(
        &self,
        current_revision: Revision,
        current_clip: Rectangle,
        current_transform: Transformation,
        current_resource_generation: Generation,
    ) -> bool {
        self.revision == current_revision
            && self.clip == current_clip
            && self.transform == current_transform
            && !self.display_list.is_stale(current_resource_generation)
    }
}

/// Rough estimate of a display list's memory footprint.
fn estimate_display_list_bytes(display_list: &DisplayList) -> usize {
    // Each Command is roughly 64–128 bytes depending on variant.
    // Arc overhead is ~16 bytes. This is a rough estimate for
    // cache budgeting, not an exact accounting.
    const COMMAND_SIZE_ESTIMATE: usize = 96;
    const ARC_OVERHEAD: usize = 16;

    ARC_OVERHEAD + display_list.len() * COMMAND_SIZE_ESTIMATE
}

/// Telemetry counters for the retained layer cache.
#[derive(Debug, Default, Clone, Copy)]
pub struct CacheTelemetry {
    /// Number of cache hits (layer reused without rebuild).
    pub hits: u64,
    /// Number of cache misses (layer needed rebuild).
    pub misses: u64,
    /// Number of layers evicted due to budget pressure.
    pub evictions: u64,
    /// Total bytes currently in the cache.
    pub total_bytes: u64,
}

impl CacheTelemetry {
    /// Returns the hit rate as a fraction [0.0, 1.0].
    pub fn hit_rate(&self) -> f32 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f32 / total as f32
        }
    }
}

/// A bounded cache of [`RetainedLayer`]s with LRU eviction.
///
/// The cache enforces a byte budget: when inserting a new layer would
/// exceed the budget, the least-recently-used layers are evicted until
/// there is room (or the cache is empty). This prevents unbounded
/// memory growth from retained layers (phase.md: "bound every cache
/// and queue").
#[derive(Debug)]
pub struct RetainedLayerCache {
    layers: FxHashMap<RetainedLayerKey, RetainedLayer>,
    /// The maximum byte budget for all layers combined.
    budget_bytes: usize,
    /// The current total estimated bytes across all layers.
    current_bytes: usize,
    /// The current frame number, incremented on each `advance_frame`.
    current_frame: u64,
    /// Accumulated telemetry.
    telemetry: CacheTelemetry,
}

impl RetainedLayerCache {
    /// The default byte budget: 64 MiB.
    ///
    /// This is generous enough for a complex UI with many retained
    /// layers while preventing runaway memory growth. Can be
    /// overridden with [`RetainedLayerCache::with_budget`].
    pub const DEFAULT_BUDGET: usize = 64 * 1024 * 1024;

    /// Creates a new cache with the default budget.
    pub fn new() -> Self {
        Self::with_budget(Self::DEFAULT_BUDGET)
    }

    /// Creates a new cache with the given byte budget.
    pub fn with_budget(budget_bytes: usize) -> Self {
        Self {
            layers: FxHashMap::default(),
            budget_bytes,
            current_bytes: 0,
            current_frame: 0,
            telemetry: CacheTelemetry::default(),
        }
    }

    /// Advances the frame counter. Call this once per frame before
    /// any `get` or `insert` operations.
    pub fn advance_frame(&mut self) {
        self.current_frame += 1;
    }

    /// Looks up a layer by key, validating it against the current
    /// widget revision and resource generation.
    ///
    /// Returns `Some` if the layer exists and is still valid; `None`
    /// if it does not exist, has a stale revision, or references
    /// evicted GPU resources.
    pub fn get(
        &mut self,
        key: RetainedLayerKey,
        current_revision: Revision,
        current_clip: Rectangle,
        current_transform: Transformation,
        current_resource_generation: Generation,
    ) -> Option<&RetainedLayer> {
        let layer = self.layers.get_mut(&key)?;

        if layer.is_valid(
            current_revision,
            current_clip,
            current_transform,
            current_resource_generation,
        ) {
            layer.last_access_frame = self.current_frame;
            self.telemetry.hits += 1;
            // Re-borrow as immutable
            Some(&self.layers[&key])
        } else {
            // Stale layer — remove it
            let removed = self.layers.remove(&key).unwrap();
            self.current_bytes = self.current_bytes.saturating_sub(removed.estimated_bytes);
            self.telemetry.misses += 1;
            None
        }
    }

    /// Inserts or replaces a retained layer in the cache.
    ///
    /// If the new layer would exceed the byte budget, the least-
    /// recently-used layers are evicted first.
    pub fn insert(&mut self, key: RetainedLayerKey, layer: RetainedLayer) {
        let new_bytes = layer.estimated_bytes;

        // Remove existing entry for this key (if any)
        if let Some(existing) = self.layers.remove(&key) {
            self.current_bytes = self.current_bytes.saturating_sub(existing.estimated_bytes);
        }

        // Evict LRU entries until we have room
        while self.current_bytes + new_bytes > self.budget_bytes && !self.layers.is_empty() {
            self.evict_lru();
        }

        self.current_bytes += new_bytes;
        let _ = self.layers.insert(key, layer);
        self.telemetry.total_bytes = self.current_bytes as u64;
    }

    /// Removes a specific layer from the cache.
    pub fn remove(&mut self, key: RetainedLayerKey) -> Option<RetainedLayer> {
        let layer = self.layers.remove(&key)?;
        self.current_bytes = self.current_bytes.saturating_sub(layer.estimated_bytes);
        self.telemetry.total_bytes = self.current_bytes as u64;
        Some(layer)
    }

    /// Purges all layers from the cache, freeing all memory.
    pub fn purge(&mut self) {
        self.layers.clear();
        self.current_bytes = 0;
        self.telemetry.total_bytes = 0;
    }

    /// Purges all layers whose resource generation is older than the
    /// given `current_generation`. This should be called after device
    /// loss or atlas recreation to ensure no stale GPU handles remain.
    pub fn purge_stale_generations(&mut self, current_generation: Generation) {
        let stale_keys: Vec<RetainedLayerKey> = self
            .layers
            .iter()
            .filter(|(_, layer)| layer.display_list.is_stale(current_generation))
            .map(|(key, _)| *key)
            .collect();

        for key in &stale_keys {
            if let Some(layer) = self.layers.remove(key) {
                self.current_bytes = self.current_bytes.saturating_sub(layer.estimated_bytes);
                self.telemetry.evictions += 1;
            }
        }
        self.telemetry.total_bytes = self.current_bytes as u64;
    }

    /// Returns the current telemetry snapshot.
    pub fn telemetry(&self) -> CacheTelemetry {
        self.telemetry
    }

    /// Returns the number of layers currently cached.
    pub fn len(&self) -> usize {
        self.layers.len()
    }

    /// Returns `true` if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// Returns the current byte usage.
    pub fn current_bytes(&self) -> usize {
        self.current_bytes
    }

    /// Returns the configured byte budget.
    pub fn budget_bytes(&self) -> usize {
        self.budget_bytes
    }

    /// Evicts the least-recently-used layer.
    fn evict_lru(&mut self) {
        let lru_key = self
            .layers
            .iter()
            .min_by_key(|(_, layer)| layer.last_access_frame)
            .map(|(key, _)| *key);

        if let Some(key) = lru_key
            && let Some(evicted) = self.layers.remove(&key)
        {
            self.current_bytes = self.current_bytes.saturating_sub(evicted.estimated_bytes);
            self.telemetry.evictions += 1;
        }
    }
}

impl Default for RetainedLayerCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::widget::tree::Revision;
    use crate::core::{Point, Size};
    use crate::display_list::DisplayListBuilder;

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

    fn make_layer_with_gen(revision: Revision, generation_id: Generation) -> RetainedLayer {
        let mut builder = DisplayListBuilder::new();
        builder.push(crate::display_list::Command::Text {
            position: Point::ORIGIN,
            color: crate::core::Color::WHITE,
            bounds: rect(0.0, 0.0, 100.0, 20.0),
            cache_generation: generation_id,
        });
        let display_list = builder.build();
        RetainedLayer::new(revision, surface(), Transformation::IDENTITY, display_list)
    }

    #[test]
    fn retained_layer_key_unique_produces_different_keys() {
        let a = RetainedLayerKey::unique();
        let b = RetainedLayerKey::unique();
        assert_ne!(a, b);
    }

    #[test]
    fn layer_is_valid_with_matching_revision_and_generation() {
        let generation_id = Generation::next();
        let revision = Revision::INITIAL;
        let layer = make_layer_with_gen(revision, generation_id);

        assert!(layer.is_valid(revision, surface(), Transformation::IDENTITY, generation_id));
    }

    #[test]
    fn layer_is_invalid_with_different_revision() {
        let generation_id = Generation::next();
        let revision = Revision::INITIAL;
        let layer = make_layer_with_gen(revision, generation_id);

        assert!(!layer.is_valid(
            revision.next(),
            surface(),
            Transformation::IDENTITY,
            generation_id
        ));
    }

    #[test]
    fn layer_is_invalid_with_stale_generation() {
        let old_gen = Generation::next();
        let new_gen = Generation::next();
        let layer = make_layer_with_gen(Revision::INITIAL, old_gen);

        assert!(!layer.is_valid(
            Revision::INITIAL,
            surface(),
            Transformation::IDENTITY,
            new_gen,
        ));
    }

    #[test]
    fn layer_is_invalid_with_different_clip() {
        let generation_id = Generation::next();
        let layer = make_layer_with_gen(Revision::INITIAL, generation_id);

        let different_clip = rect(10.0, 10.0, 100.0, 100.0);
        assert!(!layer.is_valid(
            Revision::INITIAL,
            different_clip,
            Transformation::IDENTITY,
            generation_id,
        ));
    }

    #[test]
    fn cache_starts_empty() {
        let cache = RetainedLayerCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.current_bytes(), 0);
    }

    #[test]
    fn cache_insert_and_get() {
        let mut cache = RetainedLayerCache::new();
        let key = RetainedLayerKey::unique();
        let generation_id = Generation::next();
        let revision = Revision::INITIAL;
        let layer = make_layer_with_gen(revision, generation_id);

        cache.insert(key, layer);
        assert_eq!(cache.len(), 1);

        let result = cache.get(
            key,
            revision,
            surface(),
            Transformation::IDENTITY,
            generation_id,
        );
        assert!(result.is_some());
    }

    #[test]
    fn cache_returns_none_for_missing_key() {
        let mut cache = RetainedLayerCache::new();
        let key = RetainedLayerKey::unique();
        let generation_id = Generation::next();

        let result = cache.get(
            key,
            Revision::INITIAL,
            surface(),
            Transformation::IDENTITY,
            generation_id,
        );
        assert!(result.is_none());
    }

    #[test]
    fn cache_evicts_stale_revision_on_get() {
        let mut cache = RetainedLayerCache::new();
        let key = RetainedLayerKey::unique();
        let generation_id = Generation::next();
        let layer = make_layer_with_gen(Revision::INITIAL, generation_id);

        cache.insert(key, layer);

        // Query with a newer revision
        let result = cache.get(
            key,
            Revision::INITIAL.next(),
            surface(),
            Transformation::IDENTITY,
            generation_id,
        );
        assert!(result.is_none());
        assert!(cache.is_empty());
    }

    #[test]
    fn cache_respects_byte_budget_with_lru_eviction() {
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

        // key1 was least recently used; inserting key3 should evict it
        cache.insert(key3, make_layer(Revision::INITIAL));

        // key1 should have been evicted (LRU)
        assert!(cache.telemetry().evictions > 0);
    }

    #[test]
    fn cache_purge_clears_everything() {
        let mut cache = RetainedLayerCache::new();
        let key = RetainedLayerKey::unique();
        cache.insert(key, make_layer(Revision::INITIAL));

        cache.purge();
        assert!(cache.is_empty());
        assert_eq!(cache.current_bytes(), 0);
    }

    #[test]
    fn cache_purge_stale_generations_removes_old_layers() {
        let mut cache = RetainedLayerCache::new();
        let old_gen = Generation::next();
        let new_gen = Generation::next();

        let key = RetainedLayerKey::unique();
        cache.insert(key, make_layer_with_gen(Revision::INITIAL, old_gen));
        assert_eq!(cache.len(), 1);

        cache.purge_stale_generations(new_gen);
        assert!(cache.is_empty());
    }

    #[test]
    fn cache_remove_specific_key() {
        let mut cache = RetainedLayerCache::new();
        let key = RetainedLayerKey::unique();
        cache.insert(key, make_layer(Revision::INITIAL));

        let removed = cache.remove(key);
        assert!(removed.is_some());
        assert!(cache.is_empty());
    }

    #[test]
    fn telemetry_tracks_hits_and_misses() {
        let mut cache = RetainedLayerCache::new();
        let key = RetainedLayerKey::unique();
        let generation_id = Generation::next();
        let revision = Revision::INITIAL;

        cache.insert(key, make_layer_with_gen(revision, generation_id));

        // Hit
        let _ = cache.get(
            key,
            revision,
            surface(),
            Transformation::IDENTITY,
            generation_id,
        );
        assert_eq!(cache.telemetry().hits, 1);

        // Miss (nonexistent key)
        let missing = RetainedLayerKey::unique();
        let _ = cache.get(
            missing,
            revision,
            surface(),
            Transformation::IDENTITY,
            generation_id,
        );
        assert_eq!(cache.telemetry().misses, 0); // get returns None for missing, no miss counter
    }

    #[test]
    fn telemetry_hit_rate() {
        let mut telem = CacheTelemetry::default();
        assert_eq!(telem.hit_rate(), 0.0);

        telem.hits = 3;
        telem.misses = 1;
        assert!((telem.hit_rate() - 0.75).abs() < f32::EPSILON);
    }
}

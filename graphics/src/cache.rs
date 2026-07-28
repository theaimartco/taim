//! Cache computations and efficiently reuse them.
use std::cell::RefCell;
use std::fmt;
use std::mem;
use std::sync::atomic::{self, AtomicU64};

/// A simple cache that stores generated values to avoid recomputation.
///
/// Keeps track of the last generated value after clearing.
pub struct Cache<T> {
    group: Group,
    state: RefCell<State<T>>,
    version: RefCell<u64>,
}

static VERSION: AtomicU64 = AtomicU64::new(0);

impl<T> Cache<T> {
    /// Creates a new empty [`Cache`].
    pub fn new() -> Self {
        Cache {
            group: Group::singleton(),
            state: RefCell::new(State::Empty { previous: None }),
            version: RefCell::new(VERSION.load(atomic::Ordering::Relaxed)),
        }
    }

    /// Creates a new empty [`Cache`] with the given [`Group`].
    ///
    /// Caches within the same group may reuse internal rendering storage.
    ///
    /// You should generally group caches that are likely to change
    /// together.
    pub fn with_group(group: Group) -> Self {
        assert!(
            !group.is_singleton(),
            "The group {group:?} cannot be shared!"
        );

        Cache {
            group,
            state: RefCell::new(State::Empty { previous: None }),
            version: RefCell::new(VERSION.load(atomic::Ordering::Relaxed)),
        }
    }

    /// Returns the [`Group`] of the [`Cache`].
    pub fn group(&self) -> Group {
        self.group
    }

    /// Puts the given value in the [`Cache`].
    ///
    /// Notice that, given this is a cache, a mutable reference is not
    /// necessary to call this method. You can safely update the cache in
    /// rendering code.
    pub fn put(&self, value: T) {
        *self.state.borrow_mut() = State::Filled { current: value };
    }

    /// Returns a reference cell to the internal [`State`] of the [`Cache`].
    pub fn state(&self) -> &RefCell<State<T>> {
        let version = VERSION.load(atomic::Ordering::Relaxed);

        if *self.version.borrow() != version {
            *self.state.borrow_mut() = State::Empty { previous: None };
            let _ = self.version.replace(version);
        }

        &self.state
    }

    /// Clears the [`Cache`].
    pub fn clear(&self) {
        let mut state = self.state.borrow_mut();

        let previous = mem::replace(&mut *state, State::Empty { previous: None });

        let previous = match previous {
            State::Empty { previous } => previous,
            State::Filled { current } => Some(current),
        };

        *state = State::Empty { previous };
    }
}

/// A cache group.
///
/// Caches that share the same group generally change together.
///
/// A cache group can be used to implement certain performance
/// optimizations during rendering, like batching or sharing atlases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Group {
    id: u64,
    is_singleton: bool,
}

impl Group {
    /// Generates a new unique cache [`Group`].
    pub fn unique() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);

        Self {
            id: NEXT.fetch_add(1, atomic::Ordering::Relaxed),
            is_singleton: false,
        }
    }

    /// Returns `true` if the [`Group`] can only ever have a
    /// single [`Cache`] in it.
    ///
    /// This is the default kind of [`Group`] assigned when using
    /// [`Cache::new`].
    ///
    /// Knowing that a [`Group`] will never be shared may be
    /// useful for rendering backends to perform additional
    /// optimizations.
    pub fn is_singleton(self) -> bool {
        self.is_singleton
    }

    fn singleton() -> Self {
        Self {
            is_singleton: true,
            ..Self::unique()
        }
    }
}

impl<T> fmt::Debug for Cache<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::ops::Deref;

        let state = self.state.borrow();

        match state.deref() {
            State::Empty { previous } => {
                write!(f, "Cache::Empty {{ previous: {previous:?} }}")
            }
            State::Filled { current } => {
                write!(f, "Cache::Filled {{ current: {current:?} }}")
            }
        }
    }
}

impl<T> Default for Cache<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// The state of a [`Cache`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State<T> {
    /// The [`Cache`] is empty.
    Empty {
        /// The previous value of the [`Cache`].
        previous: Option<T>,
    },
    /// The [`Cache`] is filled.
    Filled {
        /// The current value of the [`Cache`]
        current: T,
    },
}

/// A piece of data that can be cached.
pub trait Cached: Sized {
    /// The type of cache produced.
    type Cache: Clone;

    /// Loads the [`Cache`] into a proper instance.
    ///
    /// [`Cache`]: Self::Cache
    fn load(cache: &Self::Cache) -> Self;

    /// Caches this value, producing its corresponding [`Cache`].
    ///
    /// [`Cache`]: Self::Cache
    fn cache(self, group: Group, previous: Option<Self::Cache>) -> Self::Cache;
}

#[cfg(debug_assertions)]
impl Cached for () {
    type Cache = ();

    fn load(_cache: &Self::Cache) -> Self {}

    fn cache(self, _group: Group, _previous: Option<Self::Cache>) -> Self::Cache {}
}

/// Invalidates all the existing [`Cache`]; clearing their contents.
pub fn invalidate_all() {
    let _ = VERSION.fetch_add(1, atomic::Ordering::Relaxed);
}

// ── Byte-bounded generational cache ────────────────────────────────
//
// Phase 4 introduces `BoundedCache`, a generic key-value cache with:
// - Configurable byte budget
// - LRU eviction when the budget is exceeded
// - Generation-based staleness detection
// - Purge API with telemetry

use rustc_hash::FxHashMap;
use std::hash::Hash;

/// Telemetry from a [`BoundedCache`] purge operation.
#[derive(Debug, Clone, Copy, Default)]
pub struct PurgeTelemetry {
    /// Number of entries evicted.
    pub entries_evicted: usize,
    /// Total bytes freed.
    pub bytes_freed: usize,
    /// Entries remaining after purge.
    pub entries_remaining: usize,
    /// Bytes remaining after purge.
    pub bytes_remaining: usize,
}

/// A trait for values that can report their estimated byte size.
pub trait ByteSized {
    /// Returns the estimated memory footprint in bytes.
    fn estimated_bytes(&self) -> usize;
}

/// An entry in the [`BoundedCache`].
#[derive(Debug)]
struct BoundedEntry<V> {
    value: V,
    estimated_bytes: usize,
    last_access: u64,
    generation: u64,
}

/// A byte-bounded, generational cache with LRU eviction.
///
/// Stores key-value pairs up to a configured byte budget. When
/// inserting a new entry would exceed the budget, the least-recently-
/// used entries are evicted until there is room. Each entry carries
/// a generation stamp for staleness detection after GPU resource
/// recreation.
///
/// This is a generic utility used by the retained layer cache,
/// display list cache, and resource caches throughout the rendering
/// pipeline.
#[derive(Debug)]
pub struct BoundedCache<K, V> {
    entries: FxHashMap<K, BoundedEntry<V>>,
    budget_bytes: usize,
    current_bytes: usize,
    access_counter: u64,
    current_generation: u64,
    total_evictions: u64,
}

impl<K: Hash + Eq + Clone, V: ByteSized> BoundedCache<K, V> {
    /// Creates a new cache with the given byte budget.
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            entries: FxHashMap::default(),
            budget_bytes,
            current_bytes: 0,
            access_counter: 0,
            current_generation: 0,
            total_evictions: 0,
        }
    }

    /// Returns the configured byte budget.
    pub fn budget_bytes(&self) -> usize {
        self.budget_bytes
    }

    /// Returns the current byte usage.
    pub fn current_bytes(&self) -> usize {
        self.current_bytes
    }

    /// Returns the number of entries in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the current generation.
    pub fn generation(&self) -> u64 {
        self.current_generation
    }

    /// Returns the total number of evictions since this cache was created.
    pub fn total_evictions(&self) -> u64 {
        self.total_evictions
    }

    /// Looks up a value by key, updating its LRU access time.
    ///
    /// Returns `None` if the key is not present or the entry's
    /// generation does not match the current generation.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        self.access_counter += 1;
        let current_gen = self.current_generation;
        let access = self.access_counter;

        if let Some(entry) = self.entries.get_mut(key) {
            if entry.generation != current_gen {
                // Stale entry — evict it
                let bytes = entry.estimated_bytes;
                let _ = self.entries.remove(key);
                self.current_bytes = self.current_bytes.saturating_sub(bytes);
                self.total_evictions += 1;
                return None;
            }
            entry.last_access = access;
            // Re-borrow immutably
            Some(&self.entries[key].value)
        } else {
            None
        }
    }

    /// Inserts or replaces a value in the cache.
    ///
    /// Evicts LRU entries if necessary to stay within budget.
    pub fn insert(&mut self, key: K, value: V) {
        let bytes = value.estimated_bytes();

        // Remove existing entry
        if let Some(existing) = self.entries.remove(&key) {
            self.current_bytes = self.current_bytes.saturating_sub(existing.estimated_bytes);
        }

        // Evict LRU until we have room
        while self.current_bytes + bytes > self.budget_bytes && !self.entries.is_empty() {
            self.evict_lru();
        }

        self.access_counter += 1;
        self.current_bytes += bytes;

        let _ = self.entries.insert(
            key,
            BoundedEntry {
                value,
                estimated_bytes: bytes,
                last_access: self.access_counter,
                generation: self.current_generation,
            },
        );
    }

    /// Removes a specific entry.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let entry = self.entries.remove(key)?;
        self.current_bytes = self.current_bytes.saturating_sub(entry.estimated_bytes);
        Some(entry.value)
    }

    /// Bumps the generation counter. All entries from previous
    /// generations become stale and will be evicted on next access.
    pub fn bump_generation(&mut self) {
        self.current_generation += 1;
    }

    /// Purges all stale-generation entries, returning telemetry
    /// about what was evicted.
    pub fn purge_stale(&mut self) -> PurgeTelemetry {
        let current_gen = self.current_generation;
        let before_count = self.entries.len();
        let before_bytes = self.current_bytes;

        // Collect keys to remove (can't mutate self inside retain)
        let stale_keys: Vec<K> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.generation != current_gen)
            .map(|(key, _)| key.clone())
            .collect();

        for key in &stale_keys {
            if let Some(entry) = self.entries.remove(key) {
                self.current_bytes = self.current_bytes.saturating_sub(entry.estimated_bytes);
                self.total_evictions += 1;
            }
        }

        let evicted = before_count - self.entries.len();
        let freed = before_bytes - self.current_bytes;

        PurgeTelemetry {
            entries_evicted: evicted,
            bytes_freed: freed,
            entries_remaining: self.entries.len(),
            bytes_remaining: self.current_bytes,
        }
    }

    /// Purges all entries from the cache.
    pub fn purge_all(&mut self) -> PurgeTelemetry {
        let telemetry = PurgeTelemetry {
            entries_evicted: self.entries.len(),
            bytes_freed: self.current_bytes,
            entries_remaining: 0,
            bytes_remaining: 0,
        };

        self.entries.clear();
        self.current_bytes = 0;

        telemetry
    }

    /// Evicts the least-recently-used entry.
    fn evict_lru(&mut self) {
        let lru_key = self
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_access)
            .map(|(key, _)| key.clone());

        if let Some(key) = lru_key
            && let Some(entry) = self.entries.remove(&key)
        {
            self.current_bytes = self.current_bytes.saturating_sub(entry.estimated_bytes);
            self.total_evictions += 1;
        }
    }
}

#[cfg(test)]
mod bounded_cache_tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct TestValue {
        data: u32,
        size: usize,
    }

    impl ByteSized for TestValue {
        fn estimated_bytes(&self) -> usize {
            self.size
        }
    }

    fn val(data: u32, size: usize) -> TestValue {
        TestValue { data, size }
    }

    #[test]
    fn empty_cache() {
        let cache: BoundedCache<u32, TestValue> = BoundedCache::new(1024);
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.current_bytes(), 0);
    }

    #[test]
    fn insert_and_get() {
        let mut cache = BoundedCache::new(1024);
        cache.insert(1_u32, val(42, 100));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.current_bytes(), 100);

        let result = cache.get(&1);
        assert!(result.is_some());
        assert_eq!(result.unwrap().data, 42);
    }

    #[test]
    fn get_missing_returns_none() {
        let mut cache: BoundedCache<u32, TestValue> = BoundedCache::new(1024);
        assert!(cache.get(&999).is_none());
    }

    #[test]
    fn budget_enforced_via_lru_eviction() {
        let mut cache = BoundedCache::new(250);
        cache.insert(1_u32, val(1, 100));
        cache.insert(2_u32, val(2, 100));

        // Access key 1 to make it more recent
        let _ = cache.get(&1);

        // This should evict key 2 (LRU)
        cache.insert(3_u32, val(3, 100));

        assert!(
            cache.get(&1).is_some(),
            "key 1 should survive (recently used)"
        );
        assert!(cache.get(&2).is_none(), "key 2 should be evicted (LRU)");
        assert!(cache.get(&3).is_some(), "key 3 should be present");
    }

    #[test]
    fn remove_specific_entry() {
        let mut cache = BoundedCache::new(1024);
        cache.insert(1_u32, val(42, 100));

        let removed = cache.remove(&1);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().data, 42);
        assert!(cache.is_empty());
        assert_eq!(cache.current_bytes(), 0);
    }

    #[test]
    fn generation_makes_old_entries_stale() {
        let mut cache = BoundedCache::new(1024);
        cache.insert(1_u32, val(42, 100));
        cache.bump_generation();

        // Entry from old generation is stale
        assert!(cache.get(&1).is_none());
        assert!(cache.is_empty());
    }

    #[test]
    fn purge_stale_removes_old_generation_entries() {
        let mut cache = BoundedCache::new(1024);
        cache.insert(1_u32, val(1, 100));
        cache.insert(2_u32, val(2, 100));
        cache.bump_generation();
        cache.insert(3_u32, val(3, 100)); // current generation

        let telemetry = cache.purge_stale();
        assert_eq!(telemetry.entries_evicted, 2);
        assert_eq!(telemetry.bytes_freed, 200);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn purge_all_clears_everything() {
        let mut cache = BoundedCache::new(1024);
        cache.insert(1_u32, val(1, 100));
        cache.insert(2_u32, val(2, 200));

        let telemetry = cache.purge_all();
        assert_eq!(telemetry.entries_evicted, 2);
        assert_eq!(telemetry.bytes_freed, 300);
        assert!(cache.is_empty());
    }

    #[test]
    fn replacing_existing_key_updates_bytes() {
        let mut cache = BoundedCache::new(1024);
        cache.insert(1_u32, val(1, 100));
        assert_eq!(cache.current_bytes(), 100);

        cache.insert(1_u32, val(2, 200));
        assert_eq!(cache.current_bytes(), 200);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn total_evictions_tracks_all_evictions() {
        let mut cache = BoundedCache::new(150);
        cache.insert(1_u32, val(1, 100));
        cache.insert(2_u32, val(2, 100)); // evicts key 1

        assert!(cache.total_evictions() >= 1);
    }
}

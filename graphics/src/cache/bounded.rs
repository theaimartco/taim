//! Hard-budget generational cache for retained rendering resources.
use rustc_hash::FxHashMap;
use std::hash::Hash;

/// Telemetry produced by a purge.
#[derive(Debug, Clone, Copy, Default)]
pub struct PurgeTelemetry {
    /// Number of entries removed.
    pub entries_evicted: usize,
    /// Estimated bytes released.
    pub bytes_freed: usize,
    /// Entries still retained.
    pub entries_remaining: usize,
    /// Estimated bytes still retained.
    pub bytes_remaining: usize,
}

/// A value that can estimate its retained memory footprint.
pub trait ByteSized {
    /// Returns the value's estimated retained byte size.
    fn estimated_bytes(&self) -> usize;
}

#[derive(Debug)]
struct Entry<V> {
    value: V,
    bytes: usize,
    last_access: u64,
    generation: u64,
}

/// A byte-bounded LRU cache with cheap generation invalidation.
#[derive(Debug)]
pub struct BoundedCache<K, V> {
    entries: FxHashMap<K, Entry<V>>,
    budget_bytes: usize,
    current_bytes: usize,
    access_counter: u64,
    current_generation: u64,
    total_evictions: u64,
}

impl<K: Hash + Eq + Clone, V: ByteSized> BoundedCache<K, V> {
    /// Creates an empty cache with a hard byte budget.
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

    #[inline]
    /// Returns the configured hard byte budget.
    pub fn budget_bytes(&self) -> usize {
        self.budget_bytes
    }

    #[inline]
    /// Returns estimated bytes currently retained.
    pub fn current_bytes(&self) -> usize {
        self.current_bytes
    }

    #[inline]
    /// Returns the number of retained entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[inline]
    /// Returns whether no entries are retained.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[inline]
    /// Returns the active resource generation.
    pub fn generation(&self) -> u64 {
        self.current_generation
    }

    #[inline]
    /// Returns the lifetime number of automatic and purge evictions.
    pub fn total_evictions(&self) -> u64 {
        self.total_evictions
    }

    /// Reads an entry and promotes it in LRU order. Stale entries are dropped
    /// lazily so a generation bump remains O(1).
    pub fn get(&mut self, key: &K) -> Option<&V> {
        self.advance_access_clock();
        let generation = self.current_generation;
        let access = self.access_counter;
        let stale_bytes = self
            .entries
            .get(key)
            .and_then(|entry| (entry.generation != generation).then_some(entry.bytes));

        if let Some(bytes) = stale_bytes {
            let _stale = self.entries.remove(key);
            self.current_bytes = self.current_bytes.saturating_sub(bytes);
            self.total_evictions = self.total_evictions.saturating_add(1);
            return None;
        }

        let entry = self.entries.get_mut(key)?;
        entry.last_access = access;
        Some(&entry.value)
    }

    /// Inserts a value while strictly preserving the configured byte budget.
    pub fn insert(&mut self, key: K, value: V) {
        let bytes = value.estimated_bytes();

        if let Some(existing) = self.entries.remove(&key) {
            self.current_bytes = self.current_bytes.saturating_sub(existing.bytes);
        }

        if bytes > self.budget_bytes {
            return;
        }

        self.evict_until_fits(bytes);
        self.advance_access_clock();
        self.current_bytes = self.current_bytes.saturating_add(bytes);

        let _replaced = self.entries.insert(
            key,
            Entry {
                value,
                bytes,
                last_access: self.access_counter,
                generation: self.current_generation,
            },
        );
    }

    /// Removes and returns a specific value.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let entry = self.entries.remove(key)?;
        self.current_bytes = self.current_bytes.saturating_sub(entry.bytes);
        Some(entry.value)
    }

    /// Invalidates existing entries without walking the map.
    pub fn bump_generation(&mut self) {
        if self.current_generation == u64::MAX {
            let _telemetry = self.purge_all();
            self.current_generation = 0;
        } else {
            self.current_generation += 1;
        }
    }

    /// Removes every stale entry in one allocation-free map pass.
    pub fn purge_stale(&mut self) -> PurgeTelemetry {
        let generation = self.current_generation;
        let before_count = self.entries.len();
        let before_bytes = self.current_bytes;
        let mut evicted = 0usize;
        let mut freed = 0usize;

        self.entries.retain(|_, entry| {
            let keep = entry.generation == generation;
            if !keep {
                evicted += 1;
                freed = freed.saturating_add(entry.bytes);
            }
            keep
        });

        self.current_bytes = self.current_bytes.saturating_sub(freed);
        self.total_evictions = self.total_evictions.saturating_add(evicted as u64);
        debug_assert_eq!(evicted, before_count - self.entries.len());
        debug_assert_eq!(freed, before_bytes - self.current_bytes);

        PurgeTelemetry {
            entries_evicted: evicted,
            bytes_freed: freed,
            entries_remaining: self.entries.len(),
            bytes_remaining: self.current_bytes,
        }
    }

    /// Removes every retained entry and returns accounting telemetry.
    pub fn purge_all(&mut self) -> PurgeTelemetry {
        let telemetry = PurgeTelemetry {
            entries_evicted: self.entries.len(),
            bytes_freed: self.current_bytes,
            entries_remaining: 0,
            bytes_remaining: 0,
        };

        self.total_evictions = self
            .total_evictions
            .saturating_add(self.entries.len() as u64);
        self.entries.clear();
        self.current_bytes = 0;
        telemetry
    }

    /// Performs one ordering pass under memory pressure. This avoids the
    /// repeated full-map scans of a naïve per-entry LRU loop.
    fn evict_until_fits(&mut self, incoming: usize) {
        if self.current_bytes.saturating_add(incoming) <= self.budget_bytes {
            return;
        }

        let mut oldest = self
            .entries
            .iter()
            .map(|(key, entry)| (entry.last_access, key.clone()))
            .collect::<Vec<_>>();
        oldest.sort_unstable_by_key(|(access, _)| *access);

        for (_, key) in oldest {
            if let Some(entry) = self.entries.remove(&key) {
                self.current_bytes = self.current_bytes.saturating_sub(entry.bytes);
                self.total_evictions = self.total_evictions.saturating_add(1);
            }

            if self.current_bytes.saturating_add(incoming) <= self.budget_bytes {
                break;
            }
        }
    }

    #[inline]
    fn advance_access_clock(&mut self) {
        if self.access_counter == u64::MAX {
            let minimum = self
                .entries
                .values()
                .map(|entry| entry.last_access)
                .min()
                .unwrap_or(0);

            for entry in self.entries.values_mut() {
                entry.last_access = entry.last_access.saturating_sub(minimum);
            }

            self.access_counter = self
                .entries
                .values()
                .map(|entry| entry.last_access)
                .max()
                .unwrap_or(0);
        }

        self.access_counter += 1;
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!(cache.get(&1).map(|value| value.data), Some(42));
    }

    #[test]
    fn hard_budget_rejects_oversized_values() {
        let mut cache = BoundedCache::new(64);
        cache.insert(1_u32, val(1, 65));
        assert!(cache.is_empty());
        assert_eq!(cache.current_bytes(), 0);
    }

    #[test]
    fn budget_pressure_uses_lru_order() {
        let mut cache = BoundedCache::new(250);
        cache.insert(1_u32, val(1, 100));
        cache.insert(2_u32, val(2, 100));
        let _hit = cache.get(&1);
        cache.insert(3_u32, val(3, 100));

        assert!(cache.get(&1).is_some());
        assert!(cache.get(&2).is_none());
        assert!(cache.get(&3).is_some());
    }

    #[test]
    fn generation_staleness_is_lazy() {
        let mut cache = BoundedCache::new(1024);
        cache.insert(1_u32, val(42, 100));
        cache.bump_generation();
        assert!(cache.get(&1).is_none());
        assert!(cache.is_empty());
    }

    #[test]
    fn stale_purge_reports_freed_storage() {
        let mut cache = BoundedCache::new(1024);
        cache.insert(1_u32, val(1, 100));
        cache.insert(2_u32, val(2, 100));
        cache.bump_generation();
        cache.insert(3_u32, val(3, 100));

        let telemetry = cache.purge_stale();
        assert_eq!(telemetry.entries_evicted, 2);
        assert_eq!(telemetry.bytes_freed, 200);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn replacing_updates_accounting() {
        let mut cache = BoundedCache::new(1024);
        cache.insert(1_u32, val(1, 100));
        cache.insert(1_u32, val(2, 200));
        assert_eq!(cache.current_bytes(), 200);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn purge_all_updates_telemetry_and_counter() {
        let mut cache = BoundedCache::new(1024);
        cache.insert(1_u32, val(1, 100));
        cache.insert(2_u32, val(2, 200));

        let telemetry = cache.purge_all();
        assert_eq!(telemetry.entries_evicted, 2);
        assert_eq!(telemetry.bytes_freed, 300);
        assert_eq!(cache.total_evictions(), 2);
        assert!(cache.is_empty());
    }
}

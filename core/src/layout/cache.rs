//! A bounded, multi-entry measure cache for layout results.
//!
//! [`widget::Tree::layout_with_cache`](crate::widget::Tree::layout_with_cache)
//! already gives every widget a single-slot cache: it reuses a node's
//! last measurement only when the incoming [`Limits`] are exactly the
//! ones it was last measured against. That is enough for a node
//! measured under one stable set of constraints, but it thrashes to
//! nothing the moment a node is repeatedly measured against a *handful*
//! of different [`Limits`] -- exactly what happens inside a flex
//! container's multi-pass solver (`layout::flex::resolve`), where the
//! same child can be asked for its size under several candidate
//! constraint sets across the static/cross-fluid/min-max/main-fluid
//! passes, and, across frames, may oscillate between a small set of
//! recurring [`Limits`] (e.g. during a resize-settle animation).
//!
//! [`Cache`] fixes this by keeping the `N` most recently used entries
//! instead of just one, evicting the least-recently-used entry once
//! full so memory stays proportional to how many distinct constraint
//! sets are actually in rotation, not to how many measurements have ever
//! happened.
use crate::layout::{Limits, LimitsKey, Node};
use smallvec::SmallVec;

/// A caller-supplied fingerprint of everything about a node's *content*
/// that would invalidate a cached measurement even under unchanged
/// [`Limits`] -- e.g. its text, font, the active scale factor, and text
/// direction.
///
/// [`Cache`] cannot detect these on its own (unlike [`Limits`], content
/// is not something a layout cache can see), so a correct cache key must
/// fold them in explicitly: two calls with equal [`Limits`] but a
/// different [`Fingerprint`] are treated as unrelated entries, exactly
/// as if the [`Limits`] themselves had differed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Fingerprint(u64);

impl Fingerprint {
    /// A [`Fingerprint`] that never matches a fingerprint built from
    /// real content -- useful as a placeholder when a caller has no
    /// content-sensitive state to track and relies solely on [`Limits`]
    /// for invalidation.
    pub const NONE: Self = Self(0);

    /// Builds a [`Fingerprint`] from any [`std::hash::Hash`] value.
    pub fn of(value: impl std::hash::Hash) -> Self {
        use std::hash::Hasher;

        let mut hasher = rustc_hash::FxHasher::default();
        value.hash(&mut hasher);

        // Distinguish a real all-zero hash from `NONE`.
        Self(hasher.finish() | 1)
    }

    /// Builds a [`Fingerprint`] by writing directly into a
    /// [`std::hash::Hasher`], for values that cannot implement
    /// [`std::hash::Hash`] themselves -- most notably
    /// [`Length`](crate::Length), which holds `f32` fields and instead
    /// exposes [`Length::hash_stable`](crate::Length::hash_stable).
    pub fn build(write: impl FnOnce(&mut dyn std::hash::Hasher)) -> Self {
        let mut hasher = rustc_hash::FxHasher::default();
        write(&mut hasher);

        use std::hash::Hasher;
        Self(hasher.finish() | 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    limits: LimitsKey,
    fingerprint: Fingerprint,
}

#[derive(Debug, Clone)]
struct Entry {
    key: Key,
    node: Node,
}

/// A bounded, least-recently-used cache of [`Limits`]-keyed layout
/// measurements.
///
/// Holds at most [`Cache::capacity`] entries; inserting beyond that
/// evicts the least-recently-used entry first. A [`Cache`] with capacity
/// `1` behaves like
/// [`widget::Tree::layout_with_cache`](crate::widget::Tree::layout_with_cache)
/// (modulo the added [`Fingerprint`] dimension), but most containers
/// that face a handful of recurring constraint sets should use a small
/// capacity greater than `1` (4-8 is typically enough to cover a resize
/// settling between a couple of sizes, or a multi-pass solver
/// re-querying the same child).
#[derive(Debug, Clone)]
pub struct Cache {
    capacity: usize,
    // Most-recently-used entry is at the back. Inline capacity of 4 avoids heap allocations.
    entries: SmallVec<[Entry; 4]>,
}

impl Cache {
    /// Creates an empty [`Cache`] with the given capacity.
    ///
    /// # Panics
    /// Panics if `capacity` is `0` -- a zero-capacity cache can never
    /// store anything and almost certainly indicates a bug at the call
    /// site rather than an intentional "never cache" request (use
    /// [`Cache::disabled`] for that instead).
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Cache capacity must be greater than zero");

        Self {
            capacity,
            entries: SmallVec::with_capacity(capacity),
        }
    }

    /// Creates a [`Cache`] that never stores anything, so every
    /// [`Cache::get_or_insert_with`] call always recomputes.
    ///
    /// Useful for a call site that wants to opt out of caching (e.g.
    /// while debugging a suspected invalidation bug) without threading
    /// an `Option<Cache>` through its caller.
    pub fn disabled() -> Self {
        Self {
            capacity: 0,
            entries: SmallVec::new(),
        }
    }

    /// Returns the maximum number of entries this [`Cache`] holds.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the number of entries currently stored.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the [`Cache`] holds no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Removes every entry, forcing the next lookup for any [`Limits`]
    /// to recompute.
    ///
    /// Call this whenever something changed that the cache has no other
    /// way of observing and that invalidates *every* entry at once (a
    /// content change not captured by the [`Fingerprint`] a caller is
    /// using is a caller bug, not a reason to reach for this -- prefer
    /// folding the change into the [`Fingerprint`] so unrelated entries
    /// need not be thrown away).
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns the cached [`Node`] for the given [`Limits`] and
    /// [`Fingerprint`], computing and storing it with `compute` if no
    /// matching entry exists.
    ///
    /// A cache with capacity `0` (see [`Cache::disabled`]) always calls
    /// `compute` and never stores the result.
    pub fn get_or_insert_with(
        &mut self,
        limits: &Limits,
        fingerprint: Fingerprint,
        compute: impl FnOnce() -> Node,
    ) -> Node {
        if self.capacity == 0 {
            return compute();
        }

        let key = Key {
            limits: limits.key(),
            fingerprint,
        };

        if let Some(position) = self.entries.iter().position(|entry| entry.key == key) {
            let entry = self.entries.remove(position);
            let node = entry.node.clone();

            self.entries.push(entry);

            return node;
        }

        let node = compute();

        if self.entries.len() >= self.capacity {
            let _ = self.entries.remove(0);
        }

        self.entries.push(Entry {
            key,
            node: node.clone(),
        });

        node
    }
}

impl Default for Cache {
    /// Creates a [`Cache`] with a small capacity (4) suitable for most
    /// containers that face a handful of recurring constraint sets.
    fn default() -> Self {
        Self::new(4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Size;
    use std::cell::Cell;

    fn limits(width: f32, height: f32) -> Limits {
        Limits::new(Size::ZERO, Size::new(width, height))
    }

    #[test]
    fn a_fresh_cache_computes_on_first_lookup() {
        let mut cache = Cache::new(4);
        let calls = Cell::new(0);

        let node = cache.get_or_insert_with(&limits(10.0, 10.0), Fingerprint::NONE, || {
            calls.set(calls.get() + 1);
            Node::new(Size::new(10.0, 10.0))
        });

        assert_eq!(calls.get(), 1);
        assert_eq!(node.size(), Size::new(10.0, 10.0));
    }

    #[test]
    fn repeating_the_same_limits_and_fingerprint_reuses_the_entry() {
        let mut cache = Cache::new(4);
        let calls = Cell::new(0);

        let compute = |calls: &Cell<u32>| {
            calls.set(calls.get() + 1);
            Node::new(Size::new(10.0, 10.0))
        };

        let _ =
            cache.get_or_insert_with(&limits(10.0, 10.0), Fingerprint::NONE, || compute(&calls));
        let _ =
            cache.get_or_insert_with(&limits(10.0, 10.0), Fingerprint::NONE, || compute(&calls));

        assert_eq!(
            calls.get(),
            1,
            "the second lookup must reuse the cached entry"
        );
    }

    #[test]
    fn different_limits_are_cached_independently() {
        let mut cache = Cache::new(4);
        let calls = Cell::new(0);

        let compute = |calls: &Cell<u32>| {
            calls.set(calls.get() + 1);
            Node::new(Size::new(10.0, 10.0))
        };

        let _ =
            cache.get_or_insert_with(&limits(10.0, 10.0), Fingerprint::NONE, || compute(&calls));
        let _ =
            cache.get_or_insert_with(&limits(20.0, 20.0), Fingerprint::NONE, || compute(&calls));
        let _ =
            cache.get_or_insert_with(&limits(10.0, 10.0), Fingerprint::NONE, || compute(&calls));

        assert_eq!(
            calls.get(),
            2,
            "both distinct limits must be cached, and reused thereafter"
        );
    }

    #[test]
    fn different_fingerprints_under_the_same_limits_are_cached_independently() {
        let mut cache = Cache::new(4);
        let calls = Cell::new(0);

        let compute = |calls: &Cell<u32>| {
            calls.set(calls.get() + 1);
            Node::new(Size::new(10.0, 10.0))
        };

        let _ = cache.get_or_insert_with(&limits(10.0, 10.0), Fingerprint::of("font-a"), || {
            compute(&calls)
        });
        let _ = cache.get_or_insert_with(&limits(10.0, 10.0), Fingerprint::of("font-b"), || {
            compute(&calls)
        });

        assert_eq!(
            calls.get(),
            2,
            "a fingerprint change must invalidate the cache even under identical limits, \
             modeling a font/scale/direction change"
        );
    }

    #[test]
    fn exceeding_capacity_evicts_the_least_recently_used_entry() {
        let mut cache = Cache::new(2);
        let calls = Cell::new(0);

        let compute = |calls: &Cell<u32>| {
            calls.set(calls.get() + 1);
            Node::new(Size::new(10.0, 10.0))
        };

        let _ = cache.get_or_insert_with(&limits(1.0, 1.0), Fingerprint::NONE, || compute(&calls));
        let _ = cache.get_or_insert_with(&limits(2.0, 2.0), Fingerprint::NONE, || compute(&calls));
        // Inserting a third entry must evict `limits(1.0, 1.0)`, the least-recently-used.
        let _ = cache.get_or_insert_with(&limits(3.0, 3.0), Fingerprint::NONE, || compute(&calls));

        assert_eq!(calls.get(), 3);
        assert_eq!(cache.len(), 2);

        let _ = cache.get_or_insert_with(&limits(1.0, 1.0), Fingerprint::NONE, || compute(&calls));
        assert_eq!(calls.get(), 4, "the evicted entry must be recomputed");
    }

    #[test]
    fn using_an_entry_marks_it_as_recently_used_and_protects_it_from_eviction() {
        let mut cache = Cache::new(2);
        let calls = Cell::new(0);

        let compute = |calls: &Cell<u32>| {
            calls.set(calls.get() + 1);
            Node::new(Size::new(10.0, 10.0))
        };

        let _ = cache.get_or_insert_with(&limits(1.0, 1.0), Fingerprint::NONE, || compute(&calls));
        let _ = cache.get_or_insert_with(&limits(2.0, 2.0), Fingerprint::NONE, || compute(&calls));

        // Touch `limits(1.0, 1.0)` again so it becomes the most-recently-used.
        let _ = cache.get_or_insert_with(&limits(1.0, 1.0), Fingerprint::NONE, || compute(&calls));
        assert_eq!(calls.get(), 2, "re-touching must still be a cache hit");

        // Now `limits(2.0, 2.0)` is the least-recently-used and should be evicted.
        let _ = cache.get_or_insert_with(&limits(3.0, 3.0), Fingerprint::NONE, || compute(&calls));

        let _ = cache.get_or_insert_with(&limits(1.0, 1.0), Fingerprint::NONE, || compute(&calls));
        assert_eq!(
            calls.get(),
            3,
            "limits(1.0, 1.0) must have survived the eviction"
        );

        let _ = cache.get_or_insert_with(&limits(2.0, 2.0), Fingerprint::NONE, || compute(&calls));
        assert_eq!(calls.get(), 4, "limits(2.0, 2.0) must have been evicted");
    }

    #[test]
    fn clear_forces_every_entry_to_recompute() {
        let mut cache = Cache::new(4);
        let calls = Cell::new(0);

        let compute = |calls: &Cell<u32>| {
            calls.set(calls.get() + 1);
            Node::new(Size::new(10.0, 10.0))
        };

        let _ = cache.get_or_insert_with(&limits(1.0, 1.0), Fingerprint::NONE, || compute(&calls));
        cache.clear();
        let _ = cache.get_or_insert_with(&limits(1.0, 1.0), Fingerprint::NONE, || compute(&calls));

        assert_eq!(calls.get(), 2);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn a_disabled_cache_never_reuses_a_result() {
        let mut cache = Cache::disabled();
        let calls = Cell::new(0);

        let compute = |calls: &Cell<u32>| {
            calls.set(calls.get() + 1);
            Node::new(Size::new(10.0, 10.0))
        };

        let _ = cache.get_or_insert_with(&limits(1.0, 1.0), Fingerprint::NONE, || compute(&calls));
        let _ = cache.get_or_insert_with(&limits(1.0, 1.0), Fingerprint::NONE, || compute(&calls));

        assert_eq!(calls.get(), 2);
        assert!(cache.is_empty());
    }

    #[test]
    #[should_panic(expected = "capacity")]
    fn zero_capacity_panics() {
        let _ = Cache::new(0);
    }

    #[test]
    fn fingerprint_none_never_collides_with_a_hashed_value() {
        assert_ne!(Fingerprint::NONE, Fingerprint::of(0u64));
        assert_ne!(Fingerprint::NONE, Fingerprint::of(""));
    }
}

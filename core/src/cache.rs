//! Cache policy vocabulary shared by every bounded cache in later phases.
//!
//! `phase.md` rule 4 requires every cache and queue to have a budget and
//! an eviction/backpressure behavior. This module defines the shared
//! vocabulary ([`Policy`], [`Budget`], [`PurgeReason`]) that Phase 4's
//! layer/display-list caches, Phase 5's GPU resource caches, and Phase
//! 10's text-shaping caches are expected to describe themselves with, so
//! diagnostics and purge logic can be written once against a common
//! shape instead of once per cache.
//!
//! This module does not implement a cache; it defines what a cache
//! reports about itself.

/// How a cache should be sized and evicted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    /// The [`Budget`] this cache must stay within.
    pub budget: Budget,
    /// The class of content this cache holds, used to prioritize
    /// eviction when multiple caches compete for the same resource
    /// (e.g. GPU memory) under pressure.
    pub class: Class,
}

impl Policy {
    /// Creates a new [`Policy`] with the given [`Budget`] and [`Class`].
    pub const fn new(budget: Budget, class: Class) -> Self {
        Self { budget, class }
    }
}

/// The size limit of a cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Budget {
    /// A limit on the number of bytes the cache's content may occupy.
    ///
    /// This is the preferred budget for anything backed by GPU or raster
    /// memory (images, glyph atlases, retained layers), since byte count
    /// is what actually bounds memory pressure.
    Bytes(u64),
    /// A limit on the number of entries the cache may hold, regardless
    /// of their individual size.
    ///
    /// Appropriate for caches whose entries have a roughly uniform, small
    /// cost (e.g. a bounded LRU of recently used layout results).
    Entries(u32),
}

/// The class of content a cache holds, used to prioritize eviction under
/// pressure (see `runtime::memory_pressure`, added in a later phase).
///
/// Ordered from most to least disposable: [`Class::Prefetched`] content
/// was never actually needed by the current frame and should be the
/// first evicted; [`Class::Essential`] content backs what is on screen
/// right now and should be the last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Class {
    /// Speculatively loaded ahead of need (e.g. off-screen list rows).
    /// Safe to evict first; nothing currently visible depends on it.
    Prefetched,
    /// Recently used but not currently visible (e.g. a scrolled-away
    /// image). Cheap to evict; will simply be recomputed/reloaded if
    /// needed again.
    Retained,
    /// Backs content that is currently visible or about to be presented.
    /// Evicting this causes a visible flash or a synchronous rebuild;
    /// only evict under genuine memory pressure.
    Essential,
}

/// Why a cache entry was purged.
///
/// Reported to `runtime::diagnostics` (Phase 2) and later phases'
/// profiler (Phase 15) so a cache miss can be explained rather than only
/// observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurgeReason {
    /// The entry's key is no longer reachable from the current widget
    /// tree (e.g. a removed list row).
    NoLongerReferenced,
    /// The cache exceeded its [`Budget`] and evicted the
    /// least-recently-used entry to make room.
    BudgetExceeded,
    /// The entry's underlying resource became invalid (e.g. a GPU device
    /// loss) and must be rebuilt from scratch.
    ResourceInvalidated,
    /// The whole cache was cleared, typically in response to a theme,
    /// scale factor, or content-affecting settings change.
    Cleared,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_orders_from_most_to_least_disposable() {
        assert!(Class::Prefetched < Class::Retained);
        assert!(Class::Retained < Class::Essential);
    }

    #[test]
    fn policy_stores_its_budget_and_class() {
        let policy = Policy::new(Budget::Bytes(1024), Class::Retained);

        assert_eq!(policy.budget, Budget::Bytes(1024));
        assert_eq!(policy.class, Class::Retained);
    }
}

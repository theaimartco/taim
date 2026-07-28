//! Bounded, reusable intermediate-texture accounting for effect passes.
//!
//! The renderer owns the actual GPU textures; this pool owns only the
//! byte accounting and reuse bookkeeping that keeps effect intermediates
//! within a hard budget (`phase.md` Phase 12: "reusable intermediate
//! texture pool with byte budget", "no effect can trigger unbounded
//! texture allocation").
//!
//! # Accounting model
//!
//! - `used` is the total bytes of textures currently *live* (acquired
//!   and not yet released).
//! - `reserved` is the total bytes of *idle* textures kept around in the
//!   free list for reuse.
//! - `used + reserved` never exceeds `budget`: an acquire that would
//!   push the combined footprint past the budget first evicts idle
//!   textures, and fails (allocating nothing) if it still would not fit.
//!
//! Releasing a texture moves its bytes from `used` to `reserved` (it
//! becomes reusable, not freed); acquiring a matching descriptor moves
//! them back with no new allocation. This is the fix for the original
//! accounting, where `release` never decremented `used`, so the pool's
//! footprint grew monotonically and the budget exhausted even under
//! perfect reuse.

use std::collections::BTreeMap;

/// A texture descriptor used for reusable effect intermediates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Descriptor {
    /// Texture width in physical pixels.
    pub width: u32,
    /// Texture height in physical pixels.
    pub height: u32,
    /// Bytes per pixel of the texture format.
    pub bytes_per_pixel: u8,
}

impl Descriptor {
    /// Returns the byte footprint of one texture of this descriptor.
    pub fn bytes(self) -> u64 {
        u64::from(self.width) * u64::from(self.height) * u64::from(self.bytes_per_pixel)
    }
}

/// A byte-bounded intermediate-texture pool. Actual texture creation
/// remains owned by the renderer; this tracks only reuse and budget.
#[derive(Debug)]
pub struct Pool {
    budget: u64,
    used: u64,
    reserved: u64,
    free: BTreeMap<Descriptor, usize>,
}

impl Pool {
    /// Creates a pool with a hard byte budget.
    pub fn new(budget: u64) -> Self {
        Self {
            budget,
            used: 0,
            reserved: 0,
            free: BTreeMap::new(),
        }
    }

    /// Acquires a texture matching `descriptor`, reusing an idle one when
    /// possible. Returns `false` (allocating nothing) when honoring the
    /// request would exceed the budget even after evicting every idle
    /// texture -- the concrete "no unbounded allocation" guarantee.
    pub fn acquire(&mut self, descriptor: Descriptor) -> bool {
        let bytes = descriptor.bytes();

        // Reuse an idle texture of exactly this descriptor: move its
        // bytes from `reserved` back to `used`, no new allocation.
        if let Some(count) = self.free.get_mut(&descriptor)
            && *count > 0
        {
            *count -= 1;
            if *count == 0 {
                let _ = self.free.remove(&descriptor);
            }
            self.reserved = self.reserved.saturating_sub(bytes);
            self.used += bytes;
            return true;
        }

        // A single texture larger than the whole budget can never fit.
        if bytes > self.budget {
            return false;
        }

        // Make room by evicting idle (reserved) textures if the live +
        // idle footprint would otherwise exceed the budget.
        if self.used + self.reserved + bytes > self.budget {
            let ceiling = self.budget.saturating_sub(self.used + bytes);
            self.evict_until(ceiling);
        }

        if self.used + self.reserved + bytes > self.budget {
            return false;
        }

        self.used += bytes;
        true
    }

    /// Releases a texture back to the pool, keeping it idle and reusable
    /// (its bytes move from `used` to `reserved`) rather than freeing it.
    pub fn release(&mut self, descriptor: Descriptor) {
        let bytes = descriptor.bytes();
        // Guard against releasing more than was acquired.
        self.used = self.used.saturating_sub(bytes);
        self.reserved += bytes;
        *self.free.entry(descriptor).or_default() += 1;
    }

    /// Evicts idle textures (largest first) until reserved bytes fit
    /// within `target_reserved_ceiling`, or nothing idle remains.
    fn evict_until(&mut self, target_reserved_ceiling: u64) {
        while self.reserved > target_reserved_ceiling {
            // Evict the largest idle descriptor first for the biggest
            // reclaim per eviction.
            let Some((&descriptor, _)) = self.free.iter().next_back() else {
                break;
            };

            let count = self.free.get_mut(&descriptor).expect("just found it");
            *count -= 1;
            if *count == 0 {
                let _ = self.free.remove(&descriptor);
            }
            self.reserved = self.reserved.saturating_sub(descriptor.bytes());
        }
    }

    /// Returns bytes currently live (acquired and not released).
    pub fn used_bytes(&self) -> u64 {
        self.used
    }

    /// Returns bytes currently idle in the free list (reusable).
    pub fn reserved_bytes(&self) -> u64 {
        self.reserved
    }

    /// Returns the total committed footprint (live + idle).
    pub fn footprint_bytes(&self) -> u64 {
        self.used + self.reserved
    }

    /// Returns the hard byte budget.
    pub fn budget_bytes(&self) -> u64 {
        self.budget
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(width: u32, height: u32) -> Descriptor {
        Descriptor {
            width,
            height,
            bytes_per_pixel: 4,
        }
    }

    #[test]
    fn release_reclaims_used_bytes_into_reserved() {
        let one = descriptor(32, 32);
        let mut pool = Pool::new(one.bytes() * 4);

        assert!(pool.acquire(one));
        assert_eq!(pool.used_bytes(), one.bytes());
        assert_eq!(pool.reserved_bytes(), 0);

        pool.release(one);
        assert_eq!(pool.used_bytes(), 0);
        assert_eq!(pool.reserved_bytes(), one.bytes());
    }

    #[test]
    fn reuse_does_not_grow_the_footprint() {
        let one = descriptor(32, 32);
        let mut pool = Pool::new(one.bytes());

        for _ in 0..1_000 {
            assert!(pool.acquire(one));
            pool.release(one);
        }
        assert!(pool.footprint_bytes() <= pool.budget_bytes());
    }

    #[test]
    fn footprint_never_exceeds_budget_across_mixed_sizes() {
        let mut pool = Pool::new(descriptor(64, 64).bytes() * 2);
        let a = descriptor(64, 64);
        let b = descriptor(32, 32);

        assert!(pool.acquire(a));
        assert!(pool.acquire(b));
        assert!(pool.footprint_bytes() <= pool.budget_bytes());

        // A third live texture that overflows is refused, allocating
        // nothing.
        let before = pool.footprint_bytes();
        assert!(!pool.acquire(a));
        assert_eq!(pool.footprint_bytes(), before);
    }

    #[test]
    fn a_texture_over_the_whole_budget_is_refused() {
        let mut pool = Pool::new(1_000);
        assert!(!pool.acquire(descriptor(64, 64)));
        assert_eq!(pool.footprint_bytes(), 0);
    }

    #[test]
    fn eviction_makes_room_for_a_new_acquire_from_idle_textures() {
        // Budget for one 64x64. Acquire+release leaves it idle; acquiring
        // a same-footprint texture reuses it rather than failing.
        let d = descriptor(64, 64);
        let mut pool = Pool::new(d.bytes());
        assert!(pool.acquire(d));
        pool.release(d);
        assert!(pool.acquire(d));
        assert!(pool.footprint_bytes() <= pool.budget_bytes());
    }
}

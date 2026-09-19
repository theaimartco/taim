//! Opt-in allocation counting.
//!
//! The counting allocator wraps the system allocator with a pair of
//! atomic counters. It is only installed as the process's global
//! allocator when the `profiling` feature is enabled, since a counting
//! allocator adds overhead to every allocation in the process --
//! including the harness's own bookkeeping -- and would otherwise bias
//! every other metric this tool reports.
//!
//! `unsafe_code = "deny"` is the workspace default for the production
//! engine crates (phase.md rule 8: no unsafe without an RFC); this
//! dev-only benchmark tool needs it for exactly one thing -- a
//! `GlobalAlloc` impl that forwards every call unmodified to `System`
//! after bumping a counter, which the type system cannot express
//! safely. The safety invariant is documented at each `unsafe` site
//! below.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// A snapshot of allocation activity between two points in time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AllocationSnapshot {
    /// Number of allocations performed.
    pub allocations: u64,
    /// Number of deallocations performed.
    pub deallocations: u64,
    /// Total bytes allocated (not net; frees do not subtract from this).
    pub bytes_allocated: u64,
    /// Live (allocated minus freed) bytes at the time of the snapshot.
    pub live_bytes: i64,
}

impl AllocationSnapshot {
    /// The difference between this (later) snapshot and an earlier one,
    /// suitable for reporting "allocations during this frame/run".
    pub fn since(&self, earlier: &Self) -> Self {
        Self {
            allocations: self.allocations.saturating_sub(earlier.allocations),
            deallocations: self.deallocations.saturating_sub(earlier.deallocations),
            bytes_allocated: self.bytes_allocated.saturating_sub(earlier.bytes_allocated),
            live_bytes: self.live_bytes - earlier.live_bytes,
        }
    }

    /// Adds another measured UI interval without including time spent by the
    /// harness between intervals (notably screenshot readback).
    pub fn accumulate(&mut self, interval: Self) {
        self.allocations = self.allocations.saturating_add(interval.allocations);
        self.deallocations = self.deallocations.saturating_add(interval.deallocations);
        self.bytes_allocated = self
            .bytes_allocated
            .saturating_add(interval.bytes_allocated);
        self.live_bytes = self.live_bytes.saturating_add(interval.live_bytes);
    }
}

// Only constructed by the `#[global_allocator]` static below, which is
// itself only compiled in under the `profiling` feature; genuinely dead
// otherwise, not a bug.
#[cfg_attr(not(feature = "profiling"), allow(dead_code))]
struct CountingAllocator;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static DEALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static BYTES_ALLOCATED: AtomicU64 = AtomicU64::new(0);
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        let _ = BYTES_ALLOCATED.fetch_add(layout.size() as u64, Ordering::Relaxed);
        let _ = LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed);

        // SAFETY: forwards to the system allocator with the same layout
        // the caller gave us; this wrapper only adds bookkeeping around
        // an otherwise-unmodified call.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let _ = DEALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        let _ = LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);

        // SAFETY: forwards to the system allocator with the same
        // pointer/layout the caller received from `alloc` above.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[cfg(feature = "profiling")]
#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

/// Takes a snapshot of allocation counters at this instant.
///
/// Outside the `profiling` feature, the counters are never incremented
/// (the counting allocator is not installed), so every snapshot reads as
/// zero. Callers should treat a zero snapshot as "not measured", not as
/// "zero allocations occurred" -- see `docs/performance/methodology.md`.
pub fn snapshot() -> AllocationSnapshot {
    AllocationSnapshot {
        allocations: ALLOCATIONS.load(Ordering::Relaxed),
        deallocations: DEALLOCATIONS.load(Ordering::Relaxed),
        bytes_allocated: BYTES_ALLOCATED.load(Ordering::Relaxed),
        live_bytes: LIVE_BYTES.load(Ordering::Relaxed) as i64,
    }
}

/// Whether allocation counting is actually active in this build.
pub const fn is_active() -> bool {
    cfg!(feature = "profiling")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn since_computes_a_nonnegative_delta_for_monotonic_counters() {
        let earlier = AllocationSnapshot {
            allocations: 10,
            deallocations: 5,
            bytes_allocated: 1000,
            live_bytes: 500,
        };

        let later = AllocationSnapshot {
            allocations: 15,
            deallocations: 8,
            bytes_allocated: 1500,
            live_bytes: 300,
        };

        let delta = later.since(&earlier);

        assert_eq!(delta.allocations, 5);
        assert_eq!(delta.deallocations, 3);
        assert_eq!(delta.bytes_allocated, 500);
        assert_eq!(delta.live_bytes, -200);
    }

    #[test]
    fn accumulation_excludes_unmeasured_gaps() {
        let mut total = AllocationSnapshot::default();
        total.accumulate(AllocationSnapshot {
            allocations: 2,
            deallocations: 1,
            bytes_allocated: 64,
            live_bytes: 32,
        });
        total.accumulate(AllocationSnapshot {
            allocations: 3,
            deallocations: 4,
            bytes_allocated: 128,
            live_bytes: -16,
        });

        assert_eq!(total.allocations, 5);
        assert_eq!(total.deallocations, 5);
        assert_eq!(total.bytes_allocated, 192);
        assert_eq!(total.live_bytes, 16);
    }
}

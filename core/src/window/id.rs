//! TAIM-generated window identities with explicit exhaustion semantics.
//!
//! Identities are process-local, nonzero, and never wrap to a previously
//! issued value. This matters for retained window caches and frame gates:
//! an ABA collision would alias an unrelated window's live state.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// An opaque, process-unique identity for a managed window.
///
/// A window's identity is stable for its lifetime. Zero is reserved and
/// cannot be produced by the allocator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Id(u64);

/// Number of identities issued, not the next identity. Starts at zero so
/// every value in 1..=u64::MAX is usable exactly once.
static ISSUED: AtomicU64 = AtomicU64::new(0);

impl Id {
    /// Issue a new process-unique window identity.
    ///
    /// Panics rather than issuing a duplicate if the full u64 identity
    /// space is exhausted. Ordinary applications cannot approach this limit.
    #[must_use]
    pub fn unique() -> Self {
        Self::allocate(&ISSUED).expect("TAIM window identity space exhausted")
    }

    /// Numeric identity for diagnostics and stable process-local indexing.
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    fn allocate(counter: &AtomicU64) -> Option<Self> {
        let previous = counter
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| current.checked_add(1))
            .ok()?;
        Some(Self(previous + 1))
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::{Arc, Barrier};

    #[test]
    fn nonzero_and_exhaustion_are_explicit() {
        let counter = AtomicU64::new(0);
        let first = Id::allocate(&counter).expect("first ID");
        assert_eq!(first.as_u64(), 1);
        assert_eq!(first.to_string(), "1");
        counter.store(u64::MAX - 1, Ordering::Relaxed);
        assert_eq!(Id::allocate(&counter).expect("last ID").as_u64(), u64::MAX);
        assert!(Id::allocate(&counter).is_none());
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }

    #[test]
    fn concurrent_window_creation_never_aliases_identities() {
        const THREADS: usize = 8;
        const PER_THREAD: usize = 512;
        let counter = Arc::new(AtomicU64::new(0));
        let barrier = Arc::new(Barrier::new(THREADS));
        let mut workers = Vec::new();

        for _ in 0..THREADS {
            let counter = counter.clone();
            let barrier = barrier.clone();
            workers.push(std::thread::spawn(move || {
                let _ = barrier.wait();
                (0..PER_THREAD)
                    .map(|_| Id::allocate(&counter).expect("no overflow"))
                    .collect::<Vec<_>>()
            }));
        }

        let mut identities = HashSet::new();
        for worker in workers {
            for id in worker.join().expect("thread joined") {
                assert!(identities.insert(id), "window identity reused");
            }
        }
        assert_eq!(identities.len(), THREADS * PER_THREAD);
        assert_eq!(counter.load(Ordering::Relaxed), (THREADS * PER_THREAD) as u64);
    }
}

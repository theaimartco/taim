//! Instrumented TAIM executor using a native futures worker pool.
//!
//! Unlike the historical type alias, this adapter retains stable counters
//! for queued/running tasks and accounts for cancellation through a
//! drop guard. Metrics are lightweight atomics, not a synchronized snapshot.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Read-only native executor counters sampled without taking a global lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Telemetry {
    /// Tasks submitted through the TAIM worker adapter.
    pub submitted: u64,
    /// Tasks not yet terminated (queued, running or pending).
    pub active: usize,
    /// Tasks completed or cancelled since executor creation.
    pub terminated: u64,
}

#[derive(Default)]
struct Counters {
    submitted: AtomicU64,
    active: AtomicUsize,
    terminated: AtomicU64,
}

/// TAIM's native worker backend with cancellation-safe task accounting.
///
/// Execution is delegated to `futures::executor::ThreadPool`, but lifecycle
/// metrics belong to TAIM. It is not a custom thread scheduler; it supplies
/// a measurable task boundary until TAIM-native scheduling is proven.
#[derive(Clone)]
pub struct Executor {
    pool: futures::executor::ThreadPool,
    counters: Arc<Counters>,
}

impl fmt::Debug for Executor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TaimWorkerPool")
            .field("telemetry", &self.telemetry())
            .finish()
    }
}

struct InFlight(Arc<Counters>);

impl Drop for InFlight {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::AcqRel);
        self.0.terminated.fetch_add(1, Ordering::Relaxed);
    }
}

impl Executor {
    /// Read approximate task counters; these can change between atomics.
    #[inline]
    pub fn telemetry(&self) -> Telemetry {
        Telemetry {
            submitted: self.counters.submitted.load(Ordering::Relaxed),
            active: self.counters.active.load(Ordering::Acquire),
            terminated: self.counters.terminated.load(Ordering::Relaxed),
        }
    }
}

impl crate::Executor for Executor {
    fn new() -> Result<Self, futures::io::Error> {
        Ok(Self {
            pool: futures::executor::ThreadPool::new()?,
            counters: Arc::new(Counters::default()),
        })
    }

    fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        self.counters.submitted.fetch_add(1, Ordering::Relaxed);
        self.counters.active.fetch_add(1, Ordering::Release);

        // Construct the guard before scheduling. It is then owned by the
        // submitted future even if the pool cancels it before first poll.
        // Building it inside the async body would leak the active count for
        // never-polled tasks.
        let guard = InFlight(Arc::clone(&self.counters));
        self.pool.spawn_ok(async move {
            let _guard = guard;
            future.await;
        });
    }

    fn block_on<T>(&self, future: impl Future<Output = T>) -> T {
        futures::executor::block_on(future)
    }
}

/// Native thread workers do not expose an independent time driver;
/// applications needing periodic tasks use subscriptions with a timer backend.
pub mod time {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_start_at_zero_and_debug_is_observable() {
        let pool = <Executor as crate::Executor>::new().expect("thread pool available");
        assert_eq!(
            pool.telemetry(),
            Telemetry {
                submitted: 0,
                active: 0,
                terminated: 0,
            }
        );
        assert!(format!("{pool:?}").contains("TaimWorkerPool"));
    }

    #[test]
    fn completion_guard_decrements_active_even_on_cancellation() {
        let counters = Arc::new(Counters::default());
        counters.active.store(1, Ordering::Release);
        {
            let _guard = InFlight(Arc::clone(&counters));
            assert_eq!(counters.active.load(Ordering::Acquire), 1);
        }
        assert_eq!(counters.active.load(Ordering::Acquire), 0);
        assert_eq!(counters.terminated.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn never_polled_task_releases_counter_when_cancelled() {
        let counters = Arc::new(Counters::default());
        counters.active.store(1, Ordering::Release);

        // This future is never scheduled or polled: cancelling it still
        // drops the preconstructed guard and releases active telemetry.
        let guard = InFlight(Arc::clone(&counters));
        let task = async move {
            let _guard = guard;
            futures::future::pending::<()>().await;
        };
        drop(task);

        assert_eq!(counters.active.load(Ordering::Acquire), 0);
        assert_eq!(counters.terminated.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn spawned_tasks_complete_and_return_messages() {
        let pool = <Executor as crate::Executor>::new().expect("thread pool available");
        let (tx, rx) = futures::channel::oneshot::channel();
        <Executor as crate::Executor>::spawn(&pool, async move {
            let _ = tx.send(42_u32);
        });

        let received = futures::executor::block_on(rx).expect("task should finish");
        assert_eq!(received, 42);
        assert_eq!(pool.telemetry().submitted, 1);
    }
}

//! Choose your preferred executor to power a runtime.
use crate::MaybeSend;

/// A type that can run futures.
pub trait Executor: Sized {
    /// Creates a new [`Executor`].
    fn new() -> Result<Self, futures::io::Error>
    where
        Self: Sized;

    /// Spawns a future in the [`Executor`].
    fn spawn(&self, future: impl Future<Output = ()> + MaybeSend + 'static);

    /// Runs a future to completion in the current thread within the [`Executor`].
    #[cfg(not(target_arch = "wasm32"))]
    fn block_on<T>(&self, future: impl Future<Output = T>) -> T;

    /// Runs the given closure inside the [`Executor`].
    ///
    /// Some executors, like `tokio`, require some global state to be in place
    /// before creating futures. This method can be leveraged to set up this
    /// global state, call a function, restore the state, and obtain the result
    /// of the call.
    fn enter<R>(&self, f: impl FnOnce() -> R) -> R {
        f()
    }

    /// Skip background work when its owner is hidden, suspended or disabled.
    /// New work only: previously spawned futures are not cancelled.
    fn spawn_when(
        &self,
        enabled: bool,
        future: impl Future<Output = ()> + MaybeSend + 'static,
    ) -> bool {
        if enabled {
            self.spawn(future);
            true
        } else {
            false
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod taim_executor_contract_tests {
    use super::*;
    use std::cell::Cell;

    struct RecordingExecutor(Cell<usize>);
    impl Executor for RecordingExecutor {
        fn new() -> Result<Self, futures::io::Error> {
            Ok(Self(Cell::new(0)))
        }
        fn spawn(&self, _future: impl Future<Output = ()> + MaybeSend + 'static) {
            self.0.set(self.0.get() + 1);
        }
        fn block_on<T>(&self, future: impl Future<Output = T>) -> T {
            futures::executor::block_on(future)
        }
    }

    #[test]
    fn disabled_optional_work_does_not_enter_worker_pool() {
        let exec = RecordingExecutor::new().unwrap();
        assert!(!exec.spawn_when(false, async {}));
        assert_eq!(exec.0.get(), 0);
        assert!(exec.spawn_when(true, async {}));
        assert_eq!(exec.0.get(), 1);
    }
}

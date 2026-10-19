//! Deliberately disabled TAIM async backend.
//!
//! Explicitly opting into this backend discards spawned futures.
//! Production default selection must not choose it silently; see
//! backend::default for the fail-fast missing-backend configuration.
use crate::MaybeSend;

/// An executor that drops all the futures, instead of spawning them.
#[derive(Debug)]
pub struct Executor;

impl crate::Executor for Executor {
    fn new() -> Result<Self, futures::io::Error> {
        // Explicit null executors are useful for task-scheduler unit tests
        // that never delegate work to the background pool. Rejecting a
        // *default* missing executor is handled in backend::default.
        Ok(Self)
    }

    fn spawn(&self, _future: impl Future<Output = ()> + MaybeSend + 'static) {}

    #[cfg(not(target_arch = "wasm32"))]
    fn block_on<T>(&self, _future: impl Future<Output = T>) -> T {
        panic!("TAIM null backend has no block_on; configure a supported executor")
    }
}

pub mod time {
    //! Listen and react to time.
}

#[cfg(test)]
mod taim_disabled_executor_tests {
    use super::*;

    #[test]
    fn explicit_null_backend_is_constructible_for_scheduler_only_tests() {
        let executor = <Executor as crate::Executor>::new();
        assert!(executor.is_ok());
    }
}

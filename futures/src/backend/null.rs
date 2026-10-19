//! Deliberately disabled TAIM async backend.
//!
//! A runtime without an executor must fail initialization explicitly,
//! rather than silently dropping all application tasks and subscriptions.
use crate::MaybeSend;

/// An executor that drops all the futures, instead of spawning them.
#[derive(Debug)]
pub struct Executor;

impl crate::Executor for Executor {
    fn new() -> Result<Self, futures::io::Error> {
        Err(futures::io::Error::new(
            futures::io::ErrorKind::Unsupported,
            "TAIM requires an async executor: enable smol, tokio or thread-pool",
        ))
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
    fn refuses_silent_default_task_loss() {
        let error = <Executor as crate::Executor>::new().unwrap_err();
        assert_eq!(error.kind(), futures::io::ErrorKind::Unsupported);
    }
}

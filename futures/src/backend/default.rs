//! A default, cross-platform backend.
//!
//! - On native platforms, it will use:
//!   - `backend::native::tokio` when the `tokio` feature is enabled.
//!   - `backend::native::smol` when the `smol` feature is enabled.
//!   - `backend::native::thread_pool` when the `thread-pool` feature is enabled.
//!   - fail-fast missing-backend error when no native executor is enabled.
//!
//! - On Wasm, it will use `backend::wasm::wasm_bindgen`.
#[cfg(not(target_arch = "wasm32"))]
mod platform {
    #[cfg(feature = "tokio")]
    pub use crate::backend::native::tokio::*;

    #[cfg(all(feature = "smol", not(feature = "tokio"),))]
    pub use crate::backend::native::smol::*;

    #[cfg(all(feature = "thread-pool", not(any(feature = "tokio", feature = "smol"))))]
    pub use crate::backend::native::thread_pool::*;

    // The *explicit* null backend is permitted for scheduler-only tests.
    // A default application without an async executor is a configuration
    // error: silently dropping task messages would make apps appear hung.
    #[cfg(not(any(feature = "tokio", feature = "smol", feature = "thread-pool")))]
    mod missing {
        #[derive(Debug)]
        pub struct Executor;

        impl crate::Executor for Executor {
            fn new() -> Result<Self, futures::io::Error> {
                Err(futures::io::Error::new(
                    futures::io::ErrorKind::Unsupported,
                    "No TAIM default async backend selected (enable thread-pool, smol, or tokio)",
                ))
            }

            fn spawn(&self, _future: impl Future<Output = ()> + crate::MaybeSend + 'static) {
                unreachable!("unavailable default executor cannot be initialized");
            }

            fn block_on<T>(&self, _future: impl Future<Output = T>) -> T {
                unreachable!("unavailable default executor cannot be initialized");
            }
        }
    }

    #[cfg(not(any(feature = "tokio", feature = "smol", feature = "thread-pool")))]
    pub use missing::Executor;

    #[cfg(not(any(feature = "tokio", feature = "smol", feature = "thread-pool")))]
    pub use crate::backend::null::time;
}

#[cfg(target_arch = "wasm32")]
mod platform {
    pub use crate::backend::wasm::wasm_bindgen::*;
}

pub use platform::*;

#[cfg(all(
    test,
    not(target_arch = "wasm32"),
    not(any(feature = "tokio", feature = "smol", feature = "thread-pool"))
))]
mod tests {
    #[test]
    fn production_default_never_discards_subscriptions_silently() {
        let error = <super::Executor as crate::Executor>::new().unwrap_err();
        assert_eq!(error.kind(), futures::io::ErrorKind::Unsupported);
        assert!(error.to_string().contains("No TAIM default async backend"));
    }

    #[test]
    fn intentional_null_is_still_valid_for_headless_task_scheduler_tests() {
        assert!(<crate::backend::null::Executor as crate::Executor>::new().is_ok());
    }
}

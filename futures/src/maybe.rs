#[cfg(not(target_arch = "wasm32"))]
mod platform {
    /// An extension trait that enforces `Send` only on native platforms.
    ///
    /// Useful for writing cross-platform async code!
    pub trait MaybeSend: Send {}

    impl<T> MaybeSend for T where T: Send {}

    /// An extension trait that enforces `Sync` only on native platforms.
    ///
    /// Useful for writing cross-platform async code!
    pub trait MaybeSync: Sync {}

    impl<T> MaybeSync for T where T: Sync {}
}

#[cfg(target_arch = "wasm32")]
mod platform {
    /// An extension trait that enforces `Send` only on native platforms.
    ///
    /// Useful for writing cross-platform async code!
    pub trait MaybeSend {}

    impl<T> MaybeSend for T {}

    /// An extension trait that enforces `Sync` only on native platforms.
    ///
    /// Useful for writing cross-platform async code!
    pub trait MaybeSync {}

    impl<T> MaybeSync for T {}
}

pub use platform::{MaybeSend, MaybeSync};

/// Whether this compilation target requires Send for asynchronous task
/// handoff. Browser-local futures are not automatically transferable.
pub const fn requires_send_for_spawn() -> bool {
    cfg!(not(target_arch = "wasm32"))
}

#[cfg(test)]
mod taim_async_contract_tests {
    use super::*;

    #[test]
    fn task_thread_transfer_matches_target_marker_constraints() {
        assert_eq!(
            requires_send_for_spawn(),
            cfg!(not(target_arch = "wasm32"))
        );
    }
}

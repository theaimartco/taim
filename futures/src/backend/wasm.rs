//! Backends that are only available on Wasm targets.
pub mod wasm_bindgen;

/// TAIM's wasm-bindgen executor schedules local, single-threaded future
/// continuations; callers must not assume Send/Sync for browser UI state.
pub const fn is_local_executor() -> bool {
    true
}

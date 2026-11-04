//! Lightweight instrumentation hooks for TAIM applications.
pub use taim_core as core;
pub use taim_futures as futures;

use crate::core::theme::palette;
use crate::core::window;
use crate::futures::Subscription;
use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Application information exposed to an attached debugger.
#[derive(Debug, Clone, Copy)]
pub struct Metadata {
    /// Static application name.
    pub name: &'static str,
    /// Initial semantic theme seed.
    pub theme: Option<palette::Seed>,
    /// Whether the application supports message replay.
    pub can_time_travel: bool,
}

/// Render primitive category used in instrumentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Primitive {
    /// Rectangle or rounded rectangle.
    Quad,
    /// Indexed triangle mesh.
    Triangle,
    /// Application-authored shader.
    Shader,
    /// Raster or vector image.
    Image,
    /// Shaped text run.
    Text,
}

/// Command emitted by a debugger transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Replay application state through the indexed message.
    RewindTo {
        /// Zero-based message index.
        message: usize,
    },
    /// Leave replay and resume live input.
    GoLive,
}

/// RAII-compatible instrumentation span.
///
/// The built-in implementation is intentionally zero-allocation. A future
/// transport can consume these hooks without changing runtime call sites.
#[derive(Debug, Clone, Copy, Default)]
#[must_use = "finish or drop the span after the measured work"]
pub struct Span;

impl Span {
    #[inline]
    fn current() -> Self {
        Self
    }

    /// Completes the span explicitly.
    #[inline]
    pub fn finish(self) {}

    /// Returns whether this span was created while instrumentation was active.
    #[inline]
    pub fn is_active(self) -> bool {
        let _ = self;
        is_enabled()
    }
}

/// Enables instrumentation hooks.
#[inline]
pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}

/// Disables instrumentation hooks.
#[inline]
pub fn disable() {
    ENABLED.store(false, Ordering::Relaxed);
}

/// Returns whether instrumentation is enabled.
#[inline]
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Initializes debug and hot-reload state.
pub fn init(metadata: Metadata) {
    let _ = metadata;
    hot_reload::init();
}

/// Requests debugger shutdown, returning whether a transport handled it.
pub fn quit() -> bool {
    false
}

/// Lazily reports a theme change when instrumentation is active.
pub fn theme_changed(_theme: impl FnOnce() -> Option<palette::Seed>) {}

/// Reports the number of newly spawned tasks.
#[inline]
pub fn tasks_spawned(_amount: usize) {}

/// Reports the number of tracked subscriptions.
#[inline]
pub fn subscriptions_tracked(_amount: usize) {}

/// Lazily reports rendered layers without evaluating the count when disabled.
pub fn layers_rendered(_amount: impl FnOnce() -> usize) {}

#[inline]
fn stage() -> Span {
    Span::current()
}

/// Begins boot instrumentation.
pub fn boot() -> Span {
    stage()
}

/// Begins update instrumentation.
pub fn update(_message: &impl std::fmt::Debug) -> Span {
    stage()
}

/// Begins view construction instrumentation.
pub fn view(_window: window::Id) -> Span {
    stage()
}

/// Begins layout instrumentation.
pub fn layout(_window: window::Id) -> Span {
    stage()
}

/// Begins interaction instrumentation.
pub fn interact(_window: window::Id) -> Span {
    stage()
}

/// Begins draw-list instrumentation.
pub fn draw(_window: window::Id) -> Span {
    stage()
}

/// Begins primitive preparation instrumentation.
pub fn prepare(_primitive: Primitive) -> Span {
    stage()
}

/// Begins primitive rendering instrumentation.
pub fn render(_primitive: Primitive) -> Span {
    stage()
}

/// Begins presentation instrumentation.
pub fn present(_window: window::Id) -> Span {
    stage()
}

/// Begins a named custom span.
pub fn time(name: impl Into<String>) -> Span {
    let _ = name;
    stage()
}

/// Executes a closure inside a named custom span.
pub fn time_with<T>(name: impl Into<String>, work: impl FnOnce() -> T) -> T {
    let span = time(name);
    let output = work();
    span.finish();
    output
}

/// Returns the debugger command subscription.
pub fn commands() -> Subscription<Command> {
    Subscription::none()
}

/// Runs a closure through the active hot-reload adapter.
pub fn hot<O>(work: impl FnOnce() -> O) -> O {
    hot_reload::call(work)
}

/// Registers a callback for successful hot patches.
pub fn on_hotpatch(callback: impl Fn() + Send + Sync + 'static) {
    hot_reload::on_hotpatch(callback);
}

/// Returns whether a registered hot function needs rebuilding.
pub fn is_stale() -> bool {
    hot_reload::is_stale()
}

#[cfg(all(feature = "hot", debug_assertions, not(target_arch = "wasm32")))]
mod hot_transport;

#[cfg(all(feature = "hot", debug_assertions, not(target_arch = "wasm32")))]
mod hot_reload {
    use std::collections::BTreeSet;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};

    static STALE: AtomicBool = AtomicBool::new(false);
    static PENDING: Mutex<BTreeSet<u64>> = Mutex::new(BTreeSet::new());
    static REGISTERED: OnceLock<BTreeSet<u64>> = OnceLock::new();

    pub fn init() {
        super::hot_transport::connect();
        subsecond::register_handler(Arc::new(|| {
            if REGISTERED.get().is_none() {
                let functions = std::mem::take(&mut *PENDING.lock().expect("lock hot functions"));
                let _ = REGISTERED.set(functions);
            }
            STALE.store(false, Ordering::Release);
        }));
    }

    pub fn call<O>(work: impl FnOnce() -> O) -> O {
        let mut work = Some(work);
        let mut function = subsecond::HotFn::current(move || {
            work.take().expect("hot function already consumed")()
        });
        let address = function.ptr_address().0;

        if let Some(registered) = REGISTERED.get() {
            if registered.contains(&address) {
                STALE.store(true, Ordering::Release);
            }
        } else {
            let _ = PENDING.lock().expect("lock hot functions").insert(address);
        }

        function.call(())
    }

    pub fn on_hotpatch(callback: impl Fn() + Send + Sync + 'static) {
        subsecond::register_handler(Arc::new(callback));
    }

    pub fn is_stale() -> bool {
        STALE.load(Ordering::Acquire)
    }
}

#[cfg(any(not(feature = "hot"), not(debug_assertions), target_arch = "wasm32"))]
mod hot_reload {
    pub fn init() {}

    pub fn call<O>(work: impl FnOnce() -> O) -> O {
        work()
    }

    pub fn on_hotpatch(_callback: impl Fn() + Send + Sync + 'static) {}

    pub fn is_stale() -> bool {
        false
    }
}

#[cfg(all(test, feature = "hot", not(debug_assertions)))]
mod release_tests {
    #[test]
    fn enabling_hot_feature_in_release_keeps_only_the_local_call_path() {
        let mut calls = 0;
        assert_eq!(
            super::hot(|| {
                calls += 1;
                42
            }),
            42
        );
        assert_eq!(calls, 1);
        super::hot_reload::init();
        super::on_hotpatch(|| panic!("release must not register a native patch callback"));
        assert!(!super::is_stale());
    }
}

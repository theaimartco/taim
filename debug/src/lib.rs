pub use aura_core as core;
pub use aura_futures as futures;

use crate::core::theme::palette;
use crate::core::window;
use crate::futures::Subscription;

pub use internal::Span;

#[derive(Debug, Clone, Copy)]
pub struct Metadata {
    pub name: &'static str,
    pub theme: Option<palette::Seed>,
    pub can_time_travel: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum Primitive {
    Quad,
    Triangle,
    Shader,
    Image,
    Text,
}

#[derive(Debug, Clone, Copy)]
pub enum Command {
    RewindTo { message: usize },
    GoLive,
}

pub fn enable() {
    internal::enable();
}

pub fn disable() {
    internal::disable();
}

pub fn init(metadata: Metadata) {
    internal::init(metadata);
    hot::init();
}

pub fn quit() -> bool {
    internal::quit()
}

pub fn theme_changed(f: impl FnOnce() -> Option<palette::Seed>) {
    internal::theme_changed(f);
}

pub fn tasks_spawned(amount: usize) {
    internal::tasks_spawned(amount);
}

pub fn subscriptions_tracked(amount: usize) {
    internal::subscriptions_tracked(amount);
}

pub fn layers_rendered(amount: impl FnOnce() -> usize) {
    internal::layers_rendered(amount);
}

pub fn boot() -> Span {
    internal::boot()
}

pub fn update(message: &impl std::fmt::Debug) -> Span {
    internal::update(message)
}

pub fn view(window: window::Id) -> Span {
    internal::view(window)
}

pub fn layout(window: window::Id) -> Span {
    internal::layout(window)
}

pub fn interact(window: window::Id) -> Span {
    internal::interact(window)
}

pub fn draw(window: window::Id) -> Span {
    internal::draw(window)
}

pub fn prepare(primitive: Primitive) -> Span {
    internal::prepare(primitive)
}

pub fn render(primitive: Primitive) -> Span {
    internal::render(primitive)
}

pub fn present(window: window::Id) -> Span {
    internal::present(window)
}

pub fn time(name: impl Into<String>) -> Span {
    internal::time(name)
}

pub fn time_with<T>(name: impl Into<String>, f: impl FnOnce() -> T) -> T {
    let span = time(name);
    let result = f();
    span.finish();

    result
}

pub fn commands() -> Subscription<Command> {
    internal::commands()
}

pub fn hot<O>(f: impl FnOnce() -> O) -> O {
    hot::call(f)
}

pub fn on_hotpatch(f: impl Fn() + Send + Sync + 'static) {
    hot::on_hotpatch(f)
}

pub fn is_stale() -> bool {
    hot::is_stale()
}

mod internal {
    use crate::core::theme::palette;
    use crate::core::window;
    use crate::futures::Subscription;
    use crate::{Command, Metadata, Primitive};

    pub fn enable() {}
    pub fn disable() {}

    pub fn init(_metadata: Metadata) {}

    pub fn quit() -> bool {
        false
    }

    pub fn theme_changed(_f: impl FnOnce() -> Option<palette::Seed>) {}

    pub fn tasks_spawned(_amount: usize) {}

    pub fn subscriptions_tracked(_amount: usize) {}

    pub fn layers_rendered(_amount: impl FnOnce() -> usize) {}

    pub fn boot() -> Span {
        Span
    }

    pub fn update(_message: &impl std::fmt::Debug) -> Span {
        Span
    }

    pub fn view(_window: window::Id) -> Span {
        Span
    }

    pub fn layout(_window: window::Id) -> Span {
        Span
    }

    pub fn interact(_window: window::Id) -> Span {
        Span
    }

    pub fn draw(_window: window::Id) -> Span {
        Span
    }

    pub fn prepare(_primitive: Primitive) -> Span {
        Span
    }

    pub fn render(_primitive: Primitive) -> Span {
        Span
    }

    pub fn present(_window: window::Id) -> Span {
        Span
    }

    pub fn time(_name: impl Into<String>) -> Span {
        Span
    }

    pub fn commands() -> Subscription<Command> {
        Subscription::none()
    }

    #[derive(Debug)]
    pub struct Span;

    impl Span {
        pub fn finish(self) {}
    }
}

#[cfg(all(feature = "hot", not(target_arch = "wasm32")))]
mod hot {
    use std::collections::BTreeSet;
    use std::sync::atomic::{self, AtomicBool};
    use std::sync::{Arc, Mutex, OnceLock};

    static IS_STALE: AtomicBool = AtomicBool::new(false);

    static HOT_FUNCTIONS_PENDING: Mutex<BTreeSet<u64>> = Mutex::new(BTreeSet::new());

    static HOT_FUNCTIONS: OnceLock<BTreeSet<u64>> = OnceLock::new();

    pub fn init() {
        cargo_hot::connect();

        cargo_hot::subsecond::register_handler(Arc::new(|| {
            if HOT_FUNCTIONS.get().is_none() {
                HOT_FUNCTIONS
                    .set(std::mem::take(
                        &mut HOT_FUNCTIONS_PENDING.lock().expect("Lock hot functions"),
                    ))
                    .expect("Set hot functions");
            }

            IS_STALE.store(false, atomic::Ordering::Relaxed);
        }));
    }

    pub fn call<O>(f: impl FnOnce() -> O) -> O {
        let mut f = Some(f);

        // The `move` here is important. Hotpatching will not work
        // otherwise.
        let mut f = cargo_hot::subsecond::HotFn::current(move || {
            f.take().expect("Hot function is stale")()
        });

        let address = f.ptr_address().0;

        if let Some(hot_functions) = HOT_FUNCTIONS.get() {
            if hot_functions.contains(&address) {
                IS_STALE.store(true, atomic::Ordering::Relaxed);
            }
        } else {
            let _ = HOT_FUNCTIONS_PENDING
                .lock()
                .expect("Lock hot functions")
                .insert(address);
        }

        f.call(())
    }

    pub fn on_hotpatch(f: impl Fn() + Send + Sync + 'static) {
        cargo_hot::subsecond::register_handler(Arc::new(f));
    }

    pub fn is_stale() -> bool {
        IS_STALE.load(atomic::Ordering::Relaxed)
    }
}

#[cfg(any(not(feature = "hot"), target_arch = "wasm32"))]
mod hot {
    pub fn init() {}

    pub fn call<O>(f: impl FnOnce() -> O) -> O {
        f()
    }

    pub fn on_hotpatch(_f: impl Fn()) {}

    pub fn is_stale() -> bool {
        false
    }
}

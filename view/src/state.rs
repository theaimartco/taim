//! Reactive state owned by a window, and view properties that follow it.
use taim_signal::{Memo, Signal, Subscription};

use std::any::Any;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

/// A cheap, copyable handle to a reactive value owned by a [`Scope`].
///
/// Reading it inside a reactive property (`text(move || …)`) makes the
/// property follow it; writing it from an event handler updates every view
/// that follows it. Using a handle after its scope is gone panics.
pub struct State<T> {
    slot: u32,
    generation: u32,
    _value: PhantomData<fn() -> T>,
}

impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for State<T> {}

impl<T> std::fmt::Debug for State<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "State({}#{})", self.slot, self.generation)
    }
}

impl<T: Send + Sync + 'static> State<T> {
    fn signal(self) -> Signal<T> {
        let slots = registry();
        let slot = slots
            .get(self.slot as usize)
            .filter(|slot| slot.generation == self.generation)
            .and_then(|slot| slot.value.as_ref())
            .expect("a State handle was used after its scope was dropped");

        slot.downcast_ref::<Signal<T>>()
            .expect("State handles are typed at creation")
            .clone()
    }

    /// Reads the value through `read`.
    pub fn with<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        self.signal().with(read)
    }

    /// Changes the value in place and notifies followers.
    pub fn update<R>(&self, change: impl FnOnce(&mut T) -> R) -> R {
        self.signal().update(change)
    }

    /// Replaces the value and notifies followers.
    pub fn replace(&self, value: T) -> T {
        self.signal().replace(value)
    }
}

impl<T: Clone + Send + Sync + 'static> State<T> {
    /// A copy of the value.
    pub fn get(&self) -> T {
        self.signal().get()
    }
}

impl<T: PartialEq + Send + Sync + 'static> State<T> {
    /// Sets the value, notifying followers only if it changed.
    pub fn set(&self, value: T) -> bool {
        self.signal().set(value)
    }
}

struct Slot {
    generation: u32,
    value: Option<Arc<dyn Any + Send + Sync>>,
}

static SLOTS: Mutex<Vec<Slot>> = Mutex::new(Vec::new());

fn registry() -> std::sync::MutexGuard<'static, Vec<Slot>> {
    SLOTS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Owns the state of one window and knows how to tell that window it
/// needs work.
pub struct Scope {
    slots: Vec<u32>,
    invalidation: Arc<Invalidation>,
}

impl Scope {
    pub(crate) fn new(invalidation: Arc<Invalidation>) -> Self {
        Self {
            slots: Vec::new(),
            invalidation,
        }
    }

    /// Creates a value owned by this scope.
    pub fn signal<T: Send + Sync + 'static>(&mut self, value: T) -> State<T> {
        let value: Arc<dyn Any + Send + Sync> = Arc::new(Signal::new(value));
        let mut slots = registry();

        let (slot, generation) = match slots.iter().position(|slot| slot.value.is_none()) {
            Some(free) => {
                let slot = &mut slots[free];
                slot.generation = slot.generation.wrapping_add(1);
                slot.value = Some(value);
                (free, slot.generation)
            }
            None => {
                slots.push(Slot {
                    generation: 0,
                    value: Some(value),
                });
                (slots.len() - 1, 0)
            }
        };

        let slot = u32::try_from(slot).expect("fewer than 4 billion live states");
        self.slots.push(slot);

        State {
            slot,
            generation,
            _value: PhantomData,
        }
    }

    /// Runs `future` in the background; see [`spawn`].
    pub fn spawn(&self, future: impl std::future::Future<Output = ()> + Send + 'static) {
        spawn(future);
    }

    /// Runs `build` with this scope as the one reactive properties report
    /// to.
    pub(crate) fn build<R>(&mut self, build: impl FnOnce(&mut Self) -> R) -> R {
        let previous = BUILDING.with(|current| current.replace(Some(self.invalidation.clone())));
        let result = build(self);
        BUILDING.with(|current| *current.borrow_mut() = previous);
        result
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        let mut slots = registry();

        for slot in self.slots.drain(..) {
            if let Some(slot) = slots.get_mut(slot as usize) {
                slot.value = None;
            }
        }
    }
}

impl std::fmt::Debug for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scope")
            .field("states", &self.slots.len())
            .finish_non_exhaustive()
    }
}

thread_local! {
    static BUILDING: RefCell<Option<Arc<Invalidation>>> = const { RefCell::new(None) };
}

/// Runs `future` in the background. It may read and write state handles;
/// views following that state update, and their window wakes, when it does.
pub fn spawn(future: impl std::future::Future<Output = ()> + Send + 'static) {
    let spawned = std::thread::Builder::new()
        .name("taim-task".into())
        .spawn(move || futures::executor::block_on(future));

    if let Err(error) = spawned {
        log::error!("could not start a background task: {error}");
    }
}

/// The window currently being built, if any.
pub(crate) fn building() -> Option<Arc<Invalidation>> {
    BUILDING.with(|current| current.borrow().clone())
}

/// Runs `build` as if `window` were being built, so reactive properties
/// created inside report to it. Used by views that rebuild subtrees later.
pub(crate) fn rebuilding<R>(window: Option<&Arc<Invalidation>>, build: impl FnOnce() -> R) -> R {
    let previous = BUILDING.with(|current| current.replace(window.cloned()));
    let result = build();
    BUILDING.with(|current| *current.borrow_mut() = previous);
    result
}

/// Work a window owes because state changed, plus how to wake it.
#[derive(Default)]
pub struct Invalidation {
    pending: AtomicU8,
    wake: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
}

impl Invalidation {
    pub(crate) const PAINT: u8 = 1 << 0;
    pub(crate) const LAYOUT: u8 = 1 << 1;

    /// Records `work` and wakes the window if it was idle.
    pub(crate) fn mark(&self, work: u8) {
        if self.pending.fetch_or(work, Ordering::AcqRel) == 0
            && let Some(wake) = &*self.wake.lock().unwrap_or_else(PoisonError::into_inner)
        {
            wake();
        }
    }

    /// Takes the recorded work.
    pub(crate) fn take(&self) -> u8 {
        self.pending.swap(0, Ordering::AcqRel)
    }

    pub(crate) fn set_waker(&self, wake: impl Fn() + Send + Sync + 'static) {
        *self.wake.lock().unwrap_or_else(PoisonError::into_inner) = Some(Box::new(wake));
    }
}

impl std::fmt::Debug for Invalidation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Invalidation")
            .field("pending", &self.pending.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

/// A view property: a plain value, or a closure the view re-reads whenever
/// the state it read changes.
pub struct Prop<T> {
    source: Source<T>,
}

enum Source<T> {
    Fixed(T),
    Live {
        memo: Memo<T>,
        _follow: Option<Subscription>,
    },
}

impl<T: Clone + Send + Sync + 'static> Prop<T> {
    /// A property that never changes.
    pub fn fixed(value: T) -> Self {
        Self {
            source: Source::Fixed(value),
        }
    }

    /// A property recomputed by `compute` whenever state it reads changes.
    /// A change asks the window for a new layout.
    pub fn live(compute: impl Fn() -> T + Send + 'static) -> Self {
        let memo = Memo::new(compute);
        let follow = BUILDING.with(|current| {
            current.borrow().clone().map(|invalidation| {
                memo.subscribe(move |_| invalidation.mark(Invalidation::LAYOUT))
            })
        });

        Self {
            source: Source::Live {
                memo,
                _follow: follow,
            },
        }
    }

    /// The current value.
    pub fn get(&self) -> T {
        match &self.source {
            Source::Fixed(value) => value.clone(),
            Source::Live { memo, .. } => memo.get(),
        }
    }
}

impl<T> std::fmt::Debug for Prop<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.source {
            Source::Fixed(_) => "Prop::Fixed",
            Source::Live { .. } => "Prop::Live",
        })
    }
}

/// Anything that can become a [`Prop`]: the value itself, or a closure
/// producing it.
pub trait IntoProp<T> {
    /// Converts `self`.
    fn into_prop(self) -> Prop<T>;
}

impl<T, F> IntoProp<T> for F
where
    T: Clone + Send + Sync + 'static,
    F: Fn() -> T + Send + 'static,
{
    fn into_prop(self) -> Prop<T> {
        Prop::live(self)
    }
}

/// A state handle used as a property follows the state.
impl<T: Clone + Send + Sync + 'static> IntoProp<T> for State<T> {
    fn into_prop(self) -> Prop<T> {
        Prop::live(move || self.get())
    }
}

impl IntoProp<f32> for f32 {
    fn into_prop(self) -> Prop<f32> {
        Prop::fixed(self)
    }
}

impl IntoProp<String> for &str {
    fn into_prop(self) -> Prop<String> {
        Prop::fixed(self.to_owned())
    }
}

impl IntoProp<String> for String {
    fn into_prop(self) -> Prop<String> {
        Prop::fixed(self)
    }
}

impl IntoProp<String> for &String {
    fn into_prop(self) -> Prop<String> {
        Prop::fixed(self.clone())
    }
}

impl IntoProp<bool> for bool {
    fn into_prop(self) -> Prop<bool> {
        Prop::fixed(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_read_and_write_the_same_value() {
        let mut scope = Scope::new(Arc::default());
        let count = scope.signal(1);
        let copy = count;

        copy.update(|n| *n += 1);
        assert_eq!(count.get(), 2);
        assert!(count.set(5));
        assert!(!count.set(5));
    }

    #[test]
    fn live_props_follow_state_and_invalidate_the_window() {
        let invalidation = Arc::new(Invalidation::default());
        let mut scope = Scope::new(invalidation.clone());
        let count = scope.signal(0);

        let label = scope.build(|_| Prop::live(move || format!("n = {}", count.get())));
        assert_eq!(label.get(), "n = 0");
        assert_eq!(invalidation.take(), 0);

        let _ = count.set(3);
        assert_eq!(invalidation.take(), Invalidation::LAYOUT);
        assert_eq!(label.get(), "n = 3");
    }

    #[test]
    fn dropping_the_scope_frees_its_slots_for_reuse() {
        let first = {
            let mut scope = Scope::new(Arc::default());
            scope.signal(String::from("a"))
        };
        let mut scope = Scope::new(Arc::default());
        let second = scope.signal(String::from("b"));

        assert_eq!(second.get(), "b");
        assert!(std::panic::catch_unwind(|| first.get()).is_err());
    }

    #[test]
    fn plain_values_and_closures_both_convert() {
        assert_eq!("hi".into_prop().get(), "hi");
        assert!(true.into_prop().get());
        assert_eq!((|| 7_u8).into_prop().get(), 7);
    }
}

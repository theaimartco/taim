//! Reactive state owned by a window, and view properties that follow it.
use taim_signal::{Memo, Signal, Subscription};

use futures::future::{AbortHandle, Abortable};
use std::any::Any;
use std::cell::{Cell, RefCell};
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
    owner: Arc<StateOwner>,
    tasks: TaskScope,
    invalidation: Arc<Invalidation>,
    receivers: RefCell<Vec<Box<dyn FnMut() -> bool>>>,
    pub(crate) settings: Settings,
    pub(crate) restoration: crate::restoration::Restoration,
}

/// Window-wide settings an application may bind to state.
#[derive(Default)]
pub(crate) struct Settings {
    pub theme: Option<Prop<crate::Theme>>,
    pub scale: Option<Prop<f32>>,
    pub reduced_motion: Option<Prop<bool>>,
    pub back: Option<(Prop<bool>, Box<dyn Fn()>)>,
}

impl Scope {
    pub(crate) fn new(invalidation: Arc<Invalidation>) -> Self {
        let owner = Arc::new(StateOwner::default());
        Self {
            tasks: TaskScope {
                owner: Arc::downgrade(&owner),
                tasks: Arc::new(Mutex::new(Some(Vec::new()))),
            },
            owner,
            invalidation,
            receivers: RefCell::new(Vec::new()),
            settings: Settings::default(),
            restoration: crate::restoration::Restoration::default(),
        }
    }

    /// The window's theme; a closure or state handle follows state.
    pub fn theme(&mut self, theme: impl IntoProp<crate::Theme>) {
        self.settings.theme = Some(self.build(|_| theme.into_prop()));
    }

    /// Zoom applied to the whole window (1.0 is unscaled).
    pub fn scale(&mut self, scale: impl IntoProp<f32>) {
        self.settings.scale = Some(self.build(|_| scale.into_prop()));
    }

    /// Whether views should skip their transition animations.
    pub fn reduced_motion(&mut self, reduced: impl IntoProp<bool>) {
        self.settings.reduced_motion = Some(self.build(|_| reduced.into_prop()));
    }

    /// Registers ahead-of-time navigation state. While enabled Android can
    /// dispatch predictive back; the action runs only on gesture commit.
    pub fn on_back(&mut self, enabled: impl IntoProp<bool>, action: impl Fn() + 'static) {
        self.settings.back = Some((self.build(|_| enabled.into_prop()), Box::new(action)));
    }

    /// Creates explicitly persisted, versioned state. Missing, incompatible or
    /// malformed saved values use `initial`. Do not register password state.
    pub fn restorable<T>(
        &mut self,
        key: &str,
        version: u32,
        initial: T,
    ) -> Result<State<T>, crate::RestorationError>
    where
        T: Clone + serde::Serialize + serde::de::DeserializeOwned + Send + Sync + 'static,
    {
        let value = self.restoration.initial(key, version).unwrap_or(initial);
        let state = self.signal(value);
        self.restoration.register(key, version, state)?;
        Ok(state)
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
        self.owner
            .slots
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(slot);

        State {
            slot,
            generation,
            _value: PhantomData,
        }
    }

    /// Runs `future` in the background until it completes or this window
    /// closes. Cancellation is cooperative: a poll already executing can
    /// finish, with its state kept alive until it returns.
    pub fn spawn(&self, future: impl std::future::Future<Output = ()> + Send + 'static) {
        let _ = self.tasks.spawn(future);
    }

    /// A handle callbacks can use to start work owned by this window.
    pub fn tasks(&self) -> TaskScope {
        self.tasks.clone()
    }

    /// A bounded mailbox whose handler runs on the window thread when the
    /// shell processes state changes. The handler may own non-Send UI state.
    /// Senders can be used from background threads and disconnect on close.
    pub fn channel<T: Send + 'static>(
        &mut self,
        mut receive: impl FnMut(T) + 'static,
    ) -> Sender<T> {
        let (sender, receiver) = std::sync::mpsc::sync_channel(256);
        self.receivers.get_mut().push(Box::new(move || {
            let mut count = 0;
            while count < 256 {
                let Ok(message) = receiver.try_recv() else {
                    break;
                };
                receive(message);
                count += 1;
            }
            count == 256
        }));
        Sender {
            sender,
            invalidation: Arc::downgrade(&self.invalidation),
        }
    }

    pub(crate) fn receive(&self) -> bool {
        let mut more = false;
        for receive in self.receivers.borrow_mut().iter_mut() {
            more |= receive();
        }
        more
    }

    /// Runs `build` with this scope as the one reactive properties report
    /// to.
    pub(crate) fn build<R>(&mut self, build: impl FnOnce(&mut Self) -> R) -> R {
        let _restore = BuildingGuard::enter(Some(self.invalidation.clone()));
        build(self)
    }
}

/// Sends work to a window's main-thread handler without blocking its caller.
pub struct Sender<T> {
    sender: std::sync::mpsc::SyncSender<T>,
    invalidation: std::sync::Weak<Invalidation>,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            invalidation: self.invalidation.clone(),
        }
    }
}

impl<T> std::fmt::Debug for Sender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sender").finish_non_exhaustive()
    }
}

impl<T> Sender<T> {
    /// Queues a message or returns it on overload/closure. A successful
    /// send wakes the event loop, including when the window is occluded.
    pub fn send(&self, message: T) -> Result<(), std::sync::mpsc::TrySendError<T>> {
        self.sender.try_send(message)?;
        if let Some(invalidation) = self.invalidation.upgrade() {
            invalidation.mark(Invalidation::LAYOUT);
        }
        Ok(())
    }
}

type TaskList = Option<Vec<(AbortHandle, std::sync::Weak<()>)>>;

/// Starts asynchronous work tied to a window's lifetime. Clones do not
/// keep the window alive; requests after it closes are rejected.
#[derive(Clone)]
pub struct TaskScope {
    owner: std::sync::Weak<StateOwner>,
    tasks: Arc<Mutex<TaskList>>,
}

impl TaskScope {
    /// Accepts a task while the window is alive, returning false after
    /// closure. Closing cancels pending futures and keeps their state
    /// valid until any poll already executing finishes.
    pub fn spawn(&self, future: impl std::future::Future<Output = ()> + Send + 'static) -> bool {
        let Some(owner) = self.owner.upgrade() else {
            return false;
        };
        let (abort, registration) = AbortHandle::new_pair();
        let live = Arc::new(());
        let mut registry = self.tasks.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(tasks) = registry.as_mut() else {
            return false;
        };
        tasks.retain(|(_, live)| live.strong_count() != 0);
        tasks.push((abort, Arc::downgrade(&live)));
        drop(registry);
        spawn(async move {
            let _ = Abortable::new(future, registration).await;
            drop(owner);
            drop(live);
        });
        true
    }
}

impl std::fmt::Debug for TaskScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskScope").finish_non_exhaustive()
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        let tasks = self
            .tasks
            .tasks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        for (task, _) in tasks.into_iter().flatten() {
            task.abort();
        }
    }
}

#[derive(Default)]
struct StateOwner {
    slots: Mutex<Vec<u32>>,
}

impl Drop for StateOwner {
    fn drop(&mut self) {
        let mut slots = registry();
        let mut released = Vec::new();
        for slot in self
            .slots
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .drain(..)
        {
            if let Some(slot) = slots.get_mut(slot as usize) {
                released.extend(slot.value.take());
            }
        }
        drop(slots);
        // User value destructors may read other State handles.
        drop(released);
    }
}

impl std::fmt::Debug for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scope")
            .field(
                "states",
                &self
                    .owner
                    .slots
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .len(),
            )
            .finish_non_exhaustive()
    }
}

thread_local! {
    static BUILDING: RefCell<Option<Arc<Invalidation>>> = const { RefCell::new(None) };
}

// Restore the enclosing scope even when application-supplied builders panic.
struct BuildingGuard(Option<Arc<Invalidation>>);

impl BuildingGuard {
    fn enter(window: Option<Arc<Invalidation>>) -> Self {
        Self(BUILDING.with(|current| current.replace(window)))
    }
}

impl Drop for BuildingGuard {
    fn drop(&mut self) {
        BUILDING.with(|current| *current.borrow_mut() = self.0.take());
    }
}

/// Runs `future` in the background. It may read and write state handles;
/// views following that state update, and their window wakes, when it does.
/// This task is detached. Use [`Scope::spawn`] for work that accesses a
/// window's state, so closing the window cancels it safely.
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
    let _restore = BuildingGuard::enter(window.cloned());
    build()
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

/// A copy reads the same value; only the original asks the window for
/// layout when it changes.
impl<T: Clone> Clone for Prop<T> {
    fn clone(&self) -> Self {
        Self {
            source: match &self.source {
                Source::Fixed(value) => Source::Fixed(value.clone()),
                Source::Live { memo, .. } => Source::Live {
                    memo: memo.clone(),
                    _follow: None,
                },
            },
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

/// Plain values of common types are fixed properties.
macro_rules! fixed_props {
    ($($value:ty),+ $(,)?) => {
        $(
            impl IntoProp<$value> for $value {
                fn into_prop(self) -> Prop<$value> {
                    Prop::fixed(self)
                }
            }
        )+
    };
}

fixed_props!(
    f32,
    f64,
    u8,
    u16,
    u32,
    u64,
    usize,
    i8,
    i16,
    i32,
    i64,
    isize,
    crate::Color
);

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

/// Whether transitions should be skipped, as set by the window's
/// [`Scope::reduced_motion`].
pub(crate) fn motion_reduced() -> bool {
    REDUCED_MOTION.with(Cell::get)
}

// Motion is window-local. Restore the caller when a custom view drives
// another root, including when an application callback unwinds.
pub(crate) struct MotionGuard(bool);

impl MotionGuard {
    pub(crate) fn enter(reduced: bool) -> Self {
        Self(REDUCED_MOTION.with(|current| current.replace(reduced)))
    }
}

impl Drop for MotionGuard {
    fn drop(&mut self) {
        REDUCED_MOTION.with(|current| current.set(self.0));
    }
}

/// Notes that a glass surface was painted this frame, so the shell knows
/// moving the scene light would change what is on screen.
pub(crate) fn note_glass() {
    GLASS_PAINTED.with(|painted| painted.set(true));
}

/// Clears the glass note, returning whether glass was painted since.
pub(crate) fn take_glass() -> bool {
    GLASS_PAINTED.with(|painted| painted.replace(false))
}

thread_local! {
    static GLASS_PAINTED: Cell<bool> = const { Cell::new(false) };
    static REDUCED_MOTION: Cell<bool> = const { Cell::new(false) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_motion_context_restores_the_window_after_unwinding() {
        assert!(!motion_reduced());
        let window = MotionGuard::enter(true);
        let result = std::panic::catch_unwind(|| {
            let _nested = MotionGuard::enter(false);
            assert!(!motion_reduced());
            panic!("application callback");
        });
        assert!(result.is_err());
        assert!(motion_reduced());
        drop(window);
        assert!(!motion_reduced());
    }

    #[test]
    fn motion_context_does_not_cross_ui_threads() {
        let _window = MotionGuard::enter(true);
        std::thread::spawn(|| {
            assert!(!motion_reduced());
            let _other = MotionGuard::enter(false);
            assert!(!motion_reduced());
        })
        .join()
        .expect("independent UI thread");
        assert!(motion_reduced());
    }

    #[test]
    fn mailbox_transfers_work_to_the_owner_thread_without_requiring_send_ui_state() {
        let mut scope = Scope::new(Arc::default());
        let value = std::rc::Rc::new(std::cell::Cell::new(0));
        let owner = std::thread::current().id();
        let sender = scope.channel({
            let value = value.clone();
            move |next| {
                assert_eq!(std::thread::current().id(), owner);
                value.set(next);
            }
        });
        std::thread::spawn(move || sender.send(42).unwrap())
            .join()
            .unwrap();
        assert_eq!(value.get(), 0);
        assert!(!scope.receive());
        assert_eq!(value.get(), 42);
    }

    #[test]
    fn mailbox_is_bounded_and_disconnects_when_its_window_closes() {
        use std::sync::mpsc::TrySendError;
        let mut scope = Scope::new(Arc::default());
        let sender = scope.channel(|_: usize| {});
        for value in 0..256 {
            sender.send(value).unwrap();
        }
        assert!(matches!(sender.send(256), Err(TrySendError::Full(256))));
        assert!(scope.receive());
        assert!(!scope.receive());
        drop(scope);
        assert!(matches!(
            sender.send(257),
            Err(TrySendError::Disconnected(257))
        ));
    }

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

    #[test]
    fn closing_a_scope_cancels_pending_tasks_and_rejects_new_work() {
        use std::sync::mpsc;
        use std::time::Duration;

        struct Dropped(mpsc::Sender<()>);
        impl Drop for Dropped {
            fn drop(&mut self) {
                let _ = self.0.send(());
            }
        }
        let scope = Scope::new(Arc::default());
        let tasks = scope.tasks();
        let (dropped, observed) = mpsc::channel();
        let guard = Dropped(dropped);
        assert!(tasks.spawn(async move {
            let _guard = guard;
            futures::future::pending::<()>().await;
        }));
        drop(scope);
        observed
            .recv_timeout(Duration::from_secs(5))
            .expect("canceled future is dropped");
        assert!(!tasks.spawn(async {}));
    }

    #[test]
    fn an_executing_task_keeps_state_alive_until_its_poll_returns() {
        use std::sync::mpsc;
        use std::time::Duration;
        let timeout = Duration::from_secs(5);
        let mut scope = Scope::new(Arc::default());
        let value = scope.signal(42);
        let (entered, started) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        let (result, received) = mpsc::channel();
        scope.spawn(async move {
            entered.send(()).unwrap();
            wait.recv_timeout(timeout).unwrap();
            // The window closes during this poll. Its state must remain
            // usable until this poll returns, even after cancellation.
            result.send(value.get()).unwrap();
        });
        started.recv_timeout(timeout).unwrap();
        drop(scope);
        release.send(()).unwrap();
        assert_eq!(received.recv_timeout(timeout).unwrap(), 42);
    }

    #[test]
    fn panicking_builders_restore_the_enclosing_reactive_context() {
        let outer = Arc::new(Invalidation::default());
        let inner = Arc::new(Invalidation::default());
        rebuilding(Some(&outer), || {
            let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                rebuilding(Some(&inner), || panic!("user builder failed"));
            }));
            assert!(failed.is_err());
            assert!(Arc::ptr_eq(&building().unwrap(), &outer));
        });
        assert!(building().is_none());
    }
}

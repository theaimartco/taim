//! Fine-grained, thread-safe reactive state for Taim.
//!
//! [`Signal`] owns a value and publishes changes. [`Memo`] lazily derives a
//! value from any signals or memos read while its computation runs. Dependency
//! tracking is automatic, including dynamic dependency sets, and subscriptions
//! are removed when their [`Subscription`] is dropped.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};

/// A monotonically increasing version of a reactive value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(u64);

impl Revision {
    /// The revision of a newly-created reactive value.
    pub const INITIAL: Self = Self(0);

    /// Returns the raw revision number.
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Stable identity of one reactive source for the lifetime of that source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceId(usize);

impl SourceId {
    /// Returns a process-local token suitable for routing an invalidation.
    ///
    /// The token is stable while the source is alive. It is deliberately not
    /// a persistent identifier and must never be serialized.
    pub const fn routing_token(self) -> u64 {
        self.0 as u64
    }
}

/// A cloneable reactive source that can be read by framework adapters.
pub trait Readable: Clone + Send + Sync + 'static {
    /// The value produced by this source.
    type Value: Clone + Send + Sync + 'static;

    /// Returns the stable identity of this source.
    fn source_id(&self) -> SourceId;

    /// Returns the revision currently visible to readers.
    fn revision(&self) -> Revision;

    /// Clones and returns the current value.
    fn get(&self) -> Self::Value;

    /// Subscribes to invalidation of this source.
    fn subscribe(&self, callback: impl Fn(Revision) + Send + Sync + 'static) -> Subscription;
}

type Callback = Arc<dyn Fn(Revision) + Send + Sync + 'static>;

struct Subscribers {
    next_id: AtomicU64,
    callbacks: Mutex<BTreeMap<u64, Callback>>,
}

impl Subscribers {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            next_id: AtomicU64::new(0),
            callbacks: Mutex::new(BTreeMap::new()),
        })
    }

    fn subscribe(self: &Arc<Self>, callback: Callback) -> Subscription {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let _ = self
            .callbacks
            .lock()
            .expect("reactive subscriber lock poisoned")
            .insert(id, callback);

        Subscription {
            subscribers: Arc::downgrade(self),
            id,
        }
    }

    fn notify(&self, revision: Revision) {
        let callbacks = self
            .callbacks
            .lock()
            .expect("reactive subscriber lock poisoned")
            .values()
            .cloned()
            .collect::<Vec<_>>();

        for callback in callbacks {
            callback(revision);
        }
    }
}

/// A live subscription to a reactive value.
///
/// Dropping this value disconnects its callback. A subscription therefore has
/// explicit ownership and cannot leak a listener accidentally.
#[must_use = "dropping a subscription immediately disconnects it"]
pub struct Subscription {
    subscribers: Weak<Subscribers>,
    id: u64,
}

impl std::fmt::Debug for Subscription {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Subscription")
            .field("id", &self.id)
            .finish()
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(subscribers) = self.subscribers.upgrade() {
            let _ = subscribers
                .callbacks
                .lock()
                .expect("reactive subscriber lock poisoned")
                .remove(&self.id);
        }
    }
}

trait Source: Send + Sync {
    fn id(&self) -> usize;
    fn revision(&self) -> Revision;
    fn subscribe_source(&self, callback: Callback) -> Subscription;
}

#[derive(Clone)]
struct Dependency {
    source: Arc<dyn Source>,
    observed: Revision,
}

thread_local! {
    static COLLECTORS: RefCell<Vec<Vec<Dependency>>> = const { RefCell::new(Vec::new()) };
}

fn track(source: Arc<dyn Source>) {
    COLLECTORS.with(|collectors| {
        let mut collectors = collectors.borrow_mut();
        let Some(active) = collectors.last_mut() else {
            return;
        };

        if active
            .iter()
            .all(|dependency| dependency.source.id() != source.id())
        {
            active.push(Dependency {
                observed: source.revision(),
                source,
            });
        }
    });
}

struct CollectorGuard;

impl Drop for CollectorGuard {
    fn drop(&mut self) {
        COLLECTORS.with(|collectors| {
            let _ = collectors.borrow_mut().pop();
        });
    }
}

fn capture<T>(compute: impl FnOnce() -> T) -> (T, Vec<Dependency>) {
    COLLECTORS.with(|collectors| collectors.borrow_mut().push(Vec::new()));
    let guard = CollectorGuard;
    let value = compute();
    let dependencies = COLLECTORS.with(|collectors| {
        collectors
            .borrow_mut()
            .pop()
            .expect("dependency collector must be balanced")
    });
    std::mem::forget(guard);

    (value, dependencies)
}

struct SignalInner<T> {
    value: RwLock<T>,
    revision: AtomicU64,
    subscribers: Arc<Subscribers>,
}

impl<T> SignalInner<T> {
    fn changed(&self) {
        let revision = Revision(self.revision.fetch_add(1, Ordering::AcqRel).wrapping_add(1));
        self.subscribers.notify(revision);
    }
}

impl<T: Send + Sync + 'static> Source for SignalInner<T> {
    fn id(&self) -> usize {
        std::ptr::from_ref(self).cast::<()>() as usize
    }

    fn revision(&self) -> Revision {
        Revision(self.revision.load(Ordering::Acquire))
    }

    fn subscribe_source(&self, callback: Callback) -> Subscription {
        self.subscribers.subscribe(callback)
    }
}

/// A cloneable, thread-safe reactive value.
///
/// Notifications run synchronously after the value lock has been released, so
/// a subscriber may safely read the signal again. Clones refer to the same
/// value and revision.
pub struct Signal<T> {
    inner: Arc<SignalInner<T>>,
}

impl<T> std::fmt::Debug for Signal<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Signal")
            .field("revision", &self.revision())
            .finish_non_exhaustive()
    }
}

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> Signal<T> {
    /// Creates a signal holding `value`.
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(SignalInner {
                value: RwLock::new(value),
                revision: AtomicU64::new(Revision::INITIAL.value()),
                subscribers: Subscribers::new(),
            }),
        }
    }

    /// Returns the current revision.
    pub fn revision(&self) -> Revision {
        Revision(self.inner.revision.load(Ordering::Acquire))
    }
}

impl<T: Send + Sync + 'static> Signal<T> {
    /// Reads the current value through `read` and tracks this signal when the
    /// call occurs inside a [`Memo`] computation.
    pub fn with<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        let source: Arc<dyn Source> = self.inner.clone();
        track(source);
        read(&self.inner.value.read().expect("signal value lock poisoned"))
    }

    /// Replaces the value and publishes one change notification.
    pub fn replace(&self, value: T) -> T {
        let old = {
            let mut current = self
                .inner
                .value
                .write()
                .expect("signal value lock poisoned");
            std::mem::replace(&mut *current, value)
        };
        self.inner.changed();
        old
    }

    /// Mutates the value and publishes one change notification after `update`
    /// returns.
    pub fn update<R>(&self, update: impl FnOnce(&mut T) -> R) -> R {
        let result = {
            let mut value = self
                .inner
                .value
                .write()
                .expect("signal value lock poisoned");
            update(&mut value)
        };
        self.inner.changed();
        result
    }

    /// Runs `callback` after every subsequent change.
    pub fn subscribe(&self, callback: impl Fn(Revision) + Send + Sync + 'static) -> Subscription {
        self.inner.subscribers.subscribe(Arc::new(callback))
    }
}

impl<T: Clone + Send + Sync + 'static> Signal<T> {
    /// Clones and returns the current value.
    pub fn get(&self) -> T {
        self.with(Clone::clone)
    }
}

impl<T: Clone + Send + Sync + 'static> Readable for Signal<T> {
    type Value = T;

    fn source_id(&self) -> SourceId {
        SourceId(Arc::as_ptr(&self.inner).cast::<()>() as usize)
    }

    fn revision(&self) -> Revision {
        self.revision()
    }

    fn get(&self) -> Self::Value {
        self.get()
    }

    fn subscribe(&self, callback: impl Fn(Revision) + Send + Sync + 'static) -> Subscription {
        Signal::subscribe(self, callback)
    }
}

impl<T: PartialEq + Send + Sync + 'static> Signal<T> {
    /// Replaces the value only when it differs from the current value.
    ///
    /// Returns whether a change was published.
    pub fn set(&self, value: T) -> bool {
        let changed = {
            let mut current = self
                .inner
                .value
                .write()
                .expect("signal value lock poisoned");
            if *current == value {
                false
            } else {
                *current = value;
                true
            }
        };

        if changed {
            self.inner.changed();
        }

        changed
    }
}

struct MemoInner<T> {
    value: RwLock<Option<T>>,
    compute: Mutex<Box<dyn Fn() -> T + Send + 'static>>,
    dirty: AtomicBool,
    revision: AtomicU64,
    subscribers: Arc<Subscribers>,
    dependencies: Mutex<Vec<Subscription>>,
    refreshing: Mutex<()>,
}

impl<T: Send + Sync + 'static> MemoInner<T> {
    fn invalidate(&self) {
        if !self.dirty.swap(true, Ordering::AcqRel) {
            let revision = Revision(self.revision.fetch_add(1, Ordering::AcqRel).wrapping_add(1));
            self.subscribers.notify(revision);
        }
    }

    fn refresh(self: &Arc<Self>) {
        if !self.dirty.load(Ordering::Acquire) {
            return;
        }

        let _refreshing = self.refreshing.lock().expect("memo refresh lock poisoned");
        if !self.dirty.load(Ordering::Acquire) {
            return;
        }

        let (value, dependencies) = capture(|| {
            let compute = self.compute.lock().expect("memo computation lock poisoned");
            compute()
        });

        let weak = Arc::downgrade(self);
        let subscriptions = dependencies
            .iter()
            .map(|dependency| {
                let weak = weak.clone();
                dependency.source.subscribe_source(Arc::new(move |_| {
                    if let Some(memo) = weak.upgrade() {
                        memo.invalidate();
                    }
                }))
            })
            .collect::<Vec<_>>();

        *self.value.write().expect("memo value lock poisoned") = Some(value);
        *self
            .dependencies
            .lock()
            .expect("memo dependency lock poisoned") = subscriptions;
        self.dirty.store(false, Ordering::Release);

        // A source can change between being read and being subscribed. Never
        // publish the computation as clean if that race occurred.
        if dependencies
            .iter()
            .any(|dependency| dependency.source.revision() != dependency.observed)
        {
            self.invalidate();
        }
    }
}

impl<T: Send + Sync + 'static> Source for MemoInner<T> {
    fn id(&self) -> usize {
        std::ptr::from_ref(self).cast::<()>() as usize
    }

    fn revision(&self) -> Revision {
        Revision(self.revision.load(Ordering::Acquire))
    }

    fn subscribe_source(&self, callback: Callback) -> Subscription {
        self.subscribers.subscribe(callback)
    }
}

/// A lazily-computed reactive value with automatically tracked dependencies.
///
/// The computation runs at first read and again only after one of the signals
/// or memos it read changes. Dependencies may change between runs.
pub struct Memo<T> {
    inner: Arc<MemoInner<T>>,
}

impl<T> Clone for Memo<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T: Send + Sync + 'static> Memo<T> {
    /// Creates a lazy memo from `compute`.
    pub fn new(compute: impl Fn() -> T + Send + 'static) -> Self {
        Self {
            inner: Arc::new(MemoInner {
                value: RwLock::new(None),
                compute: Mutex::new(Box::new(compute)),
                dirty: AtomicBool::new(true),
                revision: AtomicU64::new(Revision::INITIAL.value()),
                subscribers: Subscribers::new(),
                dependencies: Mutex::new(Vec::new()),
                refreshing: Mutex::new(()),
            }),
        }
    }

    /// Returns the current revision, which advances when this memo becomes
    /// invalidated.
    pub fn revision(&self) -> Revision {
        Revision(self.inner.revision.load(Ordering::Acquire))
    }

    /// Reads the current derived value, recomputing it first when stale.
    pub fn with<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        let source: Arc<dyn Source> = self.inner.clone();
        track(source);
        self.inner.refresh();

        let value = self.inner.value.read().expect("memo value lock poisoned");
        read(value.as_ref().expect("a refreshed memo must have a value"))
    }

    /// Runs `callback` whenever this memo becomes stale.
    pub fn subscribe(&self, callback: impl Fn(Revision) + Send + Sync + 'static) -> Subscription {
        self.inner.subscribers.subscribe(Arc::new(callback))
    }
}

impl<T: Clone + Send + Sync + 'static> Memo<T> {
    /// Clones and returns the current derived value.
    pub fn get(&self) -> T {
        self.with(Clone::clone)
    }
}

impl<T: Clone + Send + Sync + 'static> Readable for Memo<T> {
    type Value = T;

    fn source_id(&self) -> SourceId {
        SourceId(Arc::as_ptr(&self.inner).cast::<()>() as usize)
    }

    fn revision(&self) -> Revision {
        self.revision()
    }

    fn get(&self) -> Self::Value {
        self.get()
    }

    fn subscribe(&self, callback: impl Fn(Revision) + Send + Sync + 'static) -> Subscription {
        Memo::subscribe(self, callback)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn signal_notifies_only_for_real_set_changes() {
        let signal = Signal::new(1);
        let notifications = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&notifications);
        let _subscription = signal.subscribe(move |_| {
            let _ = observed.fetch_add(1, Ordering::Relaxed);
        });

        assert!(!signal.set(1));
        assert!(signal.set(2));
        assert_eq!(signal.get(), 2);
        assert_eq!(notifications.load(Ordering::Relaxed), 1);
        assert_eq!(signal.revision().value(), 1);
    }

    #[test]
    fn dropping_a_subscription_disconnects_it() {
        let signal = Signal::new(0);
        let notifications = Arc::new(AtomicUsize::new(0));

        {
            let observed = Arc::clone(&notifications);
            let _subscription = signal.subscribe(move |_| {
                let _ = observed.fetch_add(1, Ordering::Relaxed);
            });
            let _ = signal.set(1);
        }

        let _ = signal.set(2);
        assert_eq!(notifications.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn memo_tracks_a_chain_and_recomputes_lazily() {
        let items = Signal::new(vec![1, 2]);
        let count_runs = Arc::new(AtomicUsize::new(0));
        let label_runs = Arc::new(AtomicUsize::new(0));

        let count = {
            let items = items.clone();
            let runs = Arc::clone(&count_runs);
            Memo::new(move || {
                let _ = runs.fetch_add(1, Ordering::Relaxed);
                items.with(Vec::len)
            })
        };
        let label = {
            let count = count.clone();
            let runs = Arc::clone(&label_runs);
            Memo::new(move || {
                let _ = runs.fetch_add(1, Ordering::Relaxed);
                format!("{} items", count.get())
            })
        };

        assert_eq!(label.get(), "2 items");
        assert_eq!(label.get(), "2 items");
        assert_eq!(count_runs.load(Ordering::Relaxed), 1);
        assert_eq!(label_runs.load(Ordering::Relaxed), 1);

        items.update(|items| items.push(3));
        assert_eq!(label.get(), "3 items");
        assert_eq!(count_runs.load(Ordering::Relaxed), 2);
        assert_eq!(label_runs.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn memo_replaces_dynamic_dependencies() {
        let choose_left = Signal::new(true);
        let left = Signal::new(1);
        let right = Signal::new(10);
        let selected = {
            let choose_left = choose_left.clone();
            let left = left.clone();
            let right = right.clone();
            Memo::new(move || {
                if choose_left.get() {
                    left.get()
                } else {
                    right.get()
                }
            })
        };

        assert_eq!(selected.get(), 1);
        let _ = choose_left.set(false);
        assert_eq!(selected.get(), 10);

        let _ = left.set(2);
        assert_eq!(selected.get(), 10);
        assert_eq!(selected.revision().value(), 1);

        let _ = right.set(11);
        assert_eq!(selected.get(), 11);
        assert_eq!(selected.revision().value(), 2);
    }

    #[test]
    fn concurrent_updates_are_serialised_without_losing_values() {
        let signal = Signal::new(0_u64);
        let threads = (0..4)
            .map(|_| {
                let signal = signal.clone();
                std::thread::spawn(move || {
                    for _ in 0..1_000 {
                        signal.update(|value| *value += 1);
                    }
                })
            })
            .collect::<Vec<_>>();

        for thread in threads {
            thread.join().expect("update thread must not panic");
        }

        assert_eq!(signal.get(), 4_000);
        assert_eq!(signal.revision().value(), 4_000);
    }
}

//! Fine-grained, thread-safe reactive state for Taim.
//!
//! [`Signal`] owns a value and publishes changes. [`Memo`] lazily derives a
//! value from any signals or memos read while its computation runs. Dependency
//! tracking is automatic, including dynamic dependency sets, and subscriptions
//! are removed when their [`Subscription`] is dropped.

use std::cell::{Cell, RefCell};
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
    callbacks: Mutex<BTreeMap<u64, Listener>>,
}

#[derive(Clone)]
struct Listener {
    callback: Callback,
    dependency: bool,
}

struct PendingNotification {
    subscribers: Weak<Subscribers>,
    id: u64,
    revision: Revision,
}

thread_local! {
    static FLUSHING: Cell<bool> = const { Cell::new(false) };
    static BATCH_DEPTH: RefCell<usize> = const { RefCell::new(0) };
    static PENDING_NOTIFICATIONS: RefCell<BTreeMap<(usize, u64), PendingNotification>> = const { RefCell::new(BTreeMap::new()) };
}

struct BatchGuard;

impl Drop for BatchGuard {
    fn drop(&mut self) {
        let should_flush = BATCH_DEPTH.with_borrow_mut(|depth| {
            *depth -= 1;
            *depth == 0
        });
        if !should_flush || FLUSHING.with(|flushing| flushing.replace(true)) {
            return;
        }
        struct FlushGuard;
        impl Drop for FlushGuard {
            fn drop(&mut self) {
                FLUSHING.with(|flushing| flushing.set(false));
            }
        }
        let _flushing = FlushGuard;
        let mut first_panic = None;
        // Pop individually: writes made by a listener can replace a pending
        // revision instead of delivering a newer revision followed by an old one.
        while let Some((_, notification)) =
            PENDING_NOTIFICATIONS.with_borrow_mut(BTreeMap::pop_first)
        {
            let callback = notification.subscribers.upgrade().and_then(|subscribers| {
                subscribers
                    .callbacks
                    .lock()
                    .expect("reactive subscriber lock poisoned")
                    .get(&notification.id)
                    .map(|listener| listener.callback.clone())
            });
            if let Some(callback) = callback {
                // All committed writes must remain observable even if another
                // listener panics. Never double-panic while unwinding a batch.
                if let Err(payload) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    callback(notification.revision);
                })) && first_panic.is_none()
                {
                    first_panic = Some(payload);
                }
            }
        }
        if !std::thread::panicking()
            && let Some(payload) = first_panic
        {
            std::panic::resume_unwind(payload);
        }
    }
}

/// Groups notifications on the calling thread until the outermost batch exits.
///
/// Values and memo invalidation update immediately, so reads inside the batch
/// observe preceding writes. Each live subscriber is called once with its latest
/// queued revision. Writes made by callbacks enqueue further delivery without
/// recursive callback dispatch. Dropped subscriptions cancel queued delivery. Other threads
/// are not batched, and reads across sources are not transactional.
///
/// Writes are not rolled back on panic: the depth is restored and committed
/// notifications are delivered during unwinding. Listener panics do not suppress
/// other queued listeners; the first panic resumes after delivery, unless the
/// batch body is already unwinding (in which case its original panic is preserved).
pub fn batch<R>(f: impl FnOnce() -> R) -> R {
    BATCH_DEPTH.with_borrow_mut(|depth| *depth += 1);
    let _guard = BatchGuard;
    f()
}

/// Fine-grained dirty flags for reactive scene graph invalidation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct DirtyFlags(pub u32);

impl DirtyFlags {
    /// No changes.
    pub const NONE: Self = Self(0);
    /// Layout constraints or geometry must be recomputed.
    pub const LAYOUT: Self = Self(1 << 0);
    /// Visual appearance or paint stream must be re-rendered.
    pub const RENDER: Self = Self(1 << 1);
    /// Local or world transform matrix changed.
    pub const TRANSFORM: Self = Self(1 << 2);
    /// Text content or font shaping changed.
    pub const TEXT: Self = Self(1 << 3);
    /// Accessibility / semantic tree changed.
    pub const SEMANTICS: Self = Self(1 << 4);
    /// Clip boundary changed.
    pub const CLIP: Self = Self(1 << 5);
    /// All subsystems invalidated.
    pub const ALL: Self = Self(
        Self::LAYOUT.0
            | Self::RENDER.0
            | Self::TRANSFORM.0
            | Self::TEXT.0
            | Self::SEMANTICS.0
            | Self::CLIP.0,
    );

    /// Checks whether any of the flags in `other` are set.
    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    /// Checks whether all of the flags in `other` are set.
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Inserts the specified flags.
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    /// Removes the specified flags.
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    /// Unions two flags together.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns true if no flags are set.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for DirtyFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl std::ops::BitOrAssign for DirtyFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.insert(rhs);
    }
}

impl Subscribers {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            next_id: AtomicU64::new(0),
            callbacks: Mutex::new(BTreeMap::new()),
        })
    }

    fn subscribe(self: &Arc<Self>, callback: Callback) -> Subscription {
        self.insert(callback, false)
    }

    fn subscribe_dependency(self: &Arc<Self>, callback: Callback) -> Subscription {
        self.insert(callback, true)
    }

    fn insert(self: &Arc<Self>, callback: Callback, dependency: bool) -> Subscription {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let _ = self
            .callbacks
            .lock()
            .expect("reactive subscriber lock poisoned")
            .insert(
                id,
                Listener {
                    callback,
                    dependency,
                },
            );

        Subscription {
            subscribers: Arc::downgrade(self),
            id,
        }
    }

    fn notify(self: &Arc<Self>, revision: Revision) {
        let listeners = self
            .callbacks
            .lock()
            .expect("reactive subscriber lock poisoned")
            .iter()
            .map(|(id, listener)| (*id, listener.clone()))
            .collect::<Vec<_>>();

        // An implicit batch also protects ordinary writes: a diamond's entire
        // dependency graph is invalidated before any public callback can read it.
        batch(|| {
            for (id, listener) in listeners {
                if listener.dependency {
                    (listener.callback)(revision);
                } else {
                    PENDING_NOTIFICATIONS.with_borrow_mut(|pending| {
                        let key = (Arc::as_ptr(self) as usize, id);
                        let notification =
                            pending.entry(key).or_insert_with(|| PendingNotification {
                                subscribers: Arc::downgrade(self),
                                id,
                                revision,
                            });
                        notification.revision = revision;
                    });
                }
            }
        });
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
        self.subscribers.subscribe_dependency(callback)
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

    /// Runs `callback` after changes, coalescing delivery inside [`batch`].
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
        self.subscribers.subscribe_dependency(callback)
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

    #[test]
    fn batch_coalesces_notifications_until_completion() {
        let signal1 = Signal::new(10);
        let signal2 = Signal::new(20);
        let notify_count = Arc::new(AtomicU64::new(0));

        let _sub1 = {
            let count = Arc::clone(&notify_count);
            signal1.subscribe(move |_| {
                let _ = count.fetch_add(1, Ordering::Relaxed);
            })
        };
        let _sub2 = {
            let count = Arc::clone(&notify_count);
            signal2.subscribe(move |_| {
                let _ = count.fetch_add(1, Ordering::Relaxed);
            })
        };

        batch(|| {
            let _ = signal1.set(11);
            let _ = signal2.set(21);
            let _ = signal1.set(12);
            assert_eq!(notify_count.load(Ordering::Relaxed), 0);
        });

        // Out of batch: notifications flushed
        assert_eq!(notify_count.load(Ordering::Relaxed), 2);
        assert_eq!(signal1.get(), 12);
        assert_eq!(signal2.get(), 21);
    }

    #[test]
    fn batch_reads_see_current_memos_and_notify_once_at_latest_revision() {
        let source = Signal::new(1);
        let input = source.clone();
        let memo = Memo::new(move || input.get() * 2);
        assert_eq!(memo.get(), 2);
        let revisions = Arc::new(Mutex::new(Vec::new()));
        let output = revisions.clone();
        let _subscription = memo.subscribe(move |revision| output.lock().unwrap().push(revision));
        batch(|| {
            assert!(source.set(2));
            assert_eq!(memo.get(), 4);
            batch(|| {
                assert!(source.set(3));
                assert_eq!(memo.get(), 6);
            });
            assert!(revisions.lock().unwrap().is_empty());
        });
        assert_eq!(*revisions.lock().unwrap(), vec![memo.revision()]);
    }

    #[test]
    fn dropping_a_subscription_cancels_queued_delivery() {
        let source = Signal::new(1);
        let subscription = source.subscribe(|_| panic!("disconnected subscription called"));
        batch(|| {
            assert!(source.set(2));
            drop(subscription);
        });
    }

    #[test]
    fn panic_in_batch_restores_delivery_and_keeps_committed_writes() {
        let source = Signal::new(1);
        let seen = Arc::new(AtomicUsize::new(0));
        let output = seen.clone();
        let _subscription = source.subscribe(move |_| {
            let _ = output.fetch_add(1, Ordering::Relaxed);
        });
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            batch(|| {
                assert!(source.set(2));
                panic!("abort batch");
            })
        }));
        assert!(result.is_err());
        assert_eq!(source.get(), 2);
        assert_eq!(seen.load(Ordering::Relaxed), 1);
        assert!(source.set(3));
        assert_eq!(seen.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn diamond_is_current_before_subscriber_delivery() {
        let source = Signal::new(1);
        let input = source.clone();
        let left = Memo::new(move || input.get() * 2);
        let input = source.clone();
        let right = Memo::new(move || input.get() * 3);
        let sum = Memo::new(move || left.get() + right.get());
        assert_eq!(sum.get(), 5);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let output = seen.clone();
        let read = sum.clone();
        let _subscription = sum.subscribe(move |_| output.lock().unwrap().push(read.get()));
        assert!(source.set(2));
        assert_eq!(*seen.lock().unwrap(), vec![10]);
    }

    #[test]
    fn reentrant_writes_replace_pending_revisions_without_recursive_delivery() {
        let source = Signal::new(1);
        let input = source.clone();
        let active = Arc::new(AtomicBool::new(false));
        let check = active.clone();
        let _first = source.subscribe(move |_| {
            assert!(!check.swap(true, Ordering::AcqRel));
            let _ = input.set(3);
            check.store(false, Ordering::Release);
        });
        let revisions = Arc::new(Mutex::new(Vec::new()));
        let output = revisions.clone();
        let _second = source.subscribe(move |revision| output.lock().unwrap().push(revision));
        assert!(source.set(2));
        assert_eq!(*revisions.lock().unwrap(), vec![source.revision()]);
        assert!(!active.load(Ordering::Acquire));
    }

    #[test]
    fn panicking_listener_does_not_skip_other_listeners_or_stick_the_queue() {
        let source = Signal::new(1);
        let first = source.subscribe(|_| panic!("listener panic"));
        let seen = Arc::new(AtomicUsize::new(0));
        let output = seen.clone();
        let _second = source.subscribe(move |_| {
            let _ = output.fetch_add(1, Ordering::Relaxed);
        });
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| source.set(2)));
        assert!(result.is_err());
        assert_eq!(seen.load(Ordering::Relaxed), 1);
        drop(first);
        assert!(source.set(3));
        assert_eq!(seen.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn batch_delivers_latest_revision_across_counter_wrap() {
        let source = Signal::new(0);
        source.inner.revision.store(u64::MAX - 1, Ordering::Release);
        let revisions = Arc::new(Mutex::new(Vec::new()));
        let output = revisions.clone();
        let _subscription = source.subscribe(move |revision| output.lock().unwrap().push(revision));
        batch(|| {
            assert!(source.set(1));
            assert!(source.set(2));
        });
        assert_eq!(*revisions.lock().unwrap(), vec![Revision::INITIAL]);
    }

    #[test]
    fn dirty_flags_operations() {
        let mut flags = DirtyFlags::NONE;
        assert!(flags.is_empty());

        flags.insert(DirtyFlags::LAYOUT | DirtyFlags::RENDER);
        assert!(flags.intersects(DirtyFlags::LAYOUT));
        assert!(flags.intersects(DirtyFlags::RENDER));
        assert!(!flags.intersects(DirtyFlags::SEMANTICS));

        flags.remove(DirtyFlags::LAYOUT);
        assert!(!flags.intersects(DirtyFlags::LAYOUT));
        assert!(flags.intersects(DirtyFlags::RENDER));

        let all = DirtyFlags::ALL;
        assert!(all.contains(DirtyFlags::LAYOUT));
        assert!(all.contains(DirtyFlags::RENDER));
        assert!(all.contains(DirtyFlags::TRANSFORM));
        assert!(all.contains(DirtyFlags::TEXT));
        assert!(all.contains(DirtyFlags::SEMANTICS));
        assert!(all.contains(DirtyFlags::CLIP));
    }
}

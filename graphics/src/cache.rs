//! Retained rendering caches with explicit invalidation and hard budgets.
mod bounded;

pub use bounded::{BoundedCache, ByteSized, PurgeTelemetry};

use std::cell::{Cell, RefCell};
use std::fmt;
use std::mem;
use std::sync::atomic::{AtomicU64, Ordering};

static GLOBAL_VERSION: AtomicU64 = AtomicU64::new(0);

/// A single-value render cache that preserves its previous allocation after a
/// local clear, allowing a backend to recycle storage on the next fill.
pub struct Cache<T> {
    group: Group,
    state: RefCell<State<T>>,
    observed_version: Cell<u64>,
}

impl<T> Cache<T> {
    /// Creates an independent empty cache.
    pub fn new() -> Self {
        Self::from_group(Group::singleton())
    }

    /// Creates a cache in a shared storage group.
    pub fn with_group(group: Group) -> Self {
        assert!(
            !group.is_singleton(),
            "singleton cache groups cannot be shared"
        );
        Self::from_group(group)
    }

    #[inline]
    fn from_group(group: Group) -> Self {
        Self {
            group,
            state: RefCell::new(State::Empty { previous: None }),
            observed_version: Cell::new(GLOBAL_VERSION.load(Ordering::Relaxed)),
        }
    }

    #[inline]
    /// Returns this cache's storage-sharing group.
    pub fn group(&self) -> Group {
        self.group
    }

    /// Runs one ownership-preserving cache transaction.
    ///
    /// The current value moves into `operation`, letting renderers recycle
    /// backend allocations without cloning them. The returned state is then
    /// installed with a single mutable borrow.
    #[cfg(feature = "geometry")]
    pub(crate) fn transact<R>(
        &self,
        operation: impl FnOnce(State<T>, Group) -> (State<T>, R),
    ) -> R {
        self.synchronize();

        let current = self.state.replace(State::vacant());
        let (next, output) = operation(current, self.group);
        *self.state.borrow_mut() = next;

        output
    }

    /// Returns the state after lazily applying process-wide invalidation.
    pub fn state(&self) -> &RefCell<State<T>> {
        self.synchronize();
        &self.state
    }

    #[inline]
    fn synchronize(&self) {
        let version = GLOBAL_VERSION.load(Ordering::Relaxed);

        if self.observed_version.get() != version {
            *self.state.borrow_mut() = State::vacant();
            self.observed_version.set(version);
        }
    }

    /// Clears the current value but keeps it available for backend recycling.
    pub fn clear(&self) {
        self.synchronize();
        let mut state = self.state.borrow_mut();
        let previous = mem::replace(&mut *state, State::vacant()).into_value();
        *state = State::Empty { previous };
    }
}

impl<T> Default for Cache<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: fmt::Debug> fmt::Debug for Cache<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.synchronize();
        match &*self.state.borrow() {
            State::Empty { previous } => formatter
                .debug_struct("Cache::Empty")
                .field("previous", previous)
                .finish(),
            State::Filled { current } => formatter
                .debug_struct("Cache::Filled")
                .field("current", current)
                .finish(),
        }
    }
}

/// Storage-sharing identity for one or more caches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Group {
    id: u64,
    singleton: bool,
}

impl Group {
    /// Creates a new shareable group identity.
    pub fn unique() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);

        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            singleton: false,
        }
    }

    #[inline]
    /// Returns whether this identity belongs to exactly one cache.
    pub fn is_singleton(self) -> bool {
        self.singleton
    }

    #[inline]
    fn singleton() -> Self {
        Self {
            singleton: true,
            ..Self::unique()
        }
    }
}

/// The current or recyclable contents of a [`Cache`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State<T> {
    /// No current value, with optional recyclable backend storage.
    Empty {
        /// The last value retained for storage reuse.
        previous: Option<T>,
    },
    /// A current reusable value.
    Filled {
        /// The current value.
        current: T,
    },
}

impl<T> State<T> {
    #[inline]
    const fn vacant() -> Self {
        Self::Empty { previous: None }
    }

    #[inline]
    pub(crate) fn into_value(self) -> Option<T> {
        match self {
            Self::Empty { previous } => previous,
            Self::Filled { current } => Some(current),
        }
    }
}

/// A render value that can load and save a backend-specific representation.
pub trait Cached: Sized {
    /// Backend representation stored by [`Cache`].
    type Cache;

    /// Rehydrates a value from its cached representation.
    fn load(cache: &Self::Cache) -> Self;
    /// Converts this value into a cached representation.
    fn cache(self, group: Group, previous: Option<Self::Cache>) -> Self::Cache;
}

#[cfg(debug_assertions)]
impl Cached for () {
    type Cache = ();

    fn load(_cache: &Self::Cache) -> Self {}
    fn cache(self, _group: Group, _previous: Option<Self::Cache>) -> Self::Cache {}
}

/// Lazily invalidates every live single-value cache.
#[inline]
pub fn invalidate_all() {
    let _previous = GLOBAL_VERSION.fetch_add(1, Ordering::Relaxed);
}

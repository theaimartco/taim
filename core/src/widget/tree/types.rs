use std::any::{self, Any};
use std::fmt;

/// The identifier of some widget state.
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct Tag(any::TypeId);

impl Tag {
    /// Creates a [`Tag`] for a state of type `T`.
    pub fn of<T>() -> Self
    where
        T: 'static,
    {
        Self(any::TypeId::of::<T>())
    }

    /// Creates a [`Tag`] for a stateless widget.
    pub fn stateless() -> Self {
        Self::of::<()>()
    }
}

/// The internal [`State`] of a widget.
pub enum State {
    /// No meaningful internal state.
    None,

    /// Some meaningful internal state.
    Some(Box<dyn Any>),
}

impl State {
    /// Creates a new [`State`].
    pub fn new<T>(state: T) -> Self
    where
        T: 'static,
    {
        State::Some(Box::new(state))
    }

    /// Downcasts the [`State`] to `T` and returns a reference to it.
    ///
    /// # Panics
    /// This method will panic if the downcast fails or the [`State`] is [`State::None`].
    pub fn downcast_ref<T>(&self) -> &T
    where
        T: 'static,
    {
        match self {
            State::None => panic!("Downcast on stateless state"),
            State::Some(state) => state.downcast_ref().expect("Downcast widget state"),
        }
    }

    /// Downcasts the [`State`] to `T` and returns a mutable reference to it.
    ///
    /// # Panics
    /// This method will panic if the downcast fails or the [`State`] is [`State::None`].
    pub fn downcast_mut<T>(&mut self) -> &mut T
    where
        T: 'static,
    {
        match self {
            State::None => panic!("Downcast on stateless state"),
            State::Some(state) => state.downcast_mut().expect("Downcast widget state"),
        }
    }
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "State::None"),
            Self::Some(_) => write!(f, "State::Some"),
        }
    }
}

/// A monotonically increasing revision counter for a [`Tree`] node.
///
/// # Invariant
///
/// [`Revision`] uses **generation replacement**: [`Tree::mark_dirty`] is
/// the only way to advance it, and it only ever advances (via
/// [`Revision::next`], which wraps rather than panics on overflow, like
/// [`crate::frame::Id`]). Comparing two [`Revision`]s is only meaningful
/// when both came from the *same* [`Tree`] node's lifetime; a
/// [`Revision`] is not a global clock and must not be compared across
/// different nodes or across a node being recreated (which resets it to
/// [`Revision::INITIAL`]). Use [`Revision::wrapping_after`] rather than a
/// numeric comparison, for the same wraparound-safety reason
/// [`crate::frame::Id`] documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Revision(pub(super) u64);

impl Revision {
    /// The revision every new [`Tree`] node starts at.
    pub const INITIAL: Self = Self(0);

    /// Returns the next [`Revision`], wrapping on overflow.
    pub const fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }

    /// Returns the raw revision value for renderer cache validation.
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Returns whether `self` comes strictly after `other`, correctly
    /// accounting for one wraparound of the underlying counter. See the
    /// [`Revision`] documentation for why this -- not a numeric
    /// comparison -- is the correct way to order two revisions.
    pub const fn wrapping_after(self, other: Self) -> bool {
        self.0.wrapping_sub(other.0) as i64 > 0
    }
}

bitflags::bitflags! {
    /// Which aspects of a [`Tree`] node are stale and need recomputing.
    ///
    /// A fresh [`Tree`] starts with every flag set ([`DirtyFlags::all`]):
    /// a node that has never been laid out, painted, or had its
    /// semantics computed cannot be anything but dirty in every respect.
    /// Clearing a flag is a claim, by whichever phase implements the
    /// corresponding pass, that the cached result for that aspect (see
    /// [`Tree::cached_bounds`], [`Tree::cached_transform`],
    /// [`Tree::cached_clip`]) is valid and may be reused instead of
    /// recomputed.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DirtyFlags: u8 {
        /// This node's layout may have changed and must be recomputed.
        const LAYOUT = 1 << 0;
        /// This node's appearance may have changed and must be repainted.
        const PAINT = 1 << 1;
        /// This node's semantic tree may have changed and must be
        /// recomputed (see [`crate::semantics`]).
        const SEMANTICS = 1 << 2;
    }
}

use crate::SmolStr;

use std::borrow::Cow;
use std::sync::atomic::{self, AtomicU64};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// The identifier of a generic widget.
///
/// An [`Id`] is always cheap to clone and compare: [`Id::new`] and
/// [`Id::unique`] never allocate on the heap, and [`Id::keyed`] only
/// allocates when the given key does not fit inline (see [`SmolStr`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Id(Internal);

impl Id {
    /// Creates a new [`Id`] from a static `str`.
    ///
    /// This never allocates on the heap.
    pub const fn new(id: &'static str) -> Self {
        Self(Internal::Custom(SmolStr::new_static(id)))
    }

    /// Creates a unique [`Id`].
    ///
    /// This function produces a different [`Id`] every time it is called.
    pub fn unique() -> Self {
        let id = NEXT_ID.fetch_add(1, atomic::Ordering::Relaxed);

        Self(Internal::Unique(id))
    }

    /// Creates an [`Id`] that identifies an element of a keyed list (e.g. a
    /// row produced from application data) rather than a single fixed
    /// widget.
    ///
    /// Unlike [`Id::unique`], a keyed [`Id`] is stable across rebuilds of
    /// the list as long as the same `key` is used, which lets the runtime
    /// preserve state and avoid unnecessary work when list items are
    /// reordered, inserted, or removed (see `runtime::reconcile`, added in
    /// a later phase). Two [`Id::keyed`] values are equal if and only if
    /// their keys are equal.
    pub fn keyed(key: impl Into<Key>) -> Self {
        Self(Internal::Keyed(key.into()))
    }

    /// Returns the stable [`Key`] of this [`Id`], if it was created with
    /// [`Id::keyed`].
    pub fn key(&self) -> Option<&Key> {
        match &self.0 {
            Internal::Keyed(key) => Some(key),
            Internal::Unique(_) | Internal::Custom(_) => None,
        }
    }
}

impl From<&'static str> for Id {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

impl From<String> for Id {
    fn from(value: String) -> Self {
        Self(Internal::Custom(SmolStr::new(value)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Internal {
    Unique(u64),
    Custom(SmolStr),
    Keyed(Key),
}

/// A compact, stable key identifying an item in a keyed list.
///
/// A [`Key`] is cheap to store and compare: integers and short strings
/// (see [`SmolStr`]) are kept inline, with no heap allocation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    /// A key backed by an integer, the common case for data indexed by a
    /// database id, index, or similar.
    Integer(u64),
    /// A key backed by a short string.
    Text(SmolStr),
}

impl From<u64> for Key {
    fn from(value: u64) -> Self {
        Key::Integer(value)
    }
}

impl From<usize> for Key {
    fn from(value: usize) -> Self {
        Key::Integer(value as u64)
    }
}

impl From<&'static str> for Key {
    fn from(value: &'static str) -> Self {
        Key::Text(SmolStr::new_static(value))
    }
}

impl From<String> for Key {
    fn from(value: String) -> Self {
        Key::Text(SmolStr::new(value))
    }
}

impl From<Cow<'static, str>> for Key {
    fn from(value: Cow<'static, str>) -> Self {
        match value {
            Cow::Borrowed(value) => Key::Text(SmolStr::new_static(value)),
            Cow::Owned(value) => Key::Text(SmolStr::new(value)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Id, Key};

    #[test]
    fn unique_generates_different_ids() {
        let a = Id::unique();
        let b = Id::unique();

        assert_ne!(a, b);
    }

    #[test]
    fn keyed_ids_with_the_same_key_are_equal() {
        let a = Id::keyed(42_u64);
        let b = Id::keyed(42_u64);

        assert_eq!(a, b);
        assert_eq!(a.key(), Some(&Key::Integer(42)));
    }

    #[test]
    fn keyed_ids_with_different_keys_are_not_equal() {
        let a = Id::keyed("row-a");
        let b = Id::keyed("row-b");

        assert_ne!(a, b);
    }

    #[test]
    fn keyed_ids_do_not_equal_unique_or_custom_ids() {
        let keyed = Id::keyed(1_u64);
        let unique = Id::unique();
        let custom = Id::new("some-id");

        assert_ne!(keyed, unique);
        assert_ne!(keyed, custom);
    }

    #[test]
    fn non_keyed_ids_have_no_key() {
        assert_eq!(Id::unique().key(), None);
        assert_eq!(Id::new("static").key(), None);
    }
}

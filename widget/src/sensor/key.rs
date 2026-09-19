/// A continuity key used by [`super::Sensor`].
pub trait Key {
    /// Owned key stored in retained widget state.
    type Owned: 'static;

    /// Clones or owns this key for retained storage.
    fn to_owned(&self) -> Self::Owned;

    /// Compares this key with its retained representation.
    fn eq(&self, other: &Self::Owned) -> bool;
}

impl<T> Key for &T
where
    T: ToOwned + PartialEq<T::Owned> + ?Sized,
    T::Owned: 'static,
{
    type Owned = T::Owned;

    fn to_owned(&self) -> Self::Owned {
        ToOwned::to_owned(*self)
    }

    fn eq(&self, other: &Self::Owned) -> bool {
        *self == other
    }
}

pub(super) struct Owned<T>(pub(super) T);

impl<T> Key for Owned<T>
where
    T: PartialEq + Clone + 'static,
{
    type Owned = T;

    fn to_owned(&self) -> Self::Owned {
        self.0.clone()
    }

    fn eq(&self, other: &Self::Owned) -> bool {
        &self.0 == other
    }
}

impl Key for () {
    type Owned = ();

    fn to_owned(&self) -> Self::Owned {}

    fn eq(&self, _other: &Self::Owned) -> bool {
        true
    }
}

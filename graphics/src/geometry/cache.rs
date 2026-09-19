use crate::cache::{self, Cached};
use crate::core::{Rectangle, Size};
use crate::geometry::{self, Frame};
use std::hash::{DefaultHasher, Hash, Hasher};

pub use cache::Group;

/// A simple cache that stores generated geometry to avoid recomputation.
///
/// A [`Cache`] will not redraw its geometry unless the dimensions of its layer
/// change or it is explicitly cleared.
pub struct Cache<Renderer>
where
    Renderer: geometry::Renderer,
{
    raw: crate::Cache<Data<<Renderer::Geometry as Cached>::Cache>>,
}

#[derive(Debug)]
struct Data<T> {
    bounds: Rectangle,
    fingerprint: Option<u64>,
    geometry: T,
}

impl<Renderer> Cache<Renderer>
where
    Renderer: geometry::Renderer,
{
    /// Creates a new empty [`Cache`].
    pub fn new() -> Self {
        Cache {
            raw: cache::Cache::new(),
        }
    }

    /// Creates a new empty [`Cache`] with the given [`Group`].
    ///
    /// Caches within the same group may reuse internal rendering storage.
    ///
    /// You should generally group caches that are likely to change
    /// together.
    pub fn with_group(group: Group) -> Self {
        Cache {
            raw: crate::Cache::with_group(group),
        }
    }

    /// Clears the [`Cache`], forcing a redraw the next time it is used.
    pub fn clear(&self) {
        self.raw.clear();
    }

    /// Returns whether an unkeyed draw of `size` can be served immediately.
    pub fn contains(&self, size: Size) -> bool {
        self.matches(Rectangle::with_size(size), None)
    }

    /// Returns whether a keyed draw of `size` can be served immediately.
    pub fn contains_keyed<Key: Hash>(&self, size: Size, key: &Key) -> bool {
        self.matches(Rectangle::with_size(size), Some(fingerprint(key)))
    }

    /// Bounds-aware variant of [`Cache::contains`].
    pub fn contains_with_bounds(&self, bounds: Rectangle) -> bool {
        self.matches(bounds, None)
    }

    /// Bounds-aware variant of [`Cache::contains_keyed`].
    pub fn contains_with_bounds_keyed<Key: Hash>(&self, bounds: Rectangle, key: &Key) -> bool {
        self.matches(bounds, Some(fingerprint(key)))
    }

    /// Returns the bounds of the currently reusable geometry, if any.
    pub fn current_bounds(&self) -> Option<Rectangle> {
        match &*self.raw.state().borrow() {
            cache::State::Filled { current } => Some(current.bounds),
            cache::State::Empty { .. } => None,
        }
    }

    /// Returns whether this cache currently requires geometry generation.
    pub fn is_empty(&self) -> bool {
        self.current_bounds().is_none()
    }

    /// Draws geometry using the provided closure and stores it in the
    /// [`Cache`].
    ///
    /// The closure will only be called when
    /// - the size has changed since the previous draw call.
    /// - the [`Cache`] is empty or has been explicitly cleared.
    ///
    /// Otherwise, the previously stored geometry will be returned. The
    /// [`Cache`] is not cleared in this case. In other words, it will keep
    /// returning the stored geometry if needed.
    pub fn draw(
        &self,
        renderer: &Renderer,
        size: Size,
        draw_fn: impl FnOnce(&mut Frame<Renderer>),
    ) -> Renderer::Geometry {
        self.render(renderer, Rectangle::with_size(size), None, draw_fn)
    }

    /// Draws and caches geometry under an explicit content key.
    ///
    /// Unlike [`Cache::draw`], changing `key` invalidates the cached geometry
    /// automatically. This avoids an error-prone separate [`Cache::clear`]
    /// call when canvas output depends on application state.
    pub fn draw_keyed<Key: Hash>(
        &self,
        renderer: &Renderer,
        size: Size,
        key: &Key,
        draw_fn: impl FnOnce(&mut Frame<Renderer>),
    ) -> Renderer::Geometry {
        self.render(
            renderer,
            Rectangle::with_size(size),
            Some(fingerprint(key)),
            draw_fn,
        )
    }

    /// Draws geometry using the provided closure and stores it in the
    /// [`Cache`].
    ///
    /// Analogous to [`draw`](Self::draw), but takes a clipping [`Rectangle`] instead of
    /// a [`Size`].
    pub fn draw_with_bounds(
        &self,
        renderer: &Renderer,
        bounds: Rectangle,
        draw_fn: impl FnOnce(&mut Frame<Renderer>),
    ) -> Renderer::Geometry {
        self.render(renderer, bounds, None, draw_fn)
    }

    /// Bounds-aware variant of [`Cache::draw_keyed`].
    pub fn draw_with_bounds_keyed<Key: Hash>(
        &self,
        renderer: &Renderer,
        bounds: Rectangle,
        key: &Key,
        draw_fn: impl FnOnce(&mut Frame<Renderer>),
    ) -> Renderer::Geometry {
        self.render(renderer, bounds, Some(fingerprint(key)), draw_fn)
    }

    fn render(
        &self,
        renderer: &Renderer,
        bounds: Rectangle,
        fingerprint: Option<u64>,
        draw_fn: impl FnOnce(&mut Frame<Renderer>),
    ) -> Renderer::Geometry {
        self.raw.transact(|state, group| match state {
            cache::State::Filled { current }
                if current.bounds == bounds && current.fingerprint == fingerprint =>
            {
                let output = Cached::load(&current.geometry);
                (cache::State::Filled { current }, output)
            }
            stale => {
                let reusable = stale.into_value().map(|data| data.geometry);
                let mut frame = Frame::with_bounds(renderer, bounds);
                draw_fn(&mut frame);

                let geometry = frame.into_geometry().cache(group, reusable);
                let output = Cached::load(&geometry);
                let next = cache::State::Filled {
                    current: Data {
                        bounds,
                        fingerprint,
                        geometry,
                    },
                };

                (next, output)
            }
        })
    }

    fn matches(&self, bounds: Rectangle, fingerprint: Option<u64>) -> bool {
        matches!(
            &*self.raw.state().borrow(),
            cache::State::Filled { current }
                if current.bounds == bounds && current.fingerprint == fingerprint
        )
    }
}

#[inline]
fn fingerprint(key: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

impl<Renderer> std::fmt::Debug for Cache<Renderer>
where
    Renderer: geometry::Renderer,
    <Renderer::Geometry as Cached>::Cache: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.raw)
    }
}

impl<Renderer> Default for Cache<Renderer>
where
    Renderer: geometry::Renderer,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn content_keys_reuse_only_matching_geometry() {
        let cache = Cache::<()>::new();
        let draws = Cell::new(0);
        let size = Size::new(20.0, 10.0);

        for key in [7_u8, 7, 8] {
            cache.draw_keyed(&(), size, &key, |_| draws.set(draws.get() + 1));
        }

        assert_eq!(draws.get(), 2);
        assert!(cache.contains_keyed(size, &8_u8));
        assert!(!cache.contains_keyed(size, &7_u8));
        assert!(cache.contains_with_bounds_keyed(Rectangle::with_size(size), &8_u8));
    }

    #[test]
    fn keyed_and_unkeyed_entries_never_share_a_namespace() {
        let cache = Cache::<()>::new();
        let draws = Cell::new(0);
        let bounds = Rectangle::with_size(Size::new(20.0, 10.0));

        cache.draw_with_bounds(&(), bounds, |_| draws.set(draws.get() + 1));
        cache.render(&(), bounds, Some(0), |_| draws.set(draws.get() + 1));

        assert_eq!(draws.get(), 2);
        assert!(!cache.contains_with_bounds(bounds));
        assert!(cache.matches(bounds, Some(0)));
    }
}

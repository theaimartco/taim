//! Store internal widget state in a state tree to ensure continuity.
use crate::layout;
use crate::{Rectangle, Transformation, Widget};

use std::any::{self, Any};
use std::borrow::{Borrow, BorrowMut};
use std::fmt;

/// A persistent state widget tree.
///
/// A [`Tree`] is normally associated with a specific widget in the widget tree.
#[derive(Debug)]
pub struct Tree {
    /// The tag of the [`Tree`].
    pub tag: Tag,

    /// The [`State`] of the [`Tree`].
    pub state: State,

    /// The children of the root widget of the [`Tree`].
    pub children: Vec<Tree>,

    /// The current [`Revision`] of this [`Tree`].
    ///
    /// Starts at [`Revision::INITIAL`] and is bumped by [`Tree::mark_dirty`]
    /// whenever this node's own state changes in a way that should be
    /// observable to a cache keyed on it (e.g. a later phase's layout or
    /// paint cache). Diffing a widget does not bump this on its own; only
    /// an actual state change does, once callers start calling
    /// [`Tree::mark_dirty`] -- Phase 2 introduces the counter without yet
    /// wiring any caller to it, per its "without yet changing all
    /// behavior" objective.
    revision: Revision,

    /// Which aspects of this node are stale and need recomputing.
    ///
    /// Every [`Tree`] starts fully dirty (a first layout/paint/semantics
    /// pass is always required); clearing bits is the responsibility of
    /// whichever later phase performs that pass and can prove the result
    /// is being reused (Phase 3 for layout/paint, Phase 13 for
    /// semantics). Phase 2 only defines the flags and their default
    /// state.
    dirty: DirtyFlags,

    /// The bounds this node was last laid out to, if a later phase has
    /// populated it. `None` until then; Phase 2 never populates this
    /// itself.
    cached_bounds: Option<Rectangle>,

    /// The transformation in effect the last time this node was drawn,
    /// if a later phase has populated it. `None` until then.
    cached_transform: Option<Transformation>,

    /// The clip rectangle in effect the last time this node was drawn,
    /// if a later phase has populated it. `None` until then.
    cached_clip: Option<Rectangle>,

    /// The [`layout::Limits`] this node was last measured against,
    /// together with the [`layout::Node`] that measurement produced.
    /// Consumed by [`Tree::layout_with_cache`]; `None` until a caller
    /// uses that method at least once.
    cached_layout: Option<(layout::Limits, layout::Node)>,
}

impl Tree {
    /// Creates an empty, stateless [`Tree`] with no children.
    pub fn empty() -> Self {
        Self {
            tag: Tag::stateless(),
            state: State::None,
            children: Vec::new(),
            revision: Revision::INITIAL,
            dirty: DirtyFlags::all(),
            cached_bounds: None,
            cached_transform: None,
            cached_clip: None,
            cached_layout: None,
        }
    }

    /// Creates an empty, stateless [`Tree`] with the given `children`.
    ///
    /// Equivalent to [`Tree::empty`] but with [`Tree::children`] set
    /// directly, for containers (e.g. `pane_grid`) that manage their
    /// children's [`Tree`]s themselves without an owning [`Widget`].
    pub fn with_children(children: Vec<Tree>) -> Self {
        Self {
            children,
            ..Self::empty()
        }
    }

    /// Creates a new [`Tree`] with the given `tag`, `state`, and
    /// `children`.
    ///
    /// For widgets that build a synthetic [`Tree`] not tied to a single
    /// [`Widget`] instance (e.g. a lazily-rebuilt component wrapping its
    /// own state type); prefer [`Tree::new`] whenever a concrete
    /// [`Widget`] is available.
    pub fn with_state(tag: Tag, state: State, children: Vec<Tree>) -> Self {
        Self {
            tag,
            state,
            children,
            ..Self::empty()
        }
    }

    /// Creates a new [`Tree`] for the provided [`Widget`].
    pub fn new<'a, Message, Theme, Renderer>(
        widget: impl Borrow<dyn Widget<Message, Theme, Renderer> + 'a>,
    ) -> Self
    where
        Renderer: crate::Renderer,
    {
        let widget = widget.borrow();

        Self {
            tag: widget.tag(),
            state: widget.state(),
            children: Vec::new(),
            revision: Revision::INITIAL,
            dirty: DirtyFlags::all(),
            cached_bounds: None,
            cached_transform: None,
            cached_clip: None,
            cached_layout: None,
        }
    }

    /// Reconciles the current tree with the provided [`Widget`].
    ///
    /// If the tag of the [`Widget`] matches the tag of the [`Tree`], then the
    /// [`Widget`] proceeds with the reconciliation (i.e. [`Widget::diff`] is called).
    ///
    /// Otherwise, the whole [`Tree`] is recreated.
    ///
    /// [`Widget::diff`]: crate::Widget::diff
    pub fn diff<'a, Message, Theme, Renderer>(
        &mut self,
        mut new: impl BorrowMut<dyn Widget<Message, Theme, Renderer> + 'a>,
    ) where
        Renderer: crate::Renderer,
    {
        if self.tag != new.borrow().tag() {
            *self = Self::new(new.borrow());
        }

        new.borrow_mut().diff(self);
    }

    /// Returns the current [`Revision`] of this [`Tree`].
    pub fn revision(&self) -> Revision {
        self.revision
    }

    /// Bumps this [`Tree`]'s [`Revision`] and marks the given `flags` as
    /// dirty.
    ///
    /// Callers should pass the narrowest [`DirtyFlags`] that correctly
    /// describes what changed (design constraint: "dirty propagation
    /// stops as early as correctness allows"); marking more than
    /// necessary is always safe but defeats the purpose of tracking
    /// flags at all.
    pub fn mark_dirty(&mut self, flags: DirtyFlags) {
        self.revision = self.revision.next();
        self.dirty.insert(flags);
    }

    /// Returns the [`DirtyFlags`] currently set on this [`Tree`].
    pub fn dirty(&self) -> DirtyFlags {
        self.dirty
    }

    /// Clears the given `flags`, recording that whatever depended on them
    /// has been recomputed and is now up to date.
    pub fn clear_dirty(&mut self, flags: DirtyFlags) {
        self.dirty.remove(flags);
    }

    /// Returns the cached bounds from this node's last layout pass, if any.
    pub fn cached_bounds(&self) -> Option<Rectangle> {
        self.cached_bounds
    }

    /// Records the bounds produced by this node's most recent layout pass.
    pub fn set_cached_bounds(&mut self, bounds: Rectangle) {
        self.cached_bounds = Some(bounds);
    }

    /// Returns the cached transform from this node's last paint pass, if any.
    pub fn cached_transform(&self) -> Option<Transformation> {
        self.cached_transform
    }

    /// Records the transform in effect during this node's most recent
    /// paint pass.
    pub fn set_cached_transform(&mut self, transform: Transformation) {
        self.cached_transform = Some(transform);
    }

    /// Returns the cached clip rectangle from this node's last paint
    /// pass, if any.
    pub fn cached_clip(&self) -> Option<Rectangle> {
        self.cached_clip
    }

    /// Records the clip rectangle in effect during this node's most
    /// recent paint pass.
    pub fn set_cached_clip(&mut self, clip: Rectangle) {
        self.cached_clip = Some(clip);
    }

    /// Computes a [`layout::Node`] for the given `limits`, reusing the
    /// last computed result instead of calling `compute` when it is
    /// provably still valid.
    ///
    /// The cached result is reused only when **both** hold:
    /// - `limits` is identical to the [`layout::Limits`] the cache was
    ///   populated with ([`layout::Limits`] is a plain value type, so
    ///   this is an exact equality check, not a heuristic).
    /// - [`DirtyFlags::LAYOUT`] is *not* set on this node.
    ///
    /// A widget (or the container measuring it) is responsible for
    /// calling [`Tree::mark_dirty`] with [`DirtyFlags::LAYOUT`] whenever
    /// something that would change this node's measured size changes
    /// (its own content, its children, a font, a scale factor, ...).
    /// [`Tree`] cannot detect that on its own; a node that never calls
    /// [`Tree::mark_dirty`] keeps reusing a stale cached layout forever,
    /// which is why every fresh [`Tree`] starts with
    /// [`DirtyFlags::LAYOUT`] already set (see [`Tree::empty`]) -- the
    /// first call always computes for real.
    ///
    /// This method is opt-in: nothing in the runtime calls it yet on
    /// behalf of an unmodified widget (Phase 3's "without retrofitting
    /// every widget" scope), but any container's `Widget::layout` can
    /// call it directly to skip a child's measurement when it can prove
    /// the child is clean, and `tests/layout/incremental.rs`'s synthetic
    /// tree does exactly that to demonstrate the mechanism end to end.
    pub fn layout_with_cache(
        &mut self,
        limits: layout::Limits,
        compute: impl FnOnce(&mut Self) -> layout::Node,
    ) -> layout::Node {
        if !self.dirty.contains(DirtyFlags::LAYOUT)
            && let Some((cached_limits, cached_node)) = &self.cached_layout
            && *cached_limits == limits
        {
            return cached_node.clone();
        }

        let node = compute(self);

        self.cached_layout = Some((limits, node.clone()));
        self.dirty.remove(DirtyFlags::LAYOUT);

        node
    }

    /// Reconciles the children of the tree with the provided list of widgets.
    pub fn diff_children<'a, Message, Theme, Renderer>(
        &mut self,
        new_children: &mut [impl BorrowMut<dyn Widget<Message, Theme, Renderer> + 'a>],
    ) where
        Renderer: crate::Renderer,
    {
        self.diff_children_custom(
            new_children,
            |tree, widget| tree.diff(widget.borrow_mut()),
            |widget| Self::new(widget.borrow()),
        );
    }

    /// Reconciles the children of the tree with the provided list of widgets using custom
    /// logic both for diffing and creating new widget state.
    pub fn diff_children_custom<T>(
        &mut self,
        new_children: &mut [T],
        diff: impl Fn(&mut Tree, &mut T),
        new_state: impl Fn(&T) -> Self,
    ) {
        if self.children.len() > new_children.len() {
            self.children.truncate(new_children.len());
        }

        if self.children.len() < new_children.len() {
            self.children
                .extend(new_children[self.children.len()..].iter().map(new_state));
        }

        for (child_state, new) in self.children.iter_mut().zip(new_children.iter_mut()) {
            diff(child_state, new);
        }
    }
}

/// Reconciles the `current_children` with the provided list of widgets using
/// custom logic both for diffing and creating new widget state.
///
/// The algorithm will try to minimize the impact of diffing by querying the
/// `maybe_changed` closure.
pub fn diff_children_custom_with_search<T>(
    current_children: &mut Vec<Tree>,
    new_children: &mut [T],
    diff: impl Fn(&mut Tree, &mut T),
    maybe_changed: impl Fn(usize) -> bool,
    new_state: impl Fn(&T) -> Tree,
) {
    if new_children.is_empty() {
        current_children.clear();
        return;
    }

    if current_children.is_empty() {
        current_children.extend(new_children.iter().map(new_state));

        // TODO: Merge loop with extend logic (?)
        for (child_state, new) in current_children.iter_mut().zip(new_children.iter_mut()) {
            diff(child_state, new);
        }

        return;
    }

    let first_maybe_changed = maybe_changed(0);
    let last_maybe_changed = maybe_changed(current_children.len() - 1);

    if current_children.len() > new_children.len() {
        if !first_maybe_changed && last_maybe_changed {
            current_children.truncate(new_children.len());
        } else {
            let difference_index = if first_maybe_changed {
                0
            } else {
                (1..current_children.len())
                    .find(|&i| maybe_changed(i))
                    .unwrap_or(0)
            };

            let _ = current_children.splice(
                difference_index..difference_index + (current_children.len() - new_children.len()),
                std::iter::empty(),
            );
        }
    }

    if current_children.len() < new_children.len() {
        let first_maybe_changed = maybe_changed(0);
        let last_maybe_changed = maybe_changed(current_children.len() - 1);

        if !first_maybe_changed && last_maybe_changed {
            current_children.extend(new_children[current_children.len()..].iter().map(new_state));
        } else {
            let difference_index = if first_maybe_changed {
                0
            } else {
                (1..current_children.len())
                    .find(|&i| maybe_changed(i))
                    .unwrap_or(0)
            };

            let _ = current_children.splice(
                difference_index..difference_index,
                new_children[difference_index
                    ..difference_index + (new_children.len() - current_children.len())]
                    .iter()
                    .map(new_state),
            );
        }
    }

    // TODO: Merge loop with extend logic (?)
    for (child_state, new) in current_children.iter_mut().zip(new_children.iter_mut()) {
        diff(child_state, new);
    }
}

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
pub struct Revision(u64);

impl Revision {
    /// The revision every new [`Tree`] node starts at.
    pub const INITIAL: Self = Self(0);

    /// Returns the next [`Revision`], wrapping on overflow.
    pub const fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
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

#[cfg(test)]
mod revision_tests {
    use super::*;

    #[test]
    fn a_fresh_tree_starts_at_the_initial_revision_and_is_fully_dirty() {
        let tree = Tree::empty();

        assert_eq!(tree.revision(), Revision::INITIAL);
        assert_eq!(tree.dirty(), DirtyFlags::all());
    }

    #[test]
    fn mark_dirty_advances_the_revision_and_sets_the_given_flags() {
        let mut tree = Tree::empty();
        tree.clear_dirty(DirtyFlags::all());

        tree.mark_dirty(DirtyFlags::PAINT);

        assert!(tree.revision().wrapping_after(Revision::INITIAL));
        assert_eq!(tree.dirty(), DirtyFlags::PAINT);
    }

    #[test]
    fn clear_dirty_only_clears_the_given_flags() {
        let mut tree = Tree::empty();

        tree.clear_dirty(DirtyFlags::LAYOUT);

        assert!(!tree.dirty().contains(DirtyFlags::LAYOUT));
        assert!(tree.dirty().contains(DirtyFlags::PAINT));
        assert!(tree.dirty().contains(DirtyFlags::SEMANTICS));
    }

    #[test]
    fn revision_wrapping_after_handles_counter_wraparound() {
        let near_max = Revision(u64::MAX);
        let wrapped = near_max.next();

        assert_eq!(wrapped, Revision(0));
        assert!(wrapped.wrapping_after(near_max));
    }

    #[test]
    fn cached_metadata_starts_empty_and_can_be_populated() {
        let mut tree = Tree::empty();

        assert_eq!(tree.cached_bounds(), None);
        assert_eq!(tree.cached_transform(), None);
        assert_eq!(tree.cached_clip(), None);

        let bounds = Rectangle::new(crate::Point::ORIGIN, crate::Size::new(10.0, 10.0));
        tree.set_cached_bounds(bounds);
        tree.set_cached_transform(Transformation::IDENTITY);
        tree.set_cached_clip(bounds);

        assert_eq!(tree.cached_bounds(), Some(bounds));
        assert_eq!(tree.cached_transform(), Some(Transformation::IDENTITY));
        assert_eq!(tree.cached_clip(), Some(bounds));
    }
}

#[cfg(test)]
mod layout_cache_tests {
    use super::*;
    use crate::{Point, Size};

    fn limits(width: f32, height: f32) -> layout::Limits {
        layout::Limits::new(Size::ZERO, Size::new(width, height))
    }

    fn node_at(size: f32) -> layout::Node {
        layout::Node::new(Size::new(size, size))
    }

    #[test]
    fn a_dirty_node_always_recomputes() {
        let mut tree = Tree::empty();
        let mut calls = 0;

        let node = tree.layout_with_cache(limits(10.0, 10.0), |_| {
            calls += 1;
            node_at(10.0)
        });

        assert_eq!(
            calls, 1,
            "a fresh tree starts LAYOUT-dirty, so this must compute"
        );
        assert_eq!(node.size(), Size::new(10.0, 10.0));
    }

    #[test]
    fn a_clean_node_with_identical_limits_reuses_the_cached_result() {
        let mut tree = Tree::empty();

        let _ = tree.layout_with_cache(limits(10.0, 10.0), |_| node_at(10.0));
        assert!(
            !tree.dirty().contains(DirtyFlags::LAYOUT),
            "the first call clears LAYOUT"
        );

        let mut calls = 0;
        let node = tree.layout_with_cache(limits(10.0, 10.0), |_| {
            calls += 1;
            node_at(999.0)
        });

        assert_eq!(
            calls, 0,
            "identical limits on a clean node must not recompute"
        );
        assert_eq!(
            node.size(),
            Size::new(10.0, 10.0),
            "the reused node must be the originally cached one, not a fresh 999.0 one"
        );
    }

    #[test]
    fn different_limits_always_recompute_even_when_clean() {
        let mut tree = Tree::empty();

        let _ = tree.layout_with_cache(limits(10.0, 10.0), |_| node_at(10.0));

        let mut calls = 0;
        let node = tree.layout_with_cache(limits(20.0, 20.0), |_| {
            calls += 1;
            node_at(20.0)
        });

        assert_eq!(calls, 1, "different limits must always recompute");
        assert_eq!(node.size(), Size::new(20.0, 20.0));
    }

    #[test]
    fn marking_layout_dirty_forces_a_recompute_even_with_identical_limits() {
        let mut tree = Tree::empty();

        let _ = tree.layout_with_cache(limits(10.0, 10.0), |_| node_at(10.0));
        tree.mark_dirty(DirtyFlags::LAYOUT);

        let mut calls = 0;
        let node = tree.layout_with_cache(limits(10.0, 10.0), |_| {
            calls += 1;
            node_at(999.0)
        });

        assert_eq!(
            calls, 1,
            "LAYOUT dirty must force a recompute regardless of limits"
        );
        assert_eq!(node.size(), Size::new(999.0, 999.0));
    }

    #[test]
    fn marking_only_paint_dirty_does_not_force_a_layout_recompute() {
        let mut tree = Tree::empty();

        let _ = tree.layout_with_cache(limits(10.0, 10.0), |_| node_at(10.0));
        tree.mark_dirty(DirtyFlags::PAINT);

        let mut calls = 0;
        let node = tree.layout_with_cache(limits(10.0, 10.0), |_| {
            calls += 1;
            node_at(999.0)
        });

        assert_eq!(calls, 0, "PAINT dirty must not force a LAYOUT recompute");
        assert_eq!(node.size(), Size::new(10.0, 10.0));
    }

    #[test]
    fn reused_bounds_are_unaffected_by_position() {
        let mut tree = Tree::empty();

        let node = tree.layout_with_cache(limits(10.0, 10.0), |_| {
            node_at(10.0).move_to(Point::new(5.0, 5.0))
        });

        assert_eq!(node.bounds().x, 5.0);

        let reused = tree.layout_with_cache(limits(10.0, 10.0), |_| node_at(999.0));

        assert_eq!(
            reused.bounds().x,
            5.0,
            "the cached node's own position is preserved"
        );
    }
}

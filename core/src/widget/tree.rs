//! Store internal widget state in a state tree to ensure continuity.
use crate::layout;
use crate::widget::Id;
use crate::{Rectangle, Transformation, Widget, semantics};

use std::borrow::{Borrow, BorrowMut};

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
    /// Lazily allocated metadata for retained layout, paint, and semantics.
    /// Keeping this cold state out-of-line makes the common tree traversal
    /// representation smaller without charging stateless nodes an allocation.
    cache: Option<Box<cache::Cache>>,

    /// This node's stable identity, for the lifetime of the [`Tree`].
    ///
    /// Minted once, when the node is created, and never regenerated:
    /// reconciliation (`diff_children_custom`) reuses existing [`Tree`]
    /// nodes in place rather than rebuilding them, so this survives
    /// ordinary view reconstruction exactly as a widget's own [`State`]
    /// does.
    ///
    /// That is precisely the guarantee `phase.md` Phase 13 requires --
    /// "semantic IDs remain stable across ordinary view reconstruction" --
    /// and it is why a widget can report semantics at all: an
    /// [`taim_core::semantics::Node`] must carry an identity, and a
    /// freshly-generated one per frame would make every node look new to
    /// an assistive technology on every repaint.
    ///
    /// A node whose position in the tree changes such that reconciliation
    /// cannot match it (a keyed list reordering without keys, say) gets a
    /// new [`Tree`] and therefore a new identity -- which is correct: it
    /// is, as far as the framework can tell, a different widget.
    id: Id,
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
            cache: None,
            id: Id::unique(),
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
            cache: None,
            id: Id::unique(),
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

        self.dirty.insert(DirtyFlags::SEMANTICS);
        new.borrow_mut().diff(self);
    }

    /// Returns this node's stable identity.
    ///
    /// Stable for the lifetime of the [`Tree`], which -- because
    /// reconciliation reuses nodes in place -- means stable across
    /// ordinary view reconstruction. This is the identity a widget's
    /// [`Widget::semantics`](crate::Widget::semantics) reports, and what
    /// lets an assistive technology recognise a control as *the same
    /// control* between frames.
    pub fn id(&self) -> &Id {
        &self.id
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
        self.cache.as_deref().and_then(|cache| cache.bounds)
    }

    /// Records the bounds produced by this node's most recent layout pass.
    pub fn set_cached_bounds(&mut self, bounds: Rectangle) {
        self.cache_mut().bounds = Some(bounds);
    }

    /// Returns the cached transform from this node's last paint pass, if any.
    pub fn cached_transform(&self) -> Option<Transformation> {
        self.cache.as_deref().and_then(|cache| cache.transform)
    }

    /// Records the transform in effect during this node's most recent
    /// paint pass.
    pub fn set_cached_transform(&mut self, transform: Transformation) {
        self.cache_mut().transform = Some(transform);
    }

    /// Returns the cached clip rectangle from this node's last paint
    /// pass, if any.
    pub fn cached_clip(&self) -> Option<Rectangle> {
        self.cache.as_deref().and_then(|cache| cache.clip)
    }

    /// Records the clip rectangle in effect during this node's most
    /// recent paint pass.
    pub fn set_cached_clip(&mut self, clip: Rectangle) {
        self.cache_mut().clip = Some(clip);
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
        let key = limits.key();

        if !self.dirty.contains(DirtyFlags::LAYOUT)
            && let Some((cached_key, cached_node)) = self
                .cache
                .as_deref()
                .and_then(|cache| cache.layout.as_ref())
            && *cached_key == key
        {
            return cached_node.clone();
        }

        let node = compute(self);

        self.cache_mut().layout = Some((key, node.clone()));
        self.dirty.remove(DirtyFlags::LAYOUT);

        node
    }

    /// Computes this node's own, non-recursive [`semantics::Node`],
    /// reusing the last computed result instead of calling `compute` when
    /// [`DirtyFlags::SEMANTICS`] is not set.
    ///
    /// Mirrors [`Tree::layout_with_cache`]'s contract, minus the
    /// `limits`-equality check: a node's semantics do not vary with
    /// layout constraints the way its measured size does, so
    /// [`DirtyFlags::SEMANTICS`] alone determines reuse. A caller (or the
    /// container assembling a full semantic tree, see
    /// `taim_runtime::semantics`) is responsible for calling
    /// [`Tree::mark_dirty`] with [`DirtyFlags::SEMANTICS`] whenever
    /// something that would change this node's own reported semantics
    /// changes (its label, value, state, ...); [`Tree`] cannot detect
    /// that on its own, which is why a fresh [`Tree`] starts with
    /// [`DirtyFlags::SEMANTICS`] already set.
    ///
    /// This method never recurses into children -- composing a full
    /// subtree from per-node results is `taim_runtime::semantics`'s job,
    /// exactly as `hit_test`'s `Hoverable` adapter composes a path from
    /// per-node `contains` checks rather than `Tree` walking children
    /// itself.
    pub fn semantics_with_cache(
        &mut self,
        compute: impl FnOnce(&mut Self) -> Option<semantics::Node>,
    ) -> Option<semantics::Node> {
        if !self.dirty.contains(DirtyFlags::SEMANTICS)
            && let Some(cached) = self
                .cache
                .as_deref()
                .and_then(|cache| cache.semantics.as_ref())
        {
            return cached.clone();
        }

        let node = compute(self);

        self.cache_mut().semantics = Some(node.clone());
        self.dirty.remove(DirtyFlags::SEMANTICS);

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

    #[inline]
    fn cache_mut(&mut self) -> &mut cache::Cache {
        self.cache
            .get_or_insert_with(|| Box::new(cache::Cache::default()))
    }
}

mod cache;
mod reconcile;
mod types;

pub use reconcile::diff_children_custom_with_search;
pub use types::{DirtyFlags, Revision, State, Tag};

#[cfg(test)]
mod regression_tests;
#[cfg(test)]
mod tests;

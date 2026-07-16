//! Incremental assembly of a full [`semantics::Node`] tree from
//! per-widget, dirty-tracked contributions.
//!
//! [`build`] walks a tree of `(widget, Tree, Layout)` triples (adapted
//! via [`Semantic`], the same pattern `iced_runtime::hit_test::Hoverable`
//! uses for hit-testing) and produces the full [`semantics::Node`] an
//! assistive-technology adapter would consume. Each node's own,
//! non-recursive semantics are computed through
//! [`iced_core::widget::tree::Tree::semantics_with_cache`], so a subtree
//! whose [`DirtyFlags::SEMANTICS`] is clear contributes its last result
//! without calling back into the widget at all; only children are always
//! walked, since a parent cannot know whether *they* are dirty without
//! asking.
//!
//! Stable identity for each [`semantics::Node`] is already guaranteed by
//! [`semantics::Node::id`] being a required (non-optional) field --
//! Commit 4 does not need to invent identity, only avoid re-deriving a
//! node's own properties when nothing about it changed.
//!
//! [`DirtyFlags::SEMANTICS`]: iced_core::widget::tree::DirtyFlags::SEMANTICS

use crate::core::semantics;

/// Something that can contribute a [`semantics::Node`] for itself (via
/// [`Semantic::node`]) and enumerate its children for [`build`] to
/// recurse into.
///
/// Exists for the same reason `hit_test::Hoverable` does: so [`build`]
/// can walk a tree of widget/state/layout triples without depending on
/// `iced_core::Widget`'s full, renderer-generic signature. A caller
/// adapts its own widget/tree/layout triples to it (see
/// `tests/semantics/incremental.rs` for a full example against real
/// widgets).
pub trait Semantic {
    /// Computes this node's own, non-recursive [`semantics::Node`] (not
    /// including children), reusing a cached result when this node's
    /// [`iced_core::widget::tree::Tree`] is not semantics-dirty.
    ///
    /// Returns `None` if this widget has no semantics of its own (the
    /// default for every widget today); [`build`] still recurses into
    /// its children regardless, since a semantics-less container (e.g.
    /// a plain `row`) may still have meaningful descendants.
    fn node(&mut self) -> Option<semantics::Node>;

    /// Returns the number of children this node has.
    fn child_count(&self) -> usize;

    /// Returns the `index`-th child, for recursing into. Must return the
    /// same value every time for the same `index` during a single
    /// [`build`] call.
    fn child(&mut self, index: usize) -> Self
    where
        Self: Sized;
}

/// Builds the forest of [`semantics::Node`]s rooted at `root`: usually
/// one node (when `root` reports its own semantics), zero (when neither
/// `root` nor any descendant reports anything), or more than one (when
/// `root` itself is transparent -- e.g. a plain `row` -- but several of
/// its children report semantics independently, with nothing above
/// `root` to gather them under a single node).
///
/// A node with its own semantics absorbs its semantics-reporting
/// descendants as [`semantics::Node::children`]; a node with no
/// semantics of its own is transparent -- its semantics-reporting
/// descendants are hoisted to whichever reporting ancestor is closest,
/// since a screen reader has nothing to attach a non-reporting
/// container's descendants to otherwise. `build` is the shape a real
/// caller (an accessibility tree consumer) uses at the true root of a
/// widget tree; [`Semantic`] adapters recursing into their own children
/// use the internal, single-node [`build_one`] instead, since an
/// intermediate level always has a definite parent to flatten into.
pub fn build<T: Semantic>(root: &mut T) -> Vec<semantics::Node> {
    match build_one(root) {
        Some(node) => vec![node],
        None => collect_children(root),
    }
}

/// Builds a single [`semantics::Node`] for `node` (including its
/// semantics-reporting descendants as children), or `None` if neither
/// `node` nor any of its descendants report any semantics.
fn build_one<T: Semantic>(node: &mut T) -> Option<semantics::Node> {
    let own = node.node();
    let children = collect_children(node);

    match own {
        Some(mut node) => {
            node.children = children;
            Some(node)
        }
        None if children.is_empty() => None,
        None => {
            // No semantics of our own, but our descendants have some:
            // there is no single node to attach them to here, so the
            // caller sees them via a flattened list one level up
            // ([`collect_children`]) instead of losing them -- or, if
            // this *is* the true root ([`build`]), as a forest with more
            // than one top-level node.
            None
        }
    }
}

/// Collects the [`semantics::Node`]s contributed by `node`'s children,
/// flattening through any child that reports no semantics of its own so
/// its own reporting descendants are not lost.
fn collect_children<T: Semantic>(node: &mut T) -> Vec<semantics::Node> {
    let mut children = Vec::new();

    for index in 0..node.child_count() {
        let mut child = node.child(index);

        match build_one(&mut child) {
            Some(built) => children.push(built),
            None => children.extend(collect_children(&mut child)),
        }
    }

    children
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rectangle;
    use crate::core::widget::Id;
    use crate::core::widget::Tree;
    use crate::core::widget::tree::DirtyFlags;

    /// A minimal synthetic tree for exercising [`build`] without any
    /// dependency on `iced_core::Widget`.
    ///
    /// Stored as a flat arena (rather than `Vec<Node>` children owning
    /// their own subtrees directly) so [`Adapter`] can hold plain
    /// indices instead of borrows, sidestepping the self-referential
    /// borrow that a `&'a mut Node`-holding adapter would otherwise need
    /// to reborrow through on every [`Semantic::child`] call.
    struct Node {
        id: Id,
        name: &'static str,
        reports: bool,
        // `RefCell` rather than a plain field: `Adapter` holds a shared
        // `&'a Arena` (see its doc comment for why), so mutating a
        // node's own cache through `Tree::semantics_with_cache` needs
        // interior mutability.
        tree: std::cell::RefCell<Tree>,
        children: Vec<usize>,
    }

    struct Arena {
        nodes: Vec<Node>,
        calls: std::rc::Rc<std::cell::RefCell<Vec<&'static str>>>,
    }

    impl Arena {
        fn new() -> Self {
            Self {
                nodes: Vec::new(),
                calls: std::rc::Rc::new(std::cell::RefCell::new(Vec::new())),
            }
        }

        fn insert(&mut self, name: &'static str, reports: bool, children: Vec<usize>) -> usize {
            self.nodes.push(Node {
                id: Id::unique(),
                name,
                reports,
                tree: std::cell::RefCell::new(Tree::with_children(Vec::new())),
                children,
            });

            self.nodes.len() - 1
        }

        fn leaf(&mut self, name: &'static str) -> usize {
            self.insert(name, true, Vec::new())
        }

        fn mark_dirty(&self, index: usize, flags: DirtyFlags) {
            self.nodes[index].tree.borrow_mut().mark_dirty(flags);
        }
    }

    /// Adapts an [`Arena`] node to [`Semantic`].
    ///
    /// Holds a *shared* reference to the whole arena (rather than a
    /// `&mut Node` to just this node, as a first draft of this test
    /// tried): `Semantic::child` must produce a fresh `Self` on every
    /// call while the caller ([`build`]/`collect_children`) holds `&mut
    /// self` on the parent, and a `&'a mut`-holding adapter cannot
    /// reborrow itself for a lifetime shorter than `'a` and still claim
    /// to return `Self = Adapter<'a>`. A shared reference has no such
    /// problem (it can be copied freely); node-local mutation goes
    /// through `Node::tree`'s `RefCell` instead.
    #[derive(Clone, Copy)]
    struct Adapter<'a> {
        arena: &'a Arena,
        index: usize,
    }

    impl<'a> Semantic for Adapter<'a> {
        fn node(&mut self) -> Option<semantics::Node> {
            let node = &self.arena.nodes[self.index];
            let name = node.name;
            let reports = node.reports;
            let id = node.id.clone();
            let calls = self.arena.calls.clone();

            node.tree.borrow_mut().semantics_with_cache(|_| {
                calls.borrow_mut().push(name);

                reports.then(|| {
                    semantics::Node::new(
                        id,
                        Rectangle::new(
                            crate::core::Point::ORIGIN,
                            crate::core::Size::new(10.0, 10.0),
                        ),
                        semantics::Role::StaticText,
                    )
                    .with_name(name)
                })
            })
        }

        fn child_count(&self) -> usize {
            self.arena.nodes[self.index].children.len()
        }

        fn child(&mut self, index: usize) -> Self {
            Adapter {
                arena: self.arena,
                index: self.arena.nodes[self.index].children[index],
            }
        }
    }

    fn root_adapter(arena: &Arena, root: usize) -> Adapter<'_> {
        Adapter { arena, index: root }
    }

    #[test]
    fn a_single_reporting_leaf_produces_its_own_node() {
        let mut arena = Arena::new();
        let root = arena.leaf("root");

        let built = build(&mut root_adapter(&arena, root));
        let built = built.into_iter().next().expect("root reports semantics");

        assert_eq!(built.name.as_deref(), Some("root"));
        assert!(built.children.is_empty());
    }

    #[test]
    fn a_non_reporting_root_with_no_reporting_descendants_yields_none() {
        let mut arena = Arena::new();
        let root = arena.insert("root", false, Vec::new());

        assert!(build(&mut root_adapter(&arena, root)).is_empty());
    }

    #[test]
    fn a_non_reporting_root_with_multiple_reporting_children_yields_a_forest() {
        let mut arena = Arena::new();
        let a = arena.leaf("a");
        let b = arena.leaf("b");
        // The root itself reports nothing (unlike every other test in
        // this module): there is no ancestor above it to flatten `a`
        // and `b` into, so `build` must surface both as top-level
        // siblings instead of discarding them the way an early version
        // of this function did.
        let root = arena.insert("root", false, vec![a, b]);

        let built = build(&mut root_adapter(&arena, root));

        assert_eq!(
            built
                .iter()
                .map(|node| node.name.as_deref().unwrap())
                .collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    #[test]
    fn reporting_children_become_child_nodes_in_order() {
        let mut arena = Arena::new();
        let a = arena.leaf("a");
        let b = arena.leaf("b");
        let root = arena.insert("root", true, vec![a, b]);

        let built = build(&mut root_adapter(&arena, root));
        let built = built.into_iter().next().unwrap();

        assert_eq!(
            built
                .children
                .iter()
                .map(|c| c.name.as_deref().unwrap())
                .collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    #[test]
    fn a_non_reporting_containers_reporting_descendants_are_hoisted_to_the_nearest_reporting_ancestor()
     {
        let mut arena = Arena::new();
        // root (reports) -> wrapper (does not report) -> leaf (reports)
        let inner_leaf = arena.leaf("leaf");
        let wrapper = arena.insert("wrapper", false, vec![inner_leaf]);
        let root = arena.insert("root", true, vec![wrapper]);

        let built = build(&mut root_adapter(&arena, root));
        let built = built.into_iter().next().unwrap();

        assert_eq!(
            built
                .children
                .iter()
                .map(|c| c.name.as_deref().unwrap())
                .collect::<Vec<_>>(),
            vec!["leaf"],
            "the non-reporting wrapper must not appear, and must not swallow its reporting child"
        );
    }

    #[test]
    fn a_clean_subtree_does_not_recompute_its_own_semantics_on_a_second_build() {
        let mut arena = Arena::new();
        let a = arena.leaf("a");
        let b = arena.leaf("b");
        let root = arena.insert("root", true, vec![a, b]);

        let _ = build(&mut root_adapter(&arena, root));
        arena.calls.borrow_mut().clear();

        let _ = build(&mut root_adapter(&arena, root));

        assert!(
            arena.calls.borrow().is_empty(),
            "every node was clean; nothing should have recomputed its own semantics, got {:?}",
            arena.calls.borrow()
        );
    }

    #[test]
    fn marking_one_leaf_dirty_only_recomputes_that_leaf() {
        let mut arena = Arena::new();
        let a = arena.leaf("a");
        let b = arena.leaf("b");
        let root = arena.insert("root", true, vec![a, b]);

        let _ = build(&mut root_adapter(&arena, root));
        arena.calls.borrow_mut().clear();

        arena.mark_dirty(a, DirtyFlags::SEMANTICS);

        let built = build(&mut root_adapter(&arena, root));
        let built = built.into_iter().next().unwrap();

        assert_eq!(
            *arena.calls.borrow(),
            vec!["a"],
            "only the marked leaf's own semantics should recompute"
        );
        assert_eq!(
            built
                .children
                .iter()
                .map(|c| c.name.as_deref().unwrap())
                .collect::<Vec<_>>(),
            vec!["a", "b"],
            "the clean sibling's cached result must still be present in the assembled tree"
        );
    }

    #[test]
    fn changing_one_leafs_reported_name_is_reflected_after_marking_it_dirty() {
        let mut arena = Arena::new();
        let a = arena.leaf("a");
        let root = arena.insert("root", true, vec![a]);

        let _ = build(&mut root_adapter(&arena, root));

        arena.nodes[a].name = "renamed";
        arena.mark_dirty(a, DirtyFlags::SEMANTICS);

        let built = build(&mut root_adapter(&arena, root));
        let built = built.into_iter().next().unwrap();

        assert_eq!(built.children[0].name.as_deref(), Some("renamed"));
    }

    #[test]
    fn every_nodes_id_is_stable_across_rebuilds_of_a_clean_tree() {
        let mut arena = Arena::new();
        let a = arena.leaf("a");
        let root = arena.insert("root", true, vec![a]);

        let first = build(&mut root_adapter(&arena, root))
            .into_iter()
            .next()
            .unwrap();
        let second = build(&mut root_adapter(&arena, root))
            .into_iter()
            .next()
            .unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(first.children[0].id, second.children[0].id);
    }
}

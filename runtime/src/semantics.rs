//! Incremental assembly of a full [`semantics::Node`] tree from
//! per-widget, dirty-tracked contributions.
//!
//! [`build`] walks a tree of `(widget, Tree, Layout)` triples (adapted
//! via [`aura_core::widget::visit::Widgets`], the same hook
//! `aura_runtime::hit_test` rides on) and produces the full
//! [`semantics::Node`] forest an assistive-technology adapter consumes. Each node's own,
//! non-recursive semantics are computed through
//! [`aura_core::widget::tree::Tree::semantics_with_cache`], so a subtree
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
//! [`DirtyFlags::SEMANTICS`]: aura_core::widget::tree::DirtyFlags::SEMANTICS

use crate::core::semantics;
use crate::core::widget::visit::Visit;

pub use crate::core::widget::visit::{Visit as Semantic, Widgets};

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
pub fn build(root: &mut dyn Visit) -> Vec<semantics::Node> {
    build_subtree(root)
}

/// Builds the forest contributed by `node` and everything beneath it:
/// one node when `node` reports its own semantics (absorbing its
/// reporting descendants as [`semantics::Node::children`]), and
/// otherwise its descendants' forest hoisted up, since a transparent
/// container gives a screen reader nothing to attach them to.
///
/// Visits every descendant exactly once. The previous shape of this walk
/// asked each child for a single built node and, on `None`, walked the
/// same child a second time to collect what it had already computed and
/// thrown away -- doubling the work at every transparent level, so an
/// ordinary `container(row(column(..)))` nest cost exponential time in
/// its depth. Hoisting inside one pass makes that impossible to
/// reintroduce: there is no longer a discarded result to recompute.
fn build_subtree(node: &mut dyn Visit) -> Vec<semantics::Node> {
    let own = node.semantics();
    let children = collect_children(node);

    match own {
        Some(mut node) => {
            if node.name.is_none() && names_from_content(node.role) {
                node.name = name_from_content(&children);
            }

            node.children = children;
            vec![node]
        }
        None => children,
    }
}

/// Whether a node of this [`semantics::Role`] takes its accessible name
/// from its own content when it has none of its own.
///
/// This is ARIA's "name from content" rule, and AT-SPI/UIA behave the
/// same way. It exists because a control's label is often not the
/// control's own data but a *child widget*: `button("Save")` is a button
/// wrapping a `text`, and the button has no `label` field to report from
/// — unlike `checkbox`/`radio`/`toggler`/`icon_button`, which own theirs
/// and set it directly.
///
/// A [`Role::Group`] is deliberately excluded, as ARIA excludes it: a
/// group named after everything inside it would announce a whole
/// subtree's prose as one label.
///
/// [`Role::Group`]: semantics::Role::Group
fn names_from_content(role: semantics::Role) -> bool {
    use semantics::Role;

    matches!(
        role,
        Role::Button
            | Role::Link
            | Role::Tab
            | Role::MenuItem
            | Role::ListItem
            | Role::Checkbox
            | Role::RadioButton
    )
}

/// Joins the names of `children` and their descendants, in document
/// order, into an accessible name.
///
/// Returns `None` rather than an empty string when there is nothing to
/// say: an unlabeled control must stay visibly unlabeled, not acquire a
/// name of `""` that reads as "named, with an empty name".
fn name_from_content(children: &[semantics::Node]) -> Option<crate::core::SmolStr> {
    fn collect(nodes: &[semantics::Node], out: &mut Vec<String>) {
        for node in nodes {
            // A nested control names itself; absorbing its name into an
            // ancestor's would announce it twice, and out of context.
            if names_from_content(node.role) {
                continue;
            }

            if let Some(name) = &node.name {
                out.push(name.to_string());
            }

            collect(&node.children, out);
        }
    }

    let mut parts = Vec::new();
    collect(children, &mut parts);

    if parts.is_empty() {
        return None;
    }

    Some(crate::core::SmolStr::new(parts.join(" ")))
}

/// Collects the [`semantics::Node`]s contributed by `node`'s children,
/// flattening through any child that reports no semantics of its own so
/// its own reporting descendants are not lost.
fn collect_children(node: &mut dyn Visit) -> Vec<semantics::Node> {
    let mut children = Vec::new();

    node.children(&mut |child| children.extend(build_subtree(child)));

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
    /// dependency on `aura_core::Widget`.
    ///
    /// Stored as a flat arena (rather than `Vec<Node>` children owning
    /// their own subtrees directly) so [`Adapter`] can hold plain
    /// indices instead of borrows, sidestepping the self-referential
    /// borrow a `&'a mut Node`-holding adapter would need.
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
    /// Holds a *shared* reference to the whole arena rather than a `&mut
    /// Node` to just this node: an adapter must be able to hand out
    /// children while the caller ([`build`]/`collect_children`) holds
    /// `&mut self` on the parent. A shared reference can be copied
    /// freely; node-local mutation goes through `Node::tree`'s `RefCell`
    /// instead.
    #[derive(Clone, Copy)]
    struct Adapter<'a> {
        arena: &'a Arena,
        index: usize,
    }

    impl<'a> Visit for Adapter<'a> {
        fn contains(&self, _point: crate::core::Point) -> bool {
            // These tests exercise semantic assembly only; the
            // hit-testing half of `Visit` is covered against real
            // widgets in `tests/events/`.
            false
        }

        fn bounds(&self) -> Rectangle {
            Rectangle::new(
                crate::core::Point::ORIGIN,
                crate::core::Size::new(10.0, 10.0),
            )
        }

        fn semantics(&mut self) -> Option<semantics::Node> {
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

        fn children(&mut self, visit: &mut dyn FnMut(&mut dyn Visit)) {
            let arena = self.arena;

            for &child in &arena.nodes[self.index].children {
                visit(&mut Adapter {
                    arena,
                    index: child,
                });
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

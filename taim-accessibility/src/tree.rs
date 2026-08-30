//! The retained semantic tree and its incremental diff.
//!
//! # The correctness bar
//!
//! `phase.md` Phase 13's exit gate is "incremental semantic output
//! matches full rebuild oracle". [`Tree::diff`] is the incremental path;
//! [`Tree::from_forest`] is the oracle. The bar is exact, and it is
//! stated as a property rather than a set of examples:
//!
//! > For any two trees `a` and `b`, applying `a.diff(&b)` to `a` must
//! > produce a tree equal to `b`.
//!
//! `tests/accessibility/tree.rs` proves it over hand-built cases *and*
//! a generated matrix of mutations. That equality is what lets an adapter
//! trust the cheap path: a diff is not "close enough", it is
//! indistinguishable from having rebuilt everything.

use crate::node::{Entry, Order};

use taim_core::semantics::Node;
use taim_core::widget::Id;

use rustc_hash::FxHashMap;

/// One difference between two [`Tree`]s.
///
/// The variants are deliberately narrower than "something changed": an
/// adapter reacts very differently to a node appearing (announce it) than
/// to one merely moving (update geometry, stay quiet), and collapsing
/// those into one "updated" case would force every adapter to re-derive
/// the distinction itself.
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    /// A node that did not exist before.
    Added(Entry),
    /// A node that no longer exists.
    Removed(Id),
    /// A node whose own properties changed.
    Updated {
        /// The node's identity.
        id: Id,
        /// Its new properties.
        entry: Entry,
    },
    /// A node whose *geometry alone* changed.
    ///
    /// Separated from [`Change::Updated`] because a scroll moves many
    /// nodes without changing anything worth announcing; an adapter can
    /// forward these as cheap geometry updates. This is `phase.md`'s
    /// "dirty visual bounds update semantics without rebuilding clean
    /// subtrees", seen from the diff's side.
    Moved {
        /// The node's identity.
        id: Id,
        /// Its new screen-space bounds.
        bounds: taim_core::Rectangle,
    },
    /// A node whose position in the tree changed (new parent, or a new
    /// traversal order under the same parent) while its own properties
    /// stayed the same.
    Reordered {
        /// The node's identity.
        id: Id,
        /// Its new parent, or `None` if it is now a root.
        parent: Option<Id>,
        /// Its new traversal position.
        order: Order,
    },
}

/// A retained, indexed semantic forest.
///
/// Built from the output of `taim_runtime::semantics::build`, which is
/// already incremental at the widget level (clean subtrees contribute a
/// cached node without being asked again). This adds identity indexing,
/// traversal order, and diffing on top.
#[derive(Debug, Clone, Default)]
pub struct Tree {
    entries: FxHashMap<Id, Entry>,
}

impl Tree {
    /// An empty tree.
    pub fn new() -> Self {
        Self::default()
    }

    /// Flattens and indexes a semantic forest.
    ///
    /// This is the **oracle**: a complete rebuild from scratch, with no
    /// reference to any previous tree. [`Tree::diff`]'s output applied to
    /// an old tree must be indistinguishable from calling this on the new
    /// forest.
    pub fn from_forest(forest: &[Node]) -> Self {
        let mut tree = Self::new();

        for (index, root) in forest.iter().enumerate() {
            tree.absorb(root, None, Order::root(index));
        }

        tree
    }

    fn absorb(&mut self, node: &Node, parent: Option<Id>, order: Order) {
        let entry = Entry::new(node, parent, order.clone());
        let id = entry.id.clone();

        let _ = self.entries.insert(id.clone(), entry);

        for (index, child) in node.children.iter().enumerate() {
            self.absorb(child, Some(id.clone()), order.child(index));
        }
    }

    /// Returns the entry for `id`, if present.
    pub fn get(&self, id: &Id) -> Option<&Entry> {
        self.entries.get(id)
    }

    /// The number of nodes in the tree.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the tree holds no nodes.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every entry, in deterministic traversal (depth-first document)
    /// order.
    ///
    /// This is the order a screen reader reads and Tab moves through, and
    /// it is a total order: [`Order`] sorts parents before children and
    /// earlier siblings before later ones, so no tie-breaking is needed.
    pub fn in_order(&self) -> Vec<&Entry> {
        let mut entries: Vec<&Entry> = self.entries.values().collect();
        entries.sort_by(|a, b| a.order.cmp(&b.order));
        entries
    }

    /// Returns the minimal set of [`Change`]s that turn `self` into
    /// `next`.
    ///
    /// Changes are emitted in a deterministic order (removals first, then
    /// the rest in traversal order) so two runs over the same pair of
    /// trees produce byte-identical output -- an adapter's announcements
    /// must not depend on hash iteration order.
    pub fn diff(&self, next: &Tree) -> Vec<Change> {
        let mut changes = Vec::new();

        // Removals first: an adapter should retire vanished nodes before
        // being told about whatever replaced them, or it may briefly hold
        // two nodes claiming the same screen region.
        let mut removed: Vec<&Entry> = self
            .entries
            .values()
            .filter(|entry| !next.entries.contains_key(&entry.id))
            .collect();
        removed.sort_by(|a, b| a.order.cmp(&b.order));

        changes.extend(
            removed
                .into_iter()
                .map(|entry| Change::Removed(entry.id.clone())),
        );

        for entry in next.in_order() {
            match self.entries.get(&entry.id) {
                None => changes.push(Change::Added(entry.clone())),
                Some(previous) => {
                    let moved_in_tree =
                        previous.parent != entry.parent || previous.order != entry.order;

                    if previous.only_bounds_differ_from(entry) {
                        changes.push(Change::Moved {
                            id: entry.id.clone(),
                            bounds: entry.bounds,
                        });
                    } else if previous.differs_from(entry) {
                        changes.push(Change::Updated {
                            id: entry.id.clone(),
                            entry: entry.clone(),
                        });
                    }

                    if moved_in_tree {
                        changes.push(Change::Reordered {
                            id: entry.id.clone(),
                            parent: entry.parent.clone(),
                            order: entry.order.clone(),
                        });
                    }
                }
            }
        }

        changes
    }

    /// Applies `changes` to this tree.
    ///
    /// The inverse of [`Tree::diff`], and the other half of the exit
    /// gate: `old.apply(old.diff(new))` must equal `Tree::from_forest`
    /// of the same input `new` was built from.
    pub fn apply(&mut self, changes: &[Change]) {
        for change in changes {
            match change {
                Change::Added(entry) => {
                    let _ = self.entries.insert(entry.id.clone(), entry.clone());
                }
                Change::Removed(id) => {
                    let _ = self.entries.remove(id);
                }
                Change::Updated { id, entry } => {
                    let _ = self.entries.insert(id.clone(), entry.clone());
                }
                Change::Moved { id, bounds } => {
                    if let Some(entry) = self.entries.get_mut(id) {
                        entry.bounds = *bounds;
                    }
                }
                Change::Reordered { id, parent, order } => {
                    if let Some(entry) = self.entries.get_mut(id) {
                        entry.parent = parent.clone();
                        entry.order = order.clone();
                    }
                }
            }
        }
    }
}

impl PartialEq for Tree {
    /// Two trees are equal when they hold the same nodes with the same
    /// properties *and* the same structure.
    ///
    /// Hash iteration order is irrelevant: equality is by content, which
    /// is exactly what "indistinguishable from a full rebuild" has to
    /// mean.
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use taim_core::semantics::{Actions, Role, State};
    use taim_core::{Rectangle, Size};

    fn node(id: &Id, role: Role) -> Node {
        Node::new(
            id.clone(),
            Rectangle::with_size(Size::new(10.0, 10.0)),
            role,
        )
    }

    #[test]
    fn a_rebuilt_tree_equals_itself_and_indexes_every_node() {
        let root = Id::unique();
        let child = Id::unique();

        let forest = vec![node(&root, Role::Group).with_child(node(&child, Role::Button))];

        let tree = Tree::from_forest(&forest);

        assert_eq!(tree.len(), 2);
        assert_eq!(tree.get(&child).unwrap().parent, Some(root.clone()));
        assert_eq!(tree.get(&root).unwrap().parent, None);
        assert_eq!(tree, Tree::from_forest(&forest));
    }

    #[test]
    fn in_order_is_depth_first_document_order() {
        let root = Id::unique();
        let first = Id::unique();
        let second = Id::unique();

        let forest = vec![
            node(&root, Role::Group)
                .with_child(node(&first, Role::Button))
                .with_child(node(&second, Role::Button)),
        ];

        let tree = Tree::from_forest(&forest);
        let ids: Vec<Id> = tree.in_order().iter().map(|e| e.id.clone()).collect();

        assert_eq!(ids, vec![root, first, second]);
    }

    #[test]
    fn an_identical_tree_produces_no_changes() {
        let id = Id::unique();
        let forest = vec![node(&id, Role::Button)];

        let a = Tree::from_forest(&forest);
        let b = Tree::from_forest(&forest);

        assert!(
            a.diff(&b).is_empty(),
            "nothing changed, so nothing to report"
        );
    }

    #[test]
    fn a_moved_node_reports_geometry_only() {
        let id = Id::unique();

        let before = Tree::from_forest(&[node(&id, Role::Button)]);
        let after = Tree::from_forest(&[Node::new(
            id.clone(),
            Rectangle::with_size(Size::new(99.0, 99.0)),
            Role::Button,
        )]);

        let changes = before.diff(&after);

        assert_eq!(changes.len(), 1);
        assert!(
            matches!(&changes[0], Change::Moved { id: changed, .. } if *changed == id),
            "a pure geometry change must not read as a full update: {changes:?}"
        );
    }

    #[test]
    fn a_renamed_node_reports_a_full_update() {
        let id = Id::unique();

        let before = Tree::from_forest(&[node(&id, Role::Button).with_name("Save")]);
        let after = Tree::from_forest(&[node(&id, Role::Button).with_name("Saved")]);

        let changes = before.diff(&after);

        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], Change::Updated { .. }));
    }

    #[test]
    fn removals_are_reported_before_anything_else() {
        let stays = Id::unique();
        let goes = Id::unique();
        let arrives = Id::unique();

        let before = Tree::from_forest(&[node(&stays, Role::Button), node(&goes, Role::Button)]);
        let after = Tree::from_forest(&[node(&stays, Role::Button), node(&arrives, Role::Button)]);

        let changes = before.diff(&after);

        assert!(
            matches!(&changes[0], Change::Removed(id) if *id == goes),
            "a vanished node must be retired first: {changes:?}"
        );
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, Change::Added(e) if e.id == arrives))
        );
    }

    #[test]
    fn a_diff_is_deterministic_across_runs() {
        // Hash iteration order must never reach the output: an adapter's
        // announcements would otherwise vary run to run.
        let ids: Vec<Id> = (0..16).map(|_| Id::unique()).collect();

        let mut root = node(&ids[0], Role::Group);
        for id in &ids[1..] {
            root = root.with_child(node(id, Role::Button));
        }

        let before = Tree::new();
        let after = Tree::from_forest(&[root]);

        let first = before.diff(&after);
        let second = before.diff(&after);

        assert_eq!(first, second);
    }

    #[test]
    fn state_changes_are_updates_not_moves() {
        let id = Id::unique();

        let before = Tree::from_forest(&[node(&id, Role::Checkbox)]);
        let after = Tree::from_forest(&[node(&id, Role::Checkbox).with_state(State::CHECKED)]);

        let changes = before.diff(&after);

        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], Change::Updated { .. }));
    }

    #[test]
    fn a_secure_field_carries_no_value_into_the_retained_tree() {
        // The redaction must survive retention: an entry built from a
        // SecureField must hold no value even though it was handed one.
        let id = Id::unique();
        let secure = node(&id, Role::SecureField)
            .with_value(taim_core::semantics::Value::Text("hunter2".into()));

        let tree = Tree::from_forest(&[secure]);

        assert_eq!(tree.get(&id).unwrap().value, None);
    }

    #[test]
    fn actions_survive_the_round_trip() {
        let id = Id::unique();
        let forest = vec![node(&id, Role::Button).with_actions(Actions::ACTIVATE | Actions::FOCUS)];

        let tree = Tree::from_forest(&forest);

        assert_eq!(
            tree.get(&id).unwrap().actions,
            Actions::ACTIVATE | Actions::FOCUS
        );
    }
}

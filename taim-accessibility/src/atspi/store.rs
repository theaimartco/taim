//! The shared snapshot an AT-SPI object server answers from.
//!
//! # Why a store, and not just the tree
//!
//! [`Adapter`](crate::adapter::Adapter) is **synchronous** — it is called
//! from the UI thread, in the middle of a frame, right after
//! [`Tree::diff`](crate::tree::Tree::diff). A D-Bus object server is
//! **asynchronous**, and answers questions whenever a screen reader
//! happens to ask, which is emphatically not on our frame boundary.
//!
//! [`Store`] is the seam. The UI thread writes snapshots into it; the bus
//! task reads them. Neither blocks on the other, and a screen reader
//! asking `GetChildren` mid-frame gets the last consistent snapshot
//! rather than a half-applied one.
//!
//! # Identity on the wire
//!
//! AT-SPI addresses everything by object path. Taim addresses everything
//! by [`Id`]. [`Store`] owns that correspondence, and it must be
//! **stable**: a path is a promise to a screen reader that it can ask
//! about that object again. A path reused for a different node after a
//! removal would answer questions about the wrong thing, so paths are
//! minted from a counter that never rewinds and are never recycled.

use std::collections::hash_map::Entry as MapEntry;
use std::sync::{Arc, Mutex};

use taim_core::semantics::{Actions, Role, State};
use taim_core::widget::Id;
use taim_core::{Rectangle, SmolStr};
use rustc_hash::FxHashMap;

use crate::node::Entry;
use crate::tree::Change;

/// The object path prefix every node of ours lives under.
///
/// The path space is ours to choose — a real GTK application serves its
/// tree from `/org/gnome/Nautilus/a11y/...` — but keeping the
/// conventional `/org/a11y/atspi/accessible/` prefix means anything
/// eyeballing the bus sees what it expects.
pub const PATH_PREFIX: &str = "/org/a11y/atspi/accessible/";

/// The path of the application root, fixed by AT-SPI convention.
pub const ROOT_PATH: &str = "/org/a11y/atspi/accessible/root";

/// The null object reference: what `Parent` reports when there is none,
/// and what a lookup miss answers with.
///
/// AT-SPI has no "absent" object reference; the convention is the
/// well-known null path, which every reader understands.
pub const NULL_PATH: &str = "/org/a11y/atspi/null";

/// One node, flattened into exactly what the bus interfaces need.
///
/// Deliberately not a reference into a [`Tree`](crate::tree::Tree): the
/// bus task must be able to answer without borrowing anything the UI
/// thread might be mutating.
#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    /// This node's object path.
    pub path: String,
    /// The parent's path, or [`ROOT_PATH`] for a top-level node.
    pub parent: String,
    /// Children's paths, in traversal order.
    pub children: Vec<String>,
    /// This node's index among its parent's children.
    pub index_in_parent: i32,
    /// Screen-space bounds.
    pub bounds: Rectangle,
    /// What kind of element this is.
    pub role: Role,
    /// Current state flags.
    pub state: State,
    /// Accessible name.
    pub name: Option<SmolStr>,
    /// Longer description.
    pub description: Option<SmolStr>,
    /// What this node can do.
    pub actions: Actions,
}

/// A snapshot of the whole exposed tree, addressable by object path.
#[derive(Debug, Default)]
pub struct Snapshot {
    objects: FxHashMap<String, Object>,
    roots: Vec<String>,
}

impl Snapshot {
    /// The object at `path`, if it exists.
    pub fn get(&self, path: &str) -> Option<&Object> {
        self.objects.get(path)
    }

    /// The paths of every top-level node, in order — the application
    /// root's children.
    pub fn roots(&self) -> &[String] {
        &self.roots
    }

    /// How many objects are exposed.
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Whether nothing is exposed.
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}

/// The shared, mutable correspondence between Taim's tree and AT-SPI's
/// object paths.
///
/// Cheap to clone; every clone shares one snapshot.
#[derive(Debug, Clone, Default)]
pub struct Store {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Debug, Default)]
struct Inner {
    paths: FxHashMap<Id, String>,
    entries: FxHashMap<Id, Entry>,
    snapshot: Arc<Snapshot>,
    next_path: u64,
}

impl Store {
    /// An empty [`Store`].
    pub fn new() -> Self {
        Self::default()
    }

    /// The current snapshot.
    ///
    /// Returns an [`Arc`] rather than a borrow so the bus task can answer
    /// a series of questions against one consistent view without holding
    /// the lock — and without blocking the UI thread from installing the
    /// next frame's snapshot meanwhile.
    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.lock().snapshot.clone()
    }

    /// The object path for `id`, if it is exposed.
    pub fn path_of(&self, id: &Id) -> Option<String> {
        self.lock().paths.get(id).cloned()
    }

    /// The [`Id`] at `path`, if any — for routing an incoming action back
    /// to the widget that claimed it.
    pub fn id_at(&self, path: &str) -> Option<Id> {
        self.lock()
            .paths
            .iter()
            .find(|(_, candidate)| candidate.as_str() == path)
            .map(|(id, _)| id.clone())
    }

    /// Applies a batch of [`Change`]s and installs a fresh snapshot.
    ///
    /// The batch is [`Tree::diff`](crate::tree::Tree::diff)'s output:
    /// removals first, then the rest in traversal order.
    pub fn apply(&self, changes: &[Change]) {
        let mut inner = self.lock();

        for change in changes {
            match change {
                Change::Added(entry) => {
                    inner.mint(&entry.id);
                    let _ = inner.entries.insert(entry.id.clone(), entry.clone());
                }
                Change::Removed(id) => {
                    let _ = inner.entries.remove(id);
                    // The path stays reserved. See the module docs: a
                    // recycled path would answer a screen reader's
                    // questions about the wrong node.
                    let _ = inner.paths.remove(id);
                }
                Change::Updated { id, entry } => {
                    inner.mint(id);
                    let _ = inner.entries.insert(id.clone(), entry.clone());
                }
                Change::Moved { id, bounds } => {
                    if let Some(entry) = inner.entries.get_mut(id) {
                        entry.bounds = *bounds;
                    }
                }
                Change::Reordered { id, parent, order } => {
                    if let Some(entry) = inner.entries.get_mut(id) {
                        entry.parent = parent.clone();
                        entry.order = order.clone();
                    }
                }
            }
        }

        inner.rebuild();
    }

    /// Replaces everything with `entries` — the full-rebuild path, for a
    /// platform that asked for the whole tree rather than a diff.
    pub fn reset(&self, entries: &[Entry]) {
        let mut inner = self.lock();

        inner.entries.clear();

        for entry in entries {
            inner.mint(&entry.id);
            let _ = inner.entries.insert(entry.id.clone(), entry.clone());
        }

        inner.rebuild();
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // A poisoned lock means a panic while a snapshot was half-built.
        // Recovering the guard is right: the snapshot is rebuilt wholesale
        // on the next `apply`, so there is no torn state to inherit, and
        // taking down the UI thread because the bus task panicked would be
        // a worse failure than a stale frame of accessibility data.
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Inner {
    /// Assigns `id` a path if it does not have one.
    fn mint(&mut self, id: &Id) {
        if let MapEntry::Vacant(slot) = self.paths.entry(id.clone()) {
            let path = format!("{PATH_PREFIX}{}", self.next_path);
            self.next_path += 1;
            let _ = slot.insert(path);
        }
    }

    /// Rebuilds the snapshot from the current entries.
    ///
    /// Wholesale, rather than patched in place: parent/child links and
    /// indices are derived from the whole set, and one `Reordered` can
    /// change several nodes' indices. Rebuilding is O(n) per frame
    /// against a tree that is already only rebuilt when something
    /// changed — and it makes an inconsistent snapshot unrepresentable,
    /// which a patched one would not.
    fn rebuild(&mut self) {
        let mut ordered: Vec<&Entry> = self.entries.values().collect();
        ordered.sort_by(|a, b| a.order.cmp(&b.order));

        let mut objects = FxHashMap::default();
        let mut roots = Vec::new();
        let mut children_of: FxHashMap<String, Vec<String>> = FxHashMap::default();

        for entry in &ordered {
            let Some(path) = self.paths.get(&entry.id) else {
                continue;
            };

            let parent = match &entry.parent {
                Some(parent) => self
                    .paths
                    .get(parent)
                    .cloned()
                    .unwrap_or_else(|| ROOT_PATH.to_owned()),
                None => ROOT_PATH.to_owned(),
            };

            if entry.parent.is_none() {
                roots.push(path.clone());
            }

            children_of
                .entry(parent.clone())
                .or_default()
                .push(path.clone());

            let _ = objects.insert(
                path.clone(),
                Object {
                    path: path.clone(),
                    parent,
                    children: Vec::new(),
                    index_in_parent: 0,
                    bounds: entry.bounds,
                    role: entry.role,
                    state: entry.state,
                    name: entry.name.clone(),
                    description: entry.description.clone(),
                    actions: entry.actions,
                },
            );
        }

        for (parent, children) in &children_of {
            for (index, child) in children.iter().enumerate() {
                if let Some(object) = objects.get_mut(child) {
                    object.index_in_parent = index as i32;
                }
            }

            if let Some(object) = objects.get_mut(parent) {
                object.children = children.clone();
            }
        }

        self.snapshot = Arc::new(Snapshot { objects, roots });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::Order;

    fn order(indices: &[usize]) -> Order {
        indices
            .iter()
            .skip(1)
            .fold(Order::root(indices[0]), |order, index| order.child(*index))
    }

    fn entry(id: &Id, parent: Option<&Id>, indices: &[usize], name: &str) -> Entry {
        Entry {
            id: id.clone(),
            parent: parent.cloned(),
            order: order(indices),
            bounds: Rectangle::new(taim_core::Point::ORIGIN, taim_core::Size::new(10.0, 10.0)),
            role: Role::Button,
            state: State::empty(),
            name: Some(SmolStr::new(name)),
            description: None,
            value: None,
            actions: Actions::ACTIVATE,
            live: None,
        }
    }

    #[test]
    fn an_added_node_becomes_addressable() {
        let store = Store::new();
        let id = Id::unique();

        store.apply(&[Change::Added(entry(&id, None, &[0], "Save"))]);

        let path = store.path_of(&id).expect("an added node must get a path");
        let snapshot = store.snapshot();
        let object = snapshot.get(&path).expect("and be in the snapshot");

        assert_eq!(object.name.as_deref(), Some("Save"));
        assert_eq!(object.parent, ROOT_PATH);
        assert_eq!(snapshot.roots(), &[path]);
    }

    #[test]
    fn a_path_is_never_recycled() {
        // A path is a promise to a screen reader that it can ask about
        // that object again. Handing a later node the same path would
        // answer those questions about the wrong thing.
        let store = Store::new();
        let first = Id::unique();

        store.apply(&[Change::Added(entry(&first, None, &[0], "First"))]);
        let first_path = store.path_of(&first).unwrap();

        store.apply(&[Change::Removed(first)]);

        let second = Id::unique();
        store.apply(&[Change::Added(entry(&second, None, &[0], "Second"))]);
        let second_path = store.path_of(&second).unwrap();

        assert_ne!(
            first_path, second_path,
            "a removed node's path must not be reused"
        );
    }

    #[test]
    fn children_are_linked_and_indexed_in_traversal_order() {
        let store = Store::new();
        let parent = Id::unique();
        let first = Id::unique();
        let second = Id::unique();

        store.apply(&[
            Change::Added(entry(&parent, None, &[0], "Group")),
            Change::Added(entry(&first, Some(&parent), &[0, 0], "First")),
            Change::Added(entry(&second, Some(&parent), &[0, 1], "Second")),
        ]);

        let snapshot = store.snapshot();
        let parent_path = store.path_of(&parent).unwrap();
        let parent_object = snapshot.get(&parent_path).unwrap();

        assert_eq!(parent_object.children.len(), 2);
        assert_eq!(
            parent_object.children[0],
            store.path_of(&first).unwrap(),
            "children must be linked in traversal order"
        );

        let second_object = snapshot.get(&store.path_of(&second).unwrap()).unwrap();
        assert_eq!(second_object.index_in_parent, 1);
        assert_eq!(second_object.parent, parent_path);
    }

    #[test]
    fn a_removed_node_leaves_the_snapshot() {
        let store = Store::new();
        let id = Id::unique();

        store.apply(&[Change::Added(entry(&id, None, &[0], "Save"))]);
        let path = store.path_of(&id).unwrap();

        store.apply(&[Change::Removed(id)]);

        assert!(
            store.snapshot().get(&path).is_none(),
            "a retired node must stop answering"
        );
    }

    #[test]
    fn a_move_updates_geometry_without_disturbing_identity() {
        // The cheap path a scroll takes: many nodes move, nothing is
        // added, removed, or re-announced.
        let store = Store::new();
        let id = Id::unique();

        store.apply(&[Change::Added(entry(&id, None, &[0], "Save"))]);
        let path = store.path_of(&id).unwrap();

        store.apply(&[Change::Moved {
            id: id.clone(),
            bounds: Rectangle::new(
                taim_core::Point::new(5.0, 40.0),
                taim_core::Size::new(10.0, 10.0),
            ),
        }]);

        assert_eq!(store.path_of(&id).as_deref(), Some(path.as_str()));

        let snapshot = store.snapshot();
        let object = snapshot.get(&path).unwrap();

        assert_eq!(object.bounds.y, 40.0);
        assert_eq!(object.name.as_deref(), Some("Save"));
    }

    #[test]
    fn a_path_survives_an_update() {
        let store = Store::new();
        let id = Id::unique();

        store.apply(&[Change::Added(entry(&id, None, &[0], "Save"))]);
        let path = store.path_of(&id).unwrap();

        let mut changed = entry(&id, None, &[0], "Saved");
        changed.state = State::CHECKED;

        store.apply(&[Change::Updated {
            id: id.clone(),
            entry: changed,
        }]);

        assert_eq!(
            store.path_of(&id).as_deref(),
            Some(path.as_str()),
            "a node that changed is still the same node"
        );
        assert_eq!(
            store.snapshot().get(&path).unwrap().name.as_deref(),
            Some("Saved")
        );
    }

    #[test]
    fn an_incoming_path_routes_back_to_its_id() {
        // The direction an action travels: a screen reader invokes
        // `DoAction` on a path, and it has to become a widget again.
        let store = Store::new();
        let id = Id::unique();

        store.apply(&[Change::Added(entry(&id, None, &[0], "Save"))]);
        let path = store.path_of(&id).unwrap();

        assert_eq!(store.id_at(&path), Some(id));
        assert_eq!(store.id_at("/org/a11y/atspi/accessible/nonexistent"), None);
    }

    #[test]
    fn a_snapshot_is_a_consistent_view_that_outlives_the_next_frame() {
        // The whole reason the store exists: the bus task answers a
        // series of questions without the UI thread's next frame tearing
        // the answers apart mid-sequence.
        let store = Store::new();
        let id = Id::unique();

        store.apply(&[Change::Added(entry(&id, None, &[0], "Save"))]);
        let held = store.snapshot();
        let path = store.path_of(&id).unwrap();

        store.apply(&[Change::Removed(id)]);

        assert!(
            held.get(&path).is_some(),
            "a snapshot already handed out must not change underneath its reader"
        );
        assert!(store.snapshot().get(&path).is_none());
    }
}

//! The retained view of a single semantic node.
//!
//! [`taim_core::semantics::Node`] is a *value*: produced fresh each frame,
//! owning its children, with no idea where it sits in the whole. That is
//! the right shape for a widget to hand upward, and the wrong shape to
//! retain: diffing, focus traversal, and action routing all need to ask
//! "who is this node's parent?" and "what comes after it?" without
//! walking the forest again.
//!
//! [`Entry`] is that retained view: the node's own properties, flattened,
//! plus the parent link and [`Order`] that let the rest of this crate
//! answer those questions in constant time.

use taim_core::Rectangle;
use taim_core::semantics::{Actions, Live, Node, Role, State, Value};
use taim_core::widget::Id;

use smol_str::SmolStr;

/// A node's position in the tree's deterministic traversal order.
///
/// Traversal order is the path of child indices from a root down to the
/// node -- the same shape `taim_runtime::hit_test::Path` uses for spatial
/// lookup, and for the same reason: it sorts lexicographically into
/// exactly depth-first document order, which is the order a screen reader
/// reads and Tab moves through.
///
/// Ordering by [`Order`] is therefore ordering by "what comes next",
/// with no extra bookkeeping.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Order(Vec<usize>);

impl Order {
    /// The order of a root node at `index` among the roots.
    pub fn root(index: usize) -> Self {
        Self(vec![index])
    }

    /// The order of the `index`-th child of a node at this order.
    pub fn child(&self, index: usize) -> Self {
        let mut path = self.0.clone();
        path.push(index);
        Self(path)
    }

    /// The child indices making up this order, outermost first.
    pub fn indices(&self) -> &[usize] {
        &self.0
    }

    /// The depth of this node: `1` for a root.
    pub fn depth(&self) -> usize {
        self.0.len()
    }

    /// Whether `self` is an ancestor of `other`.
    ///
    /// A node is not its own ancestor.
    pub fn is_ancestor_of(&self, other: &Self) -> bool {
        other.0.len() > self.0.len() && other.0.starts_with(&self.0)
    }
}

/// One node, retained: its own properties plus its place in the tree.
///
/// Children are *not* stored here — the [`Tree`](crate::tree::Tree) owns
/// the relationships, so a node's properties can be compared without
/// dragging its whole subtree along. That separation is what makes
/// [`Entry::differs_from`] a cheap, local check.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Stable identity, carried straight from the widget.
    pub id: Id,
    /// The parent's identity, or `None` for a root.
    pub parent: Option<Id>,
    /// Deterministic traversal position.
    pub order: Order,
    /// Screen-space bounds.
    pub bounds: Rectangle,
    /// What kind of element this is.
    pub role: Role,
    /// Current state flags.
    pub state: State,
    /// Short human-readable name.
    pub name: Option<SmolStr>,
    /// Longer supplementary description.
    pub description: Option<SmolStr>,
    /// Current value, if the node discloses one.
    pub value: Option<Value>,
    /// Currently supported actions.
    pub actions: Actions,
    /// Live-region behavior, if this node announces proactively.
    pub live: Option<Live>,
}

impl Entry {
    /// Builds a retained entry from a freshly-produced semantic node.
    ///
    /// `node.value()` is read through its accessor rather than a field,
    /// so a [`Role::SecureField`] contributes no value here either --
    /// the redaction holds all the way into the retained tree, not just
    /// at construction.
    pub fn new(node: &Node, parent: Option<Id>, order: Order) -> Self {
        Self {
            id: node.id.clone(),
            parent,
            order,
            bounds: node.bounds,
            role: node.role,
            state: node.state,
            name: node.name.clone(),
            description: node.description.clone(),
            value: node.value().cloned(),
            actions: node.actions,
            live: node.live,
        }
    }

    /// Whether this node can take keyboard focus right now.
    ///
    /// Requires both the capability (`FOCUSABLE`/`Actions::FOCUS`) and
    /// the absence of anything that would make focusing it wrong: a
    /// disabled or hidden node must never be landed on, or Tab would
    /// appear to stop working.
    pub fn is_focusable(&self) -> bool {
        let claims_focus =
            self.state.contains(State::FOCUSABLE) || self.actions.contains(Actions::FOCUS);

        claims_focus && !self.state.contains(State::DISABLED) && !self.state.contains(State::HIDDEN)
    }

    /// Whether this node's own properties differ from `other`'s,
    /// ignoring identity and tree position.
    ///
    /// This is what a diff compares: position changes are a separate kind
    /// of change ([`Change::Reordered`](crate::tree::Change::Reordered)),
    /// because a node that merely moved does not need re-announcing the
    /// way one whose label changed does.
    pub fn differs_from(&self, other: &Self) -> bool {
        self.bounds != other.bounds
            || self.role != other.role
            || self.state != other.state
            || self.name != other.name
            || self.description != other.description
            || self.value != other.value
            || self.actions != other.actions
            || self.live != other.live
    }

    /// Whether only this node's bounds differ from `other`'s.
    ///
    /// Worth distinguishing: a scroll or a resize moves a great many
    /// nodes without changing anything a screen reader would re-announce,
    /// so an adapter can forward these as cheap geometry updates instead
    /// of full node updates. This is `phase.md`'s "dirty visual bounds
    /// update semantics without rebuilding clean subtrees", observed from
    /// the diff's side.
    pub fn only_bounds_differ_from(&self, other: &Self) -> bool {
        self.bounds != other.bounds
            && self.role == other.role
            && self.state == other.state
            && self.name == other.name
            && self.description == other.description
            && self.value == other.value
            && self.actions == other.actions
            && self.live == other.live
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_sorts_into_depth_first_document_order() {
        let mut orders = vec![
            Order(vec![1]),
            Order(vec![0, 1]),
            Order(vec![0]),
            Order(vec![0, 0]),
            Order(vec![0, 0, 0]),
        ];
        orders.sort();

        assert_eq!(
            orders,
            vec![
                Order(vec![0]),
                Order(vec![0, 0]),
                Order(vec![0, 0, 0]),
                Order(vec![0, 1]),
                Order(vec![1]),
            ],
            "a parent must sort before its children, and earlier siblings before later ones"
        );
    }

    #[test]
    fn ancestry_is_prefix_containment_and_excludes_self() {
        let parent = Order(vec![0, 1]);
        let child = Order(vec![0, 1, 2]);
        let cousin = Order(vec![0, 2]);

        assert!(parent.is_ancestor_of(&child));
        assert!(!child.is_ancestor_of(&parent));
        assert!(
            !parent.is_ancestor_of(&parent),
            "a node is not its own ancestor"
        );
        assert!(!parent.is_ancestor_of(&cousin));
    }

    #[test]
    fn child_extends_the_parents_path() {
        let root = Order::root(3);
        assert_eq!(root.indices(), &[3]);
        assert_eq!(root.depth(), 1);

        let child = root.child(1);
        assert_eq!(child.indices(), &[3, 1]);
        assert_eq!(child.depth(), 2);
        assert!(root.is_ancestor_of(&child));
    }
}

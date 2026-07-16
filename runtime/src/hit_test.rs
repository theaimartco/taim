//! Cached path lookup interface, with a linear fallback retained as the
//! correctness oracle.
//!
//! [`hit_test`] finds which leaf of a widget tree contains a given
//! [`Point`], returning the [`Path`] of child indices from the root down
//! to it. It is deliberately a **linear** (depth-first, `O(nodes)`)
//! walk: `phase.md`'s Phase 8 (`aura-gesture`) is where a real spatial
//! index is expected to replace this for large trees, and its own exit
//! gate requires that index to "always agree with exhaustive hit
//! testing" -- i.e. agree with exactly the function in this file. Phase
//! 3 does not build that index; it builds the oracle the index will be
//! checked against, plus the [`Path`] vocabulary a future cache can be
//! keyed on.
//!
//! # Why no cache yet
//!
//! A hit-test cache is only sound if it is invalidated whenever the
//! layout it was computed against changes. Nothing in this codebase
//! marks a subtree "layout changed" reliably yet outside of the opt-in
//! [`iced_core::widget::tree::Tree::layout_with_cache`] mechanism
//! (Phase 3, commit 2), which no production widget has adopted. Caching
//! hit-test results against an unreliable invalidation signal would be
//! actively worse than the linear walk it replaces (a wrong cache hit
//! routes an event to the wrong widget), so this module ships only the
//! oracle for now.

use crate::core::{Layout, Point};

/// A path from the root of a widget tree down to a specific descendant,
/// expressed as a sequence of child indices.
///
/// An empty [`Path`] refers to the root itself.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Path(Vec<usize>);

impl Path {
    /// Returns the child indices making up this [`Path`], from the
    /// outermost ancestor to the innermost descendant.
    pub fn indices(&self) -> &[usize] {
        &self.0
    }

    /// Returns whether this [`Path`] refers to the root itself (i.e. it
    /// has no indices).
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }
}

/// Something that can be hit-tested at a given [`Layout`]: whether a
/// [`Point`] should be considered "inside" it, and its own children's
/// layouts, for recursing into them.
///
/// This trait exists so [`hit_test`] can walk a tree of `(widget, Tree,
/// Layout)` triples without depending on `iced_core::Widget`'s full,
/// renderer-generic signature -- callers adapt their own widget/tree/
/// layout triples to it (see `tests/events/hit_path.rs` for a full
/// example against real widgets).
pub trait Hoverable {
    /// Returns whether `point` is considered part of this node
    /// (typically via [`iced_core::Widget::hit_test`], or a plain
    /// bounds check for a simpler caller).
    fn contains(&self, point: Point) -> bool;

    /// Returns the number of children this node has.
    fn child_count(&self) -> usize;

    /// Returns the `index`-th child, and its [`Layout`], for recursing
    /// into. Must return the same value every time for the same
    /// `index` during a single [`hit_test`] call.
    fn child(&self, index: usize) -> (Self, Layout<'_>)
    where
        Self: Sized;
}

/// Finds the deepest descendant (by depth-first traversal, later
/// children take priority over earlier ones at the same level, matching
/// the usual "topmost visually wins" convention for overlapping
/// siblings) whose bounds contain `point`, returning the [`Path`] to it.
///
/// Returns `None` if `point` is not contained by `root` at all.
///
/// This is the correctness oracle: it visits every node whose ancestor
/// chain contains `point`, in `O(nodes on the path plus their
/// siblings)` time, with no caching or early-exit heuristics beyond
/// "stop descending into a subtree whose own bounds do not contain the
/// point."
pub fn hit_test<T: Hoverable>(root: &T, point: Point) -> Option<Path> {
    if !root.contains(point) {
        return None;
    }

    let mut path = Vec::new();
    descend(root, point, &mut path);

    Some(Path(path))
}

/// Recurses into `node`'s children, appending the index of each one
/// entered to `path`, stopping at the deepest descendant that still
/// contains `point`.
fn descend<T: Hoverable>(node: &T, point: Point, path: &mut Vec<usize>) {
    // Later children are checked first so that, among overlapping
    // siblings, the one drawn last (typically on top) wins -- the same
    // convention `Widget::overlay`/event dispatch already follows
    // elsewhere in this codebase.
    for index in (0..node.child_count()).rev() {
        let (child, _child_layout) = node.child(index);

        if child.contains(point) {
            path.push(index);
            descend(&child, point, path);
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Rectangle, Size};

    /// A minimal synthetic tree for exercising [`hit_test`] without any
    /// dependency on `iced_core::Widget`.
    #[derive(Debug, Clone)]
    struct Node {
        bounds: Rectangle,
        children: Vec<Node>,
    }

    impl Node {
        fn leaf(bounds: Rectangle) -> Self {
            Self {
                bounds,
                children: Vec::new(),
            }
        }

        fn branch(bounds: Rectangle, children: Vec<Node>) -> Self {
            Self { bounds, children }
        }
    }

    impl Hoverable for Node {
        fn contains(&self, point: Point) -> bool {
            self.bounds.contains(point)
        }

        fn child_count(&self) -> usize {
            self.children.len()
        }

        fn child(&self, index: usize) -> (Self, Layout<'_>) {
            (self.children[index].clone(), unreachable_layout())
        }
    }

    /// `Layout` is only produced here to satisfy `Hoverable::child`'s
    /// signature; these tests never read it (they use `Node::bounds`
    /// directly), so a static, empty backing `layout::Node` is enough.
    fn unreachable_layout() -> Layout<'static> {
        use std::sync::OnceLock;
        static NODE: OnceLock<crate::core::layout::Node> = OnceLock::new();

        Layout::new(NODE.get_or_init(|| crate::core::layout::Node::new(Size::ZERO)))
    }

    fn rect(x: f32, y: f32, size: f32) -> Rectangle {
        Rectangle::new(crate::core::Point::new(x, y), Size::new(size, size))
    }

    #[test]
    fn a_point_outside_the_root_hits_nothing() {
        let root = Node::leaf(rect(0.0, 0.0, 10.0));

        assert_eq!(hit_test(&root, Point::new(100.0, 100.0)), None);
    }

    #[test]
    fn a_point_inside_a_childless_root_hits_the_root_itself() {
        let root = Node::leaf(rect(0.0, 0.0, 10.0));

        assert_eq!(hit_test(&root, Point::new(5.0, 5.0)), Some(Path::default()));
    }

    #[test]
    fn a_point_inside_a_child_returns_the_path_to_it() {
        let root = Node::branch(
            rect(0.0, 0.0, 100.0),
            vec![
                Node::leaf(rect(0.0, 0.0, 10.0)),
                Node::leaf(rect(50.0, 50.0, 10.0)),
            ],
        );

        let path = hit_test(&root, Point::new(55.0, 55.0)).expect("point is inside child 1");

        assert_eq!(path.indices(), &[1]);
    }

    #[test]
    fn a_point_inside_nested_children_returns_the_full_path() {
        let root = Node::branch(
            rect(0.0, 0.0, 100.0),
            vec![Node::branch(
                rect(0.0, 0.0, 50.0),
                vec![Node::leaf(rect(10.0, 10.0, 10.0))],
            )],
        );

        let path =
            hit_test(&root, Point::new(15.0, 15.0)).expect("point is inside the nested leaf");

        assert_eq!(path.indices(), &[0, 0]);
    }

    #[test]
    fn overlapping_siblings_resolve_to_the_later_one() {
        let root = Node::branch(
            rect(0.0, 0.0, 100.0),
            vec![
                Node::leaf(rect(0.0, 0.0, 50.0)),
                Node::leaf(rect(0.0, 0.0, 50.0)),
            ],
        );

        let path = hit_test(&root, Point::new(10.0, 10.0)).expect("point is inside both");

        assert_eq!(path.indices(), &[1], "the later (topmost) sibling wins");
    }

    #[test]
    fn a_point_inside_the_root_but_outside_every_child_hits_the_root() {
        let root = Node::branch(
            rect(0.0, 0.0, 100.0),
            vec![Node::leaf(rect(0.0, 0.0, 10.0))],
        );

        let path = hit_test(&root, Point::new(90.0, 90.0)).expect("point is inside the root");

        assert!(path.is_root());
    }

    #[test]
    fn path_is_root_is_true_only_for_an_empty_path() {
        assert!(Path::default().is_root());
        assert!(!Path(vec![0]).is_root());
    }
}

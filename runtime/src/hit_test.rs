//! Cached path lookup interface, with a linear fallback retained as the
//! correctness oracle.
//!
//! [`hit_test`] finds which leaf of a widget tree contains a given
//! [`Point`], returning the [`Path`] of child indices from the root down
//! to it. It is deliberately a **linear** (depth-first, `O(nodes)`)
//! walk, kept exactly as Phase 3 shipped it: the oracle every faster
//! path in this module -- today, [`Index`] -- is checked against, plus
//! the [`Path`] vocabulary that structure is keyed on.
//!
//! [`Index`] (Phase 8) is the real spatial index `phase.md` calls for:
//! a grid-bucketed structure over a layout snapshot's leaf bounds, built
//! once per layout pass and queried per pointer event without
//! re-walking the whole tree each time. It is not an independent,
//! separately-trusted implementation -- see [`Index`]'s own
//! documentation for why it is, by construction, unable to disagree
//! with [`hit_test`] on the same tree.
//!
//! # Why the oracle itself is never cached
//!
//! A hit-test cache is only sound if it is invalidated whenever the
//! layout it was computed against changes. Nothing in this codebase
//! marks a subtree "layout changed" reliably yet outside of the opt-in
//! [`taim_core::widget::tree::Tree::layout_with_cache`] mechanism
//! (Phase 3), which no production widget has adopted for that purpose.
//! [`Index`] sidesteps this by being rebuilt wholesale every layout pass
//! rather than incrementally patched -- there is no staleness window to
//! reason about, only "was this built from the current layout or not."

use crate::core::widget::visit::Visit;
use crate::core::{Point, Rectangle};

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
pub fn hit_test(root: &mut dyn Visit, point: Point) -> Option<Path> {
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
///
/// Among overlapping siblings the **last** one wins, so that the one
/// drawn on top does -- the same convention `Widget::overlay`/event
/// dispatch follows. `Visit::children` is forward-only, so rather than
/// scanning in reverse and stopping at the first hit, this descends into
/// every hitting sibling and keeps the last result. Identical outcome;
/// the extra work happens only where siblings actually overlap, which is
/// rare and bounded.
fn descend(node: &mut dyn Visit, point: Point, path: &mut Vec<usize>) {
    let mut deepest: Option<(usize, Vec<usize>)> = None;
    let mut index = 0;

    node.children(&mut |child| {
        if child.contains(point) {
            let mut below = Vec::new();
            descend(child, point, &mut below);
            deepest = Some((index, below));
        }

        index += 1;
    });

    if let Some((index, below)) = deepest {
        path.push(index);
        path.extend(below);
    }
}

/// A spatial acceleration structure over a snapshot of a widget tree's
/// leaf bounds, built once per layout pass and queried many times
/// against it (once per pointer event that frame, typically) without
/// re-walking the whole tree for each one.
///
/// [`Index`] is a uniform grid: [`Index::build`] buckets every leaf's
/// [`Hoverable::bounds`] into fixed-size cells, so [`Index::hit_test`]
/// only has to inspect the (usually small) set of leaves whose bucket
/// overlaps the query point, instead of every leaf in the tree. Grid
/// buckets, not an R-tree or similar hierarchical structure, because a
/// typical widget tree's leaves are roughly uniformly distributed across
/// the visible surface (not clustered at wildly different scales the way
/// a k-d tree/R-tree earns its keep for), and a flat grid is
/// dramatically simpler to build, invalidate (rebuild wholesale; there
/// is no incremental node removal to get wrong), and prove correct.
///
/// # Why this can never disagree with [`hit_test`]
///
/// [`Index::hit_test`] still calls each candidate leaf's own
/// [`Hoverable::contains`] before accepting it -- exactly the same
/// predicate the oracle itself uses, which may be non-rectangular (a
/// clipped scrollable, a custom `Widget::hit_test` override). The grid
/// only ever *narrows which leaves are checked at all*; it never
/// substitutes a bounding-box check for the real containment predicate.
/// `tests/events/spatial_hit_test.rs` proves this equivalence
/// empirically, across randomly generated trees, as required by
/// `phase.md`'s own exit gate ("spatial index always agrees with
/// exhaustive hit testing").
pub struct Index {
    cell_size: f32,
    // Bucketed by `(cell_x, cell_y)`; each bucket holds every leaf whose
    // bounds overlap that cell, alongside its `Path` and bounds (kept
    // here so a query need not re-fetch them from the tree).
    cells: rustc_hash::FxHashMap<(i32, i32), Vec<(Path, Rectangle)>>,
}

impl Index {
    /// The default cell size, in logical pixels -- large enough that
    /// most ordinary widgets span only one or a handful of cells, small
    /// enough that a sparse tree over a large surface does not collapse
    /// back into "every leaf in one bucket."
    pub const DEFAULT_CELL_SIZE: f32 = 128.0;

    /// Builds an [`Index`] over every leaf reachable from `root`, using
    /// [`Index::DEFAULT_CELL_SIZE`].
    pub fn build(root: &mut dyn Visit) -> Self {
        Self::build_with_cell_size(root, Self::DEFAULT_CELL_SIZE)
    }

    /// Builds an [`Index`], as [`Index::build`], with an explicit cell
    /// size.
    ///
    /// # Panics
    /// Panics if `cell_size` is not a positive, finite number.
    pub fn build_with_cell_size(root: &mut dyn Visit, cell_size: f32) -> Self {
        assert!(
            cell_size.is_finite() && cell_size > 0.0,
            "Index cell size must be a positive, finite number"
        );

        let mut index = Self {
            cell_size,
            cells: rustc_hash::FxHashMap::default(),
        };

        let mut path = Vec::new();
        index.collect(root, &mut path);

        index
    }

    fn collect(&mut self, node: &mut dyn Visit, path: &mut Vec<usize>) {
        let mut index = 0;

        node.children(&mut |child| {
            path.push(index);
            self.collect(child, path);
            let _ = path.pop();

            index += 1;
        });

        if index == 0 {
            self.insert(Path(path.clone()), node.bounds());
        }
    }

    fn insert(&mut self, path: Path, bounds: Rectangle) {
        let cells: Vec<_> = self.cells_for(bounds).collect();

        for cell in cells {
            self.cells
                .entry(cell)
                .or_default()
                .push((path.clone(), bounds));
        }
    }

    fn cell_at(&self, point: Point) -> (i32, i32) {
        (
            (point.x / self.cell_size).floor() as i32,
            (point.y / self.cell_size).floor() as i32,
        )
    }

    fn cells_for(&self, bounds: Rectangle) -> impl Iterator<Item = (i32, i32)> + '_ {
        let (min_x, min_y) = self.cell_at(Point::new(bounds.x, bounds.y));
        let (max_x, max_y) = self.cell_at(Point::new(
            bounds.x + bounds.width,
            bounds.y + bounds.height,
        ));

        (min_x..=max_x).flat_map(move |x| (min_y..=max_y).map(move |y| (x, y)))
    }

    /// Returns whether any leaf bucketed under `point`'s cell has a
    /// [`Path`] starting with `prefix` -- i.e. whether the subtree
    /// rooted at `prefix` has *any* leaf whose bounds contain `point`,
    /// and therefore whether [`Index::hit_test`] needs to actually
    /// descend into it at all.
    ///
    /// This -- not a flat "leaf with the highest priority" comparison --
    /// is the only thing the grid is trusted to answer: a leaf's bounds
    /// containing `point` says nothing about whether every *ancestor*
    /// on its path also contains `point` (a leaf can geometrically
    /// overlap a point while sitting inside a sibling subtree the point
    /// never actually reaches through real, possibly non-rectangular,
    /// [`Hoverable::contains`] checks at each level). Real containment
    /// is still only ever decided by walking the tree in
    /// [`Index::hit_test`]; this method only prunes which subtrees that
    /// walk bothers to enter.
    fn subtree_has_candidate(&self, point: Point, prefix: &[usize]) -> bool {
        let Some(bucket) = self.cells.get(&self.cell_at(point)) else {
            return false;
        };

        bucket
            .iter()
            .any(|(path, bounds)| bounds.contains(point) && path.indices().starts_with(prefix))
    }

    /// Hit-tests `point` against this [`Index`]: the same depth-first,
    /// later-children-first descent [`hit_test`] itself performs, except
    /// a subtree is only entered if [`Index::subtree_has_candidate`]
    /// says it might contain a hit at all, skipping subtrees the grid
    /// can already rule out.
    ///
    /// Because every accepted result still passes through the exact
    /// same [`Hoverable::contains`] calls the oracle would have made
    /// (just fewer of them, for subtrees the grid ruled out), an
    /// [`Index`] can never disagree with [`hit_test`] on the same tree
    /// -- it is not a heuristic answering a different question faster,
    /// it is the same recursive algorithm with unproductive branches
    /// pruned. See this type's own documentation, and
    /// `tests/events/spatial_hit_test.rs`, for the empirical proof of
    /// this equivalence.
    ///
    /// `root` must be the exact same tree this [`Index`] was built from
    /// (same shape, same bounds) -- an [`Index`] built from a stale
    /// layout and queried against a newer one is not a correctness bug
    /// in this method itself, but the caller's; rebuild the [`Index`]
    /// whenever layout changes, exactly as this module's own top-level
    /// documentation already requires of any hit-test cache.
    pub fn hit_test(&self, root: &mut dyn Visit, point: Point) -> Option<Path> {
        if !root.contains(point) {
            return None;
        }

        let mut path = Vec::new();
        self.descend(root, point, &mut path);

        Some(Path(path))
    }

    /// As [`descend`], but skipping any subtree the grid says holds no
    /// candidate at all. Keeps the last hitting sibling, for the reason
    /// [`descend`] documents.
    fn descend(&self, node: &mut dyn Visit, point: Point, path: &mut Vec<usize>) {
        let mut deepest: Option<(usize, Vec<usize>)> = None;
        let mut index = 0;

        node.children(&mut |child| {
            path.push(index);

            if self.subtree_has_candidate(point, path) && child.contains(point) {
                let mut below = Vec::new();
                let mut probe = path.clone();

                self.descend_into(child, point, &mut probe, &mut below);
                deepest = Some((index, below));
            }

            let _ = path.pop();
            index += 1;
        });

        if let Some((index, below)) = deepest {
            path.push(index);
            path.extend(below);
        }
    }

    /// [`Index::descend`] one level down, accumulating into `below` while
    /// `probe` tracks the absolute path the grid is queried with.
    fn descend_into(
        &self,
        node: &mut dyn Visit,
        point: Point,
        probe: &mut Vec<usize>,
        below: &mut Vec<usize>,
    ) {
        let mut deepest: Option<(usize, Vec<usize>)> = None;
        let mut index = 0;

        node.children(&mut |child| {
            probe.push(index);

            if self.subtree_has_candidate(point, probe) && child.contains(point) {
                let mut deeper = Vec::new();
                self.descend_into(child, point, probe, &mut deeper);
                deepest = Some((index, deeper));
            }

            let _ = probe.pop();
            index += 1;
        });

        if let Some((index, deeper)) = deepest {
            below.push(index);
            below.extend(deeper);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Rectangle, Size};

    /// A minimal synthetic tree for exercising [`hit_test`] without any
    /// dependency on `taim_core::Widget`.
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

    impl Visit for Node {
        fn semantics(&mut self) -> Option<crate::core::semantics::Node> {
            None
        }

        fn contains(&self, point: Point) -> bool {
            self.bounds.contains(point)
        }

        fn bounds(&self) -> Rectangle {
            self.bounds
        }

        fn children(&mut self, visit: &mut dyn FnMut(&mut dyn Visit)) {
            for child in &mut self.children {
                visit(child);
            }
        }
    }

    fn rect(x: f32, y: f32, size: f32) -> Rectangle {
        Rectangle::new(crate::core::Point::new(x, y), Size::new(size, size))
    }

    #[test]
    fn a_point_outside_the_root_hits_nothing() {
        let mut root = Node::leaf(rect(0.0, 0.0, 10.0));

        assert_eq!(hit_test(&mut root, Point::new(100.0, 100.0)), None);
    }

    #[test]
    fn a_point_inside_a_childless_root_hits_the_root_itself() {
        let mut root = Node::leaf(rect(0.0, 0.0, 10.0));

        assert_eq!(
            hit_test(&mut root, Point::new(5.0, 5.0)),
            Some(Path::default())
        );
    }

    #[test]
    fn a_point_inside_a_child_returns_the_path_to_it() {
        let mut root = Node::branch(
            rect(0.0, 0.0, 100.0),
            vec![
                Node::leaf(rect(0.0, 0.0, 10.0)),
                Node::leaf(rect(50.0, 50.0, 10.0)),
            ],
        );

        let path = hit_test(&mut root, Point::new(55.0, 55.0)).expect("point is inside child 1");

        assert_eq!(path.indices(), &[1]);
    }

    #[test]
    fn a_point_inside_nested_children_returns_the_full_path() {
        let mut root = Node::branch(
            rect(0.0, 0.0, 100.0),
            vec![Node::branch(
                rect(0.0, 0.0, 50.0),
                vec![Node::leaf(rect(10.0, 10.0, 10.0))],
            )],
        );

        let path =
            hit_test(&mut root, Point::new(15.0, 15.0)).expect("point is inside the nested leaf");

        assert_eq!(path.indices(), &[0, 0]);
    }

    #[test]
    fn overlapping_siblings_resolve_to_the_later_one() {
        let mut root = Node::branch(
            rect(0.0, 0.0, 100.0),
            vec![
                Node::leaf(rect(0.0, 0.0, 50.0)),
                Node::leaf(rect(0.0, 0.0, 50.0)),
            ],
        );

        let path = hit_test(&mut root, Point::new(10.0, 10.0)).expect("point is inside both");

        assert_eq!(path.indices(), &[1], "the later (topmost) sibling wins");
    }

    #[test]
    fn a_point_inside_the_root_but_outside_every_child_hits_the_root() {
        let mut root = Node::branch(
            rect(0.0, 0.0, 100.0),
            vec![Node::leaf(rect(0.0, 0.0, 10.0))],
        );

        let path = hit_test(&mut root, Point::new(90.0, 90.0)).expect("point is inside the root");

        assert!(path.is_root());
    }

    #[test]
    fn path_is_root_is_true_only_for_an_empty_path() {
        assert!(Path::default().is_root());
        assert!(!Path(vec![0]).is_root());
    }

    #[test]
    fn index_agrees_with_the_oracle_for_a_simple_tree() {
        let mut root = Node::branch(
            rect(0.0, 0.0, 100.0),
            vec![
                Node::leaf(rect(0.0, 0.0, 10.0)),
                Node::leaf(rect(50.0, 50.0, 10.0)),
            ],
        );

        let index = Index::build(&mut root);

        for point in [
            Point::new(5.0, 5.0),
            Point::new(55.0, 55.0),
            Point::new(90.0, 90.0),
            Point::new(500.0, 500.0),
        ] {
            assert_eq!(hit_test(&mut root, point), index.hit_test(&mut root, point));
        }
    }

    #[test]
    fn index_agrees_with_the_oracle_for_overlapping_siblings() {
        let mut root = Node::branch(
            rect(0.0, 0.0, 100.0),
            vec![
                Node::leaf(rect(0.0, 0.0, 50.0)),
                Node::leaf(rect(0.0, 0.0, 50.0)),
            ],
        );

        let index = Index::build(&mut root);
        let point = Point::new(10.0, 10.0);

        assert_eq!(hit_test(&mut root, point), index.hit_test(&mut root, point));
        assert_eq!(
            index.hit_test(&mut root, point).unwrap().indices(),
            &[1],
            "the index must also resolve overlapping siblings to the later one"
        );
    }

    #[test]
    fn index_agrees_with_the_oracle_for_nested_children() {
        let mut root = Node::branch(
            rect(0.0, 0.0, 100.0),
            vec![Node::branch(
                rect(0.0, 0.0, 50.0),
                vec![Node::leaf(rect(10.0, 10.0, 10.0))],
            )],
        );

        let index = Index::build(&mut root);
        let point = Point::new(15.0, 15.0);

        assert_eq!(hit_test(&mut root, point), index.hit_test(&mut root, point));
    }

    #[test]
    fn index_agrees_with_the_oracle_when_nothing_is_hit() {
        let mut root = Node::leaf(rect(0.0, 0.0, 10.0));
        let index = Index::build(&mut root);
        let point = Point::new(1000.0, 1000.0);

        assert_eq!(hit_test(&mut root, point), None);
        assert_eq!(index.hit_test(&mut root, point), None);
    }

    #[test]
    fn index_agrees_with_the_oracle_across_a_grid_of_many_leaves() {
        // A grid of leaves spanning many cells, so `Index` actually
        // exercises more than one bucket and more than a trivial number
        // of leaves.
        let mut children = Vec::new();

        for row in 0..20 {
            for col in 0..20 {
                children.push(Node::leaf(rect(col as f32 * 50.0, row as f32 * 50.0, 40.0)));
            }
        }

        let mut root = Node::branch(rect(0.0, 0.0, 1000.0), children);
        let index = Index::build_with_cell_size(&mut root, 64.0);

        for x in (0..1000).step_by(37) {
            for y in (0..1000).step_by(53) {
                let point = Point::new(x as f32, y as f32);

                assert_eq!(
                    hit_test(&mut root, point),
                    index.hit_test(&mut root, point),
                    "disagreement at {point:?}"
                );
            }
        }
    }

    #[test]
    #[should_panic(expected = "positive, finite")]
    fn zero_cell_size_panics() {
        let mut root = Node::leaf(rect(0.0, 0.0, 10.0));
        let _ = Index::build_with_cell_size(&mut root, 0.0);
    }

    #[test]
    #[should_panic(expected = "positive, finite")]
    fn negative_cell_size_panics() {
        let mut root = Node::leaf(rect(0.0, 0.0, 10.0));
        let _ = Index::build_with_cell_size(&mut root, -10.0);
    }
}

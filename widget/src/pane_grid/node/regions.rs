//! TAIM iterative pane-space planning.
//!
//! No recursive descent is used to derive either split or pane regions:
//! deeply nested editor/workspace layouts have bounded call-stack usage.
//! One common geometry engine feeds both pane hit targets and divider
//! handles, preventing disagreements after min-size and ratio clamping.

use super::Node;
use crate::core::{Rectangle, Size};
use crate::pane_grid::{Axis, Pane, Split};
use rustc_hash::FxHashMap;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy)]
struct Extents {
    horizontal: usize,
    vertical: usize,
}

impl Extents {
    const LEAF: Self = Self {
        horizontal: 0,
        vertical: 0,
    };

    fn combine(axis: Axis, a: Self, b: Self) -> Self {
        match axis {
            Axis::Horizontal => Self {
                horizontal: a.horizontal.saturating_add(b.horizontal).saturating_add(1),
                vertical: a.vertical.max(b.vertical),
            },
            Axis::Vertical => Self {
                horizontal: a.horizontal.max(b.horizontal),
                vertical: a.vertical.saturating_add(b.vertical).saturating_add(1),
            },
        }
    }
}

impl Node {
    /// Compute the minimum-size split factors in one iterative postorder pass.
    ///
    /// Raw pointers are used only as hashable identities while the immutable
    /// tree is borrowed; they are never dereferenced or stored after return.
    fn subtree_extents(&self) -> FxHashMap<*const Node, Extents> {
        let mut finished = FxHashMap::default();
        let mut visit = vec![(self, false)];

        while let Some((node, children_visited)) = visit.pop() {
            let key = std::ptr::from_ref(node);

            match node {
                Self::Pane(_) => {
                    finished.insert(key, Extents::LEAF);
                }
                Self::Split { axis, a, b, .. } => {
                    if children_visited {
                        let left = finished[&std::ptr::from_ref(a.as_ref())];
                        let right = finished[&std::ptr::from_ref(b.as_ref())];
                        finished.insert(key, Extents::combine(*axis, left, right));
                    } else {
                        visit.push((node, true));
                        visit.push((b, false));
                        visit.push((a, false));
                    }
                }
            }
        }
        finished
    }

    /// Visit the same geometry plan for both pane painting and split hit-testing.
    ///
    /// Keeping output callbacks separate means pane-only and split-only
    /// callers do not allocate the other result map.
    fn traverse_regions(
        &self,
        spacing: f32,
        min_size: f32,
        bounds: Size,
        mut on_pane: impl FnMut(Pane, Rectangle),
        mut on_split: impl FnMut(Split, Axis, Rectangle, f32),
    ) {
        let spacing = finite_nonnegative(spacing);
        let min_size = finite_nonnegative(min_size);
        let bounds = finite_size(bounds);
        let extents = self.subtree_extents();

        let root = Rectangle::new([0.0, 0.0].into(), bounds);
        let mut pending = vec![(self, root)];

        while let Some((node, region)) = pending.pop() {
            match node {
                Self::Pane(pane) => on_pane(*pane, region),
                Self::Split { id, axis, ratio, a, b } => {
                    let left = extents[&std::ptr::from_ref(a.as_ref())];
                    let right = extents[&std::ptr::from_ref(b.as_ref())];
                    let (left_factor, right_factor) = match axis {
                        Axis::Horizontal => (left.horizontal, right.horizontal),
                        Axis::Vertical => (left.vertical, right.vertical),
                    };

                    let (left_region, right_region, effective_ratio) = axis.split(
                        &region,
                        *ratio,
                        spacing,
                        minimum_extent(min_size, spacing, left_factor),
                        minimum_extent(min_size, spacing, right_factor),
                    );

                    on_split(*id, *axis, region, effective_ratio);

                    // Push right first, so the visible order is deterministic
                    // and follows left/top before right/bottom.
                    pending.push((b, right_region));
                    pending.push((a, left_region));
                }
            }
        }
    }

    /// Rectangles occupied by all panes under this split tree.
    ///
    /// Coordinates are local to the pane grid and are guaranteed to have
    /// finite nonnegative dimensions even for invalid user supplied ratios.
    pub fn pane_regions(
        &self,
        spacing: f32,
        min_size: f32,
        bounds: Size,
    ) -> BTreeMap<Pane, Rectangle> {
        let mut regions = BTreeMap::new();
        self.traverse_regions(
            spacing,
            min_size,
            bounds,
            |id, region| {
                regions.insert(id, region);
            },
            |_, _, _, _| {},
        );
        regions
    }

    /// Divider geometry and effective split ratio for hit-testing.
    pub fn split_regions(
        &self,
        spacing: f32,
        min_size: f32,
        bounds: Size,
    ) -> BTreeMap<Split, (Axis, Rectangle, f32)> {
        let mut splits = BTreeMap::new();
        self.traverse_regions(
            spacing,
            min_size,
            bounds,
            |_, _| {},
            |id, axis, region, ratio| {
                splits.insert(id, (axis, region, ratio));
            },
        );
        splits
    }
}

#[inline]
fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[inline]
fn finite_size(size: Size) -> Size {
    Size::new(finite_nonnegative(size.width), finite_nonnegative(size.height))
}

#[inline]
fn minimum_extent(min_size: f32, spacing: f32, separators: usize) -> f32 {
    let number = separators as f64;
    let total = f64::from(min_size) * (number + 1.0) + f64::from(spacing) * number;
    total.min(f64::from(f32::MAX)) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mixed_tree() -> Node {
        Node::Split {
            id: Split(3),
            axis: Axis::Vertical,
            ratio: 0.5,
            a: Box::new(Node::Split {
                id: Split(4),
                axis: Axis::Horizontal,
                ratio: 0.25,
                a: Box::new(Node::Pane(Pane(0))),
                b: Box::new(Node::Pane(Pane(1))),
            }),
            b: Box::new(Node::Pane(Pane(2))),
        }
    }

    #[test]
    fn mixed_axes_keep_pane_and_split_geometry_consistent() {
        let tree = mixed_tree();
        let pane = tree.pane_regions(0.0, 0.0, Size::new(100.0, 100.0));
        let split = tree.split_regions(0.0, 0.0, Size::new(100.0, 100.0));
        let top_left = pane[&Pane(0)];
        let bottom_left = pane[&Pane(1)];
        let right = pane[&Pane(2)];

        assert_eq!((top_left.x, top_left.y, top_left.width, top_left.height), (0.0, 0.0, 50.0, 25.0));
        assert_eq!((bottom_left.x, bottom_left.y, bottom_left.width, bottom_left.height), (0.0, 25.0, 50.0, 75.0));
        assert_eq!((right.x, right.y, right.width, right.height), (50.0, 0.0, 50.0, 100.0));
        assert_eq!(split[&Split(3)].2, 0.5);
        assert_eq!(split[&Split(4)].2, 0.25);
    }

    #[test]
    fn nan_ratio_and_invalid_extents_produce_bounded_regions() {
        let tree = Node::Split {
            id: Split(9),
            axis: Axis::Vertical,
            ratio: f32::NAN,
            a: Box::new(Node::Pane(Pane(0))),
            b: Box::new(Node::Pane(Pane(1))),
        };
        let panes = tree.pane_regions(1000.0, -1.0, Size::new(50.0, f32::INFINITY));
        assert_eq!(panes.len(), 2);
        for region in panes.values() {
            assert!(region.width.is_finite());
            assert!(region.height.is_finite());
            assert!(region.width >= 0.0);
            assert_eq!(region.height, 0.0);
        }
    }

    #[test]
    fn deep_tree_has_no_recursive_geometry_traversal() {
        let mut tree = Node::Pane(Pane(0));
        for i in 1..2048 {
            tree = Node::Split {
                id: Split(i),
                axis: Axis::Vertical,
                ratio: 0.5,
                a: Box::new(tree),
                b: Box::new(Node::Pane(Pane(i))),
            };
        }
        let panes = tree.pane_regions(0.0, 0.0, Size::new(4096.0, 100.0));
        let splits = tree.split_regions(0.0, 0.0, Size::new(4096.0, 100.0));
        assert_eq!(panes.len(), 2048);
        assert_eq!(splits.len(), 2047);
    }
}

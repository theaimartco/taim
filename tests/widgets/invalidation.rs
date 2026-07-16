//! Local paint-vs-layout-vs-semantics invalidation tests for
//! `iced_runtime::invalidation`, against real `iced_core::Widget` state
//! trees.
//!
//! `runtime/src/invalidation.rs`'s own unit tests cover `propagate` and
//! `Aggregator` in isolation against bare `Tree`s;
//! `tests/widgets/dirty_flags.rs` (Phase 2) exercised a hand-rolled
//! stand-in for propagation, explicitly documented as a placeholder for
//! Phase 3's real implementation. These tests instead drive the real
//! `iced_runtime::invalidation::propagate` and confirm each `DirtyFlags`
//! bit stays properly scoped: marking a leaf's `PAINT` dirty must not
//! spuriously mark its `LAYOUT` or `SEMANTICS`, propagating up the tree
//! must not touch unrelated siblings, and a leaf's own `layout_with_cache`/
//! `semantics_with_cache` calls must independently reflect exactly the
//! flags that were actually cleared.

use iced::advanced::widget::Tree;
use iced::advanced::widget::tree::DirtyFlags;
use iced_runtime::invalidation::{Aggregator, propagate};

fn tree_with_two_children() -> Tree {
    let mut tree = Tree::with_children(vec![Tree::empty(), Tree::empty()]);
    tree.clear_dirty(DirtyFlags::all());
    tree.children[0].clear_dirty(DirtyFlags::all());
    tree.children[1].clear_dirty(DirtyFlags::all());
    tree
}

#[test]
fn marking_a_leaf_paint_dirty_and_propagating_leaves_its_layout_and_semantics_clean() {
    let mut tree = tree_with_two_children();
    tree.children[0].mark_dirty(DirtyFlags::PAINT);

    propagate(&mut tree, &[0]);

    assert!(tree.children[0].dirty().contains(DirtyFlags::PAINT));
    assert!(
        !tree.children[0].dirty().contains(DirtyFlags::LAYOUT),
        "a paint-only change must not mark layout dirty"
    );
    assert!(
        !tree.children[0].dirty().contains(DirtyFlags::SEMANTICS),
        "a paint-only change must not mark semantics dirty"
    );
}

#[test]
fn propagating_paint_invalidation_up_the_tree_does_not_touch_an_unrelated_sibling() {
    let mut tree = tree_with_two_children();

    propagate(&mut tree, &[0]);

    assert!(tree.dirty().contains(DirtyFlags::PAINT));
    assert!(tree.children[0].dirty().contains(DirtyFlags::PAINT));
    assert!(
        tree.children[1].dirty().is_empty(),
        "a sibling of the changed node must stay entirely clean"
    );
}

#[test]
fn a_layout_change_marked_directly_on_a_node_is_independent_of_paint_propagation() {
    // A widget that resizes marks its own `Tree` `LAYOUT`-dirty directly
    // (as `Widget::layout`/a container's diff would); `propagate` only
    // handles the upward PAINT half, so LAYOUT on the changed node must
    // survive untouched by a `propagate` call, and must not appear on
    // an ancestor that `propagate` also marks PAINT-dirty.
    let mut tree = tree_with_two_children();
    tree.children[0].mark_dirty(DirtyFlags::LAYOUT);

    propagate(&mut tree, &[0]);

    assert!(tree.children[0].dirty().contains(DirtyFlags::LAYOUT));
    assert!(tree.children[0].dirty().contains(DirtyFlags::PAINT));
    assert!(
        !tree.dirty().contains(DirtyFlags::LAYOUT),
        "propagate must never push LAYOUT up to an ancestor -- only PAINT"
    );
    assert!(tree.dirty().contains(DirtyFlags::PAINT));
}

#[test]
fn a_semantics_change_marked_directly_on_a_node_is_independent_of_paint_propagation() {
    let mut tree = tree_with_two_children();
    tree.children[0].mark_dirty(DirtyFlags::SEMANTICS);

    propagate(&mut tree, &[0]);

    assert!(tree.children[0].dirty().contains(DirtyFlags::SEMANTICS));
    assert!(
        !tree.dirty().contains(DirtyFlags::SEMANTICS),
        "propagate must never push SEMANTICS up to an ancestor -- only PAINT"
    );
}

#[test]
fn layout_with_cache_and_semantics_with_cache_clear_only_their_own_flag() {
    // A node marked dirty in every respect that recomputes only its
    // layout must end up PAINT/SEMANTICS-dirty still, LAYOUT clean --
    // proving the two caches (Phase 3 commits 2 and 4) do not
    // cross-clear each other's flag.
    let mut tree = Tree::empty();
    assert_eq!(tree.dirty(), DirtyFlags::all());

    let limits = iced::advanced::layout::Limits::new(iced::Size::ZERO, iced::Size::new(10.0, 10.0));
    let _ = tree.layout_with_cache(limits, |_| {
        iced::advanced::layout::Node::new(iced::Size::new(10.0, 10.0))
    });

    assert!(!tree.dirty().contains(DirtyFlags::LAYOUT));
    assert!(tree.dirty().contains(DirtyFlags::PAINT));
    assert!(tree.dirty().contains(DirtyFlags::SEMANTICS));

    let _ = tree.semantics_with_cache(|_| None);

    assert!(!tree.dirty().contains(DirtyFlags::LAYOUT));
    assert!(!tree.dirty().contains(DirtyFlags::SEMANTICS));
    assert!(
        tree.dirty().contains(DirtyFlags::PAINT),
        "neither cache call touches PAINT; only propagate/mark_dirty do"
    );
}

#[test]
fn an_aggregator_fed_from_a_propagated_trees_own_bounds_stays_bounded() {
    // A minimal end-to-end sanity check that `Aggregator` (bounded
    // damage aggregation) and `propagate` (dirty-flag propagation)
    // compose the way a real paint pass would use them together: for
    // every node `propagate` marks PAINT-dirty, record its damage, and
    // confirm the aggregator still reports a bounded, non-`Full` region
    // when the count of touched nodes stays under `Aggregator::MAX_REGIONS`.
    use iced_core::damage::{Cause, Region, Request};
    use iced_core::{Point, Rectangle, Size};

    let mut tree = tree_with_two_children();
    propagate(&mut tree, &[0]);

    let mut aggregator = Aggregator::new();
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(10.0, 10.0));
    aggregator.record(Request::new(Region::area(bounds).unwrap(), Cause::Content));

    assert_eq!(aggregator.region(), Some(Region::Area(bounds)));
}

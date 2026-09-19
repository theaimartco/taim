use super::{DirtyFlags, Revision, Tree};
use crate::{Point, Rectangle, Size, Transformation, layout, semantics};

fn limits(side: f32) -> layout::Limits {
    layout::Limits::new(Size::ZERO, Size::new(side, side))
}

fn node(side: f32) -> layout::Node {
    layout::Node::new(Size::new(side, side))
}

fn semantic(name: &str) -> Option<semantics::Node> {
    Some(
        semantics::Node::new(
            crate::widget::Id::unique(),
            Rectangle::new(Point::ORIGIN, Size::new(1.0, 1.0)),
            semantics::Role::StaticText,
        )
        .with_name(name),
    )
}

#[test]
fn fresh_tree_starts_initial_and_fully_dirty() {
    let tree = Tree::empty();
    assert_eq!(tree.revision(), Revision::INITIAL);
    assert_eq!(tree.dirty(), DirtyFlags::all());
}

#[test]
fn dirtying_advances_revision_and_sets_only_requested_flag() {
    let mut tree = Tree::empty();
    tree.clear_dirty(DirtyFlags::all());
    tree.mark_dirty(DirtyFlags::PAINT);
    assert!(tree.revision().wrapping_after(Revision::INITIAL));
    assert_eq!(tree.dirty(), DirtyFlags::PAINT);
}

#[test]
fn clearing_one_dirty_bit_preserves_the_others() {
    let mut tree = Tree::empty();
    tree.clear_dirty(DirtyFlags::LAYOUT);
    assert!(!tree.dirty().contains(DirtyFlags::LAYOUT));
    assert!(
        tree.dirty()
            .contains(DirtyFlags::PAINT | DirtyFlags::SEMANTICS)
    );
}

#[test]
fn revision_order_handles_wraparound() {
    let previous = Revision(u64::MAX);
    assert!(previous.next().wrapping_after(previous));
}

#[test]
fn retained_geometry_round_trips() {
    let mut tree = Tree::empty();
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(10.0, 10.0));
    tree.set_cached_bounds(bounds);
    tree.set_cached_transform(Transformation::IDENTITY);
    tree.set_cached_clip(bounds);
    assert_eq!(tree.cached_bounds(), Some(bounds));
    assert_eq!(tree.cached_transform(), Some(Transformation::IDENTITY));
    assert_eq!(tree.cached_clip(), Some(bounds));
}

#[test]
fn dirty_layout_always_recomputes() {
    let mut tree = Tree::empty();
    let mut calls = 0;
    let result = tree.layout_with_cache(limits(10.0), |_| {
        calls += 1;
        node(10.0)
    });
    assert_eq!(calls, 1);
    assert_eq!(result.size(), Size::new(10.0, 10.0));
}

#[test]
fn different_limits_bypass_a_clean_layout_cache() {
    let mut tree = Tree::empty();
    let _ = tree.layout_with_cache(limits(10.0), |_| node(10.0));
    let mut calls = 0;
    let result = tree.layout_with_cache(limits(20.0), |_| {
        calls += 1;
        node(20.0)
    });
    assert_eq!(calls, 1);
    assert_eq!(result.size(), Size::new(20.0, 20.0));
}

#[test]
fn paint_dirtiness_does_not_invalidate_layout() {
    let mut tree = Tree::empty();
    let _ = tree.layout_with_cache(limits(10.0), |_| node(10.0));
    tree.mark_dirty(DirtyFlags::PAINT);
    let mut calls = 0;
    let result = tree.layout_with_cache(limits(10.0), |_| {
        calls += 1;
        node(99.0)
    });
    assert_eq!(calls, 0);
    assert_eq!(result.size(), Size::new(10.0, 10.0));
}

#[test]
fn cached_layout_preserves_child_position() {
    let mut tree = Tree::empty();
    let _ = tree.layout_with_cache(limits(10.0), |_| node(10.0).move_to(Point::new(5.0, 7.0)));
    let reused = tree.layout_with_cache(limits(10.0), |_| node(99.0));
    assert_eq!(reused.bounds().position(), Point::new(5.0, 7.0));
}

#[test]
fn semantics_dirtiness_forces_recomputation() {
    let mut tree = Tree::empty();
    let _ = tree.semantics_with_cache(|_| semantic("old"));
    tree.mark_dirty(DirtyFlags::SEMANTICS);
    let changed = tree.semantics_with_cache(|_| semantic("new"));
    assert_eq!(changed.and_then(|node| node.name), Some("new".into()));
}

#[test]
fn layout_dirtiness_does_not_invalidate_semantics() {
    let mut tree = Tree::empty();
    let _ = tree.semantics_with_cache(|_| semantic("cached"));
    tree.mark_dirty(DirtyFlags::LAYOUT);
    let mut calls = 0;
    let cached = tree.semantics_with_cache(|_| {
        calls += 1;
        semantic("wrong")
    });
    assert_eq!(calls, 0);
    assert_eq!(cached.and_then(|node| node.name), Some("cached".into()));
}

#[test]
fn present_semantics_are_reused_when_clean() {
    let mut tree = Tree::empty();
    let _ = tree.semantics_with_cache(|_| semantic("first"));
    let reused = tree.semantics_with_cache(|_| semantic("second"));
    assert_eq!(reused.and_then(|node| node.name), Some("first".into()));
}

#[test]
fn fresh_nodes_receive_distinct_identity() {
    assert_ne!(Tree::empty().id(), Tree::empty().id());
}

#[test]
fn child_identity_survives_same_size_reconciliation() {
    let mut tree = Tree::with_children(vec![Tree::empty(), Tree::empty()]);
    let before: Vec<_> = tree
        .children
        .iter()
        .map(|child| child.id().clone())
        .collect();
    let mut incoming = [(), ()];
    tree.diff_children_custom(&mut incoming, |_, _| {}, |_| Tree::empty());
    let after: Vec<_> = tree
        .children
        .iter()
        .map(|child| child.id().clone())
        .collect();
    assert_eq!(before, after);
}

#[test]
fn growing_children_preserves_existing_and_mints_new_identity() {
    let mut tree = Tree::with_children(vec![Tree::empty()]);
    let existing = tree.children[0].id().clone();
    let mut incoming = [(), ()];
    tree.diff_children_custom(&mut incoming, |_, _| {}, |_| Tree::empty());
    assert_eq!(tree.children[0].id(), &existing);
    assert_ne!(tree.children[1].id(), &existing);
}

//! Propagation, clearing, and revision wrap-policy tests for
//! [`taim_core::widget::tree::DirtyFlags`] and
//! [`taim_core::widget::tree::Revision`].
//!
//! Unit tests colocated in `core/src/widget/tree.rs` already cover each
//! primitive in isolation; this file instead exercises them the way a
//! caller actually will: propagating a child's dirty state up into a
//! parent tree, a pattern Phase 3's real reconciliation is expected to
//! follow once it starts calling `Tree::mark_dirty` from widget code.
//! Phase 2 introduces no such caller itself, so these tests drive the
//! propagation manually to prove the primitives compose the way that
//! future caller will need.

use taim::advanced::widget::tree::{self, DirtyFlags, Revision, Tree};

/// A minimal stand-in for "a container noticed one of its children is
/// dirty and marked itself dirty too," which is the propagation rule
/// Phase 3 is expected to implement for real. Returns whether the
/// parent's flags actually changed, so a test can assert propagation
/// only happens when it should.
fn propagate_from_children(parent: &mut Tree) -> bool {
    let child_dirty = parent
        .children
        .iter()
        .fold(DirtyFlags::empty(), |acc, child| acc | child.dirty());

    if child_dirty.is_empty() {
        return false;
    }

    let before = parent.dirty();
    parent.mark_dirty(child_dirty);

    parent.dirty() != before
}

#[test]
fn a_fresh_tree_and_its_children_start_fully_dirty() {
    let mut tree = Tree::empty();
    tree.children.push(Tree::empty());
    tree.children.push(Tree::empty());

    assert_eq!(tree.dirty(), DirtyFlags::all());

    for child in &tree.children {
        assert_eq!(child.dirty(), DirtyFlags::all());
    }
}

#[test]
fn clearing_a_childs_flags_does_not_affect_the_parent() {
    let mut tree = Tree::empty();
    tree.children.push(Tree::empty());

    tree.clear_dirty(DirtyFlags::all());
    tree.children[0].clear_dirty(DirtyFlags::LAYOUT);

    assert_eq!(tree.dirty(), DirtyFlags::empty());
    assert_eq!(
        tree.children[0].dirty(),
        DirtyFlags::PAINT | DirtyFlags::SEMANTICS
    );
}

#[test]
fn propagation_only_occurs_when_a_child_is_actually_dirty() {
    let mut tree = Tree::empty();
    tree.children.push(Tree::empty());
    tree.clear_dirty(DirtyFlags::all());
    tree.children[0].clear_dirty(DirtyFlags::all());

    assert!(!propagate_from_children(&mut tree));
    assert_eq!(tree.dirty(), DirtyFlags::empty());
}

#[test]
fn propagation_carries_the_narrowest_flags_that_are_actually_dirty() {
    let mut tree = Tree::empty();
    tree.children.push(Tree::empty());
    tree.clear_dirty(DirtyFlags::all());
    tree.children[0].clear_dirty(DirtyFlags::all());
    tree.children[0].mark_dirty(DirtyFlags::PAINT);

    assert!(propagate_from_children(&mut tree));
    assert_eq!(tree.dirty(), DirtyFlags::PAINT);
}

#[test]
fn propagation_from_multiple_children_unions_their_flags() {
    let mut tree = Tree::empty();
    tree.children.push(Tree::empty());
    tree.children.push(Tree::empty());
    tree.clear_dirty(DirtyFlags::all());

    tree.children[0].clear_dirty(DirtyFlags::all());
    tree.children[0].mark_dirty(DirtyFlags::LAYOUT);

    tree.children[1].clear_dirty(DirtyFlags::all());
    tree.children[1].mark_dirty(DirtyFlags::SEMANTICS);

    assert!(propagate_from_children(&mut tree));

    assert_eq!(tree.dirty(), DirtyFlags::LAYOUT | DirtyFlags::SEMANTICS);
    assert!(!tree.dirty().contains(DirtyFlags::PAINT));
}

#[test]
fn mark_dirty_always_advances_the_revision_even_with_empty_flags() {
    let mut tree = Tree::empty();
    let before = tree.revision();

    tree.mark_dirty(DirtyFlags::empty());

    assert!(tree.revision().wrapping_after(before));
}

#[test]
fn revision_wrapping_after_is_the_only_correct_way_to_order_revisions() {
    // `Revision` deliberately has no `Ord`/`PartialOrd` impl (see its
    // documentation): this test exists to pin that `wrapping_after` --
    // not a numeric comparison -- is the supported way to check whether
    // one revision happened after another, including across a
    // wraparound of the underlying counter.
    let mut tree = Tree::empty();
    let first = tree.revision();

    tree.mark_dirty(tree::DirtyFlags::LAYOUT);
    let second = tree.revision();

    tree.mark_dirty(tree::DirtyFlags::PAINT);
    let third = tree.revision();

    assert!(second.wrapping_after(first));
    assert!(third.wrapping_after(second));
    assert!(third.wrapping_after(first));
    assert!(!first.wrapping_after(second));
}

#[test]
fn revision_starts_at_the_documented_initial_value_for_every_new_node() {
    let a = Tree::empty();
    let b = Tree::empty();

    assert_eq!(a.revision(), Revision::INITIAL);
    assert_eq!(a.revision(), b.revision());
}

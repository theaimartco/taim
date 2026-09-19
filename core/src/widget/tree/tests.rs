use super::{DirtyFlags, Revision, Tree};
use crate::{Point, Rectangle, Size, layout, semantics};

fn limits(side: f32) -> layout::Limits {
    layout::Limits::new(Size::ZERO, Size::new(side, side))
}

fn square(side: f32) -> layout::Node {
    layout::Node::new(Size::new(side, side))
}

#[test]
fn cold_cache_is_allocated_only_when_used() {
    let mut tree = Tree::empty();

    assert!(tree.cache.is_none());
    assert!(std::mem::size_of::<Tree>() <= 128);

    tree.set_cached_bounds(Rectangle::new(Point::ORIGIN, Size::new(8.0, 8.0)));

    assert!(tree.cache.is_some());
}

#[test]
fn layout_cache_obeys_limits_and_dirty_bits() {
    let mut tree = Tree::empty();
    let mut calls = 0;

    let _ = tree.layout_with_cache(limits(8.0), |_| {
        calls += 1;
        square(8.0)
    });
    let cached = tree.layout_with_cache(limits(8.0), |_| {
        calls += 1;
        square(99.0)
    });

    assert_eq!(calls, 1);
    assert_eq!(cached.size(), Size::new(8.0, 8.0));

    tree.mark_dirty(DirtyFlags::LAYOUT);
    let changed = tree.layout_with_cache(limits(8.0), |_| {
        calls += 1;
        square(12.0)
    });

    assert_eq!(calls, 2);
    assert_eq!(changed.size(), Size::new(12.0, 12.0));
}

#[test]
fn semantics_cache_distinguishes_cached_none() {
    let mut tree = Tree::empty();
    let mut calls = 0;

    assert_eq!(
        tree.semantics_with_cache(|_| {
            calls += 1;
            None
        }),
        None
    );
    assert_eq!(
        tree.semantics_with_cache(|_| {
            calls += 1;
            Some(semantics::Node::new(
                crate::widget::Id::unique(),
                Rectangle::new(Point::ORIGIN, Size::new(1.0, 1.0)),
                semantics::Role::StaticText,
            ))
        }),
        None
    );

    assert_eq!(calls, 1);
}

#[test]
fn revisions_and_child_identity_survive_reconciliation() {
    let mut parent = Tree::with_children(vec![Tree::empty()]);
    let child_id = parent.children[0].id().clone();

    parent.clear_dirty(DirtyFlags::all());
    parent.mark_dirty(DirtyFlags::PAINT);

    assert!(parent.revision().wrapping_after(Revision::INITIAL));
    assert_eq!(parent.dirty(), DirtyFlags::PAINT);

    let mut incoming = [(), ()];
    parent.diff_children_custom(&mut incoming, |_, _| {}, |_| Tree::empty());

    assert_eq!(parent.children[0].id(), &child_id);
    assert_ne!(parent.children[1].id(), &child_id);
}

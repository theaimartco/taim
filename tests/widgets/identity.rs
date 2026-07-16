//! List reorder, insertion, removal, and preserved-state tests.
//!
//! These tests exercise `iced_widget::keyed::Column`'s reconciliation,
//! which as of Phase 3 is backed by `iced_runtime::reconcile::reconcile`
//! -- a real, `O(n)` keyed diff. This is a behavior change from Phase 2:
//! that phase's version of this file documented (and tested) a
//! narrower, boundary-splice-only contract
//! (`iced_core::widget::tree::diff_children_custom_with_search`) that did
//! not consult keys at all when the list's length was unchanged, and
//! never relocated a slot to follow its key across a full reorder. Both
//! limitations are gone: every test below holds regardless of how the
//! key set is rearranged, grown, or shrunk, because [`reconcile`] looks
//! up every new key against every old key (not just the boundaries).
//!
//! [`reconcile`]: iced_runtime::reconcile::reconcile

use iced::advanced::widget::{self, Widget};
use iced::advanced::{Layout, mouse, renderer};
use iced::widget::keyed::Column;
use iced::{Element, Length, Rectangle, Size, Theme};
use iced_renderer::Renderer;
use iced_runtime::user_interface::{self, UserInterface};

/// A widget whose only job is to hold an identifiable counter in its
/// [`widget::Tree`] state, incremented once when the widget is first
/// created and left untouched by every subsequent `diff`. If
/// reconciliation preserves this widget's [`widget::Tree`] node, the
/// counter a test reads back later will match what was recorded the
/// first time; if reconciliation instead discarded and recreated the
/// node, the counter resets to zero.
#[derive(Debug, Clone, Copy)]
struct Marker(u32);

impl<Message> Widget<Message, Theme, Renderer> for Marker {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(20.0), Length::Fixed(20.0))
    }

    fn layout(
        &mut self,
        _tree: &mut widget::Tree,
        _renderer: &Renderer,
        _limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        iced::advanced::layout::Node::new(Size::new(20.0, 20.0))
    }

    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<u32>()
    }

    fn state(&self) -> widget::tree::State {
        // Only used the first time this widget is created for a given
        // tree slot; a preserved node never calls this again.
        widget::tree::State::new(self.0)
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }
}

fn marker<Message>(id: u32) -> Element<'static, Message> {
    Element::new(Marker(id))
}

/// Reads back the [`Marker`] state stored at `tree.children[index]`.
fn marker_state(tree: &widget::Tree, index: usize) -> u32 {
    *tree.children[index].state.downcast_ref::<u32>()
}

fn headless_renderer() -> Renderer {
    iced_futures::futures::executor::block_on(
        <Renderer as iced::advanced::renderer::Headless>::new(
            iced::advanced::renderer::Settings::default(),
            Some("tiny-skia"),
        ),
    )
    .expect("tiny-skia renderer must be available")
}

/// Builds a fresh [`widget::Tree`] for a keyed column with the given
/// `(key, marker_id)` pairs, as if this were the first `view` call.
///
/// Mirrors what `UserInterface::build` does internally: `Tree::new`
/// alone never populates children (that is `Widget::diff`'s job, called
/// once even for the very first build -- see
/// `runtime::user_interface::UserInterface::build`), so an initial
/// `diff` against the same column is required here too.
fn tree_for(entries: &[(u32, u32)]) -> widget::Tree {
    let mut column: Column<'_, u32, (), Theme, Renderer> = Column::from_vecs(
        entries.iter().map(|(key, _)| *key).collect(),
        entries.iter().map(|(_, id)| marker(*id)).collect(),
    );

    let mut tree = widget::Tree::new(&column as &dyn Widget<(), Theme, Renderer>);
    tree.diff(&mut column as &mut dyn Widget<(), Theme, Renderer>);
    tree
}

/// Diffs `tree` against a keyed column built from `entries`, mirroring
/// what `UserInterface::build`'s internal `Cache::diff` call does on a
/// subsequent `view`.
fn diff(tree: &mut widget::Tree, entries: &[(u32, u32)]) {
    let mut column: Column<'_, u32, (), Theme, Renderer> = Column::from_vecs(
        entries.iter().map(|(key, _)| *key).collect(),
        entries.iter().map(|(_, id)| marker(*id)).collect(),
    );

    tree.diff(&mut column as &mut dyn Widget<(), Theme, Renderer>);
}

#[test]
fn removing_the_last_key_preserves_the_remaining_items_state_and_order() {
    let mut tree = tree_for(&[(1, 100), (2, 200), (3, 300)]);

    diff(&mut tree, &[(1, 999), (2, 999)]);

    assert_eq!(tree.children.len(), 2);
    assert_eq!(marker_state(&tree, 0), 100);
    assert_eq!(marker_state(&tree, 1), 200);
}

#[test]
fn removing_the_first_key_preserves_the_remaining_items_state_and_order() {
    let mut tree = tree_for(&[(1, 100), (2, 200), (3, 300)]);

    diff(&mut tree, &[(2, 999), (3, 999)]);

    assert_eq!(tree.children.len(), 2);
    assert_eq!(marker_state(&tree, 0), 200);
    assert_eq!(marker_state(&tree, 1), 300);
}

#[test]
fn removing_every_key_leaves_an_empty_tree() {
    let mut tree = tree_for(&[(1, 100), (2, 200)]);

    diff(&mut tree, &[]);

    assert!(tree.children.is_empty());
}

#[test]
fn appending_a_key_preserves_the_existing_items_state() {
    let mut tree = tree_for(&[(1, 100), (2, 200)]);

    diff(&mut tree, &[(1, 999), (2, 999), (3, 300)]);

    assert_eq!(tree.children.len(), 3);

    // A real keyed diff walks the new key list in order, so the result
    // matches the new list's order exactly: keys 1 and 2 keep their
    // state, and the appended key 3 is fresh, landing at the end.
    assert_eq!(marker_state(&tree, 0), 100, "key 1's state survives");
    assert_eq!(marker_state(&tree, 1), 200, "key 2's state survives");
    assert_eq!(
        marker_state(&tree, 2),
        300,
        "key 3 is fresh, from its build argument"
    );
}

#[test]
fn prepending_a_key_preserves_the_existing_items_state() {
    let mut tree = tree_for(&[(2, 200), (3, 300)]);

    diff(&mut tree, &[(1, 100), (2, 999), (3, 999)]);

    assert_eq!(tree.children.len(), 3);

    assert_eq!(
        marker_state(&tree, 0),
        100,
        "key 1 is fresh, from its build argument"
    );
    assert_eq!(marker_state(&tree, 1), 200, "key 2's state survives");
    assert_eq!(marker_state(&tree, 2), 300, "key 3's state survives");
}

#[test]
fn a_same_length_disjoint_key_set_recreates_every_item_with_its_new_state() {
    let mut tree = tree_for(&[(1, 100), (2, 200)]);

    // Same length (2 -> 2), but no key overlap: unlike Phase 2's
    // boundary-splice algorithm (which ignored keys entirely when the
    // length didn't change), a real keyed diff has nothing to preserve
    // here regardless of length, so both slots must reflect the new
    // build arguments.
    diff(&mut tree, &[(10, 111), (20, 222)]);

    assert_eq!(tree.children.len(), 2);
    assert_eq!(marker_state(&tree, 0), 111);
    assert_eq!(marker_state(&tree, 1), 222);
}

#[test]
fn a_same_length_full_reorder_relocates_each_slot_to_follow_its_key() {
    let mut tree = tree_for(&[(1, 100), (2, 200), (3, 300)]);

    // The exact case Phase 2's algorithm could not handle: a full
    // reversal at unchanged length. A real keyed diff relocates each
    // slot to wherever its key ends up.
    diff(&mut tree, &[(3, 999), (2, 999), (1, 999)]);

    assert_eq!(tree.children.len(), 3);
    assert_eq!(marker_state(&tree, 0), 300, "key 3's state follows key 3");
    assert_eq!(marker_state(&tree, 1), 200, "key 2's state follows key 2");
    assert_eq!(marker_state(&tree, 2), 100, "key 1's state follows key 1");
}

#[test]
fn an_unchanged_key_set_does_not_recreate_any_item() {
    let mut tree = tree_for(&[(1, 100), (2, 200), (3, 300)]);

    // Same keys, same order; every item's state must be exactly what it
    // was, regardless of the (irrelevant, since nothing is recreated)
    // build arguments passed this time.
    diff(&mut tree, &[(1, 999), (2, 999), (3, 999)]);

    assert_eq!(marker_state(&tree, 0), 100);
    assert_eq!(marker_state(&tree, 1), 200);
    assert_eq!(marker_state(&tree, 2), 300);
}

#[test]
fn ui_build_and_layout_succeeds_with_keyed_columns() {
    // End-to-end smoke check: a keyed column drives real layout through
    // `UserInterface::build`, not just the lower-level `Tree::diff` this
    // file otherwise exercises directly.
    let mut renderer = headless_renderer();

    let column: Column<'_, u32, (), Theme, Renderer> =
        Column::from_vecs(vec![1, 2, 3], vec![marker(1), marker(2), marker(3)]);

    let ui = UserInterface::build(
        Element::from(column),
        Size::new(200.0, 200.0),
        user_interface::Cache::new(),
        &mut renderer,
    );

    let _ = ui.into_cache();
}

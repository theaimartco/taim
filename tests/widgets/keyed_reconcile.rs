//! Nested-identity reconciliation tests for `taim_widget::keyed::Column`.
//!
//! `tests/widgets/identity.rs` (Phase 3, commit 1) already covers
//! reorder/insert/remove/replace at the *top level* of a keyed column,
//! using a flat `Marker` widget whose own state is a single counter.
//! These tests instead put a **container** behind each key -- a small
//! `row`-like wrapper holding its own nested `Marker` -- to confirm
//! `taim_runtime::reconcile::reconcile` preserves state correctly one
//! level deeper than the keyed slot itself, since a real application's
//! keyed list rows are essentially never a single leaf widget.

type Element<'a, M, T = taim_core::Theme, R = taim_renderer::Renderer> =
    taim_core::Element<'a, M, T, R>;
use taim_core::widget::{self, Widget};
use taim_core::{Layout, mouse, renderer};
use taim_core::{Length, Point, Rectangle, Size, Theme};
use taim_renderer::Renderer;
use taim_widget::keyed::Column;

/// A single-child wrapper, so each keyed row has a nested `Tree` level
/// (`tree.children[i].children[0]`) whose own state must survive
/// reconciliation, not just the row's own top-level slot.
struct Row<Message> {
    child: Element<'static, Message>,
}

impl<Message> Widget<Message, Theme, Renderer> for Row<Message> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(20.0), Length::Fixed(20.0))
    }

    fn diff(&mut self, tree: &mut widget::Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.child));
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &taim_core::layout::Limits,
    ) -> taim_core::layout::Node {
        let child_node = self
            .child
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);

        taim_core::layout::Node::with_children(Size::new(20.0, 20.0), vec![child_node])
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

/// A leaf holding an identifiable counter in its own `widget::Tree`
/// state, set once at creation and never touched again by `diff` -- the
/// same probe `tests/widgets/identity.rs` uses, nested one level deeper
/// here (behind a `Row`) instead of sitting directly in the keyed
/// column's own slot.
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
        _limits: &taim_core::layout::Limits,
    ) -> taim_core::layout::Node {
        taim_core::layout::Node::new(Size::new(20.0, 20.0))
    }

    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<u32>()
    }

    fn state(&self) -> widget::tree::State {
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

fn row<Message: 'static>(id: u32) -> Element<'static, Message> {
    Element::new(Row {
        child: Element::new(Marker(id)),
    })
}

/// Reads back the nested [`Marker`] state one level behind the keyed
/// slot at `tree.children[index]`.
fn nested_marker_state(tree: &widget::Tree, index: usize) -> u32 {
    *tree.children[index].children[0].state.downcast_ref::<u32>()
}

fn tree_for(entries: &[(u32, u32)]) -> widget::Tree {
    let mut column: Column<'_, u32, (), Theme, Renderer> = Column::from_vecs(
        entries.iter().map(|(key, _)| *key).collect(),
        entries.iter().map(|(_, id)| row(*id)).collect(),
    );

    let mut tree = widget::Tree::new(&column as &dyn Widget<(), Theme, Renderer>);
    tree.diff(&mut column as &mut dyn Widget<(), Theme, Renderer>);
    tree
}

fn diff(tree: &mut widget::Tree, entries: &[(u32, u32)]) {
    let mut column: Column<'_, u32, (), Theme, Renderer> = Column::from_vecs(
        entries.iter().map(|(key, _)| *key).collect(),
        entries.iter().map(|(_, id)| row(*id)).collect(),
    );

    tree.diff(&mut column as &mut dyn Widget<(), Theme, Renderer>);
}

#[test]
fn a_full_reorder_relocates_each_rows_nested_state_to_follow_its_key() {
    let mut tree = tree_for(&[(1, 100), (2, 200), (3, 300)]);

    diff(&mut tree, &[(3, 999), (2, 999), (1, 999)]);

    assert_eq!(tree.children.len(), 3);
    assert_eq!(
        nested_marker_state(&tree, 0),
        300,
        "key 3's nested Marker state follows key 3, one level behind the Row"
    );
    assert_eq!(nested_marker_state(&tree, 1), 200);
    assert_eq!(
        nested_marker_state(&tree, 2),
        100,
        "key 1's nested Marker state follows key 1"
    );
}

#[test]
fn inserting_a_key_in_the_middle_preserves_neighbors_nested_state() {
    let mut tree = tree_for(&[(1, 100), (3, 300)]);

    diff(&mut tree, &[(1, 999), (2, 200), (3, 999)]);

    assert_eq!(tree.children.len(), 3);
    assert_eq!(nested_marker_state(&tree, 0), 100, "key 1 survives");
    assert_eq!(
        nested_marker_state(&tree, 1),
        200,
        "key 2 is fresh, from its build argument"
    );
    assert_eq!(nested_marker_state(&tree, 2), 300, "key 3 survives");
}

#[test]
fn removing_a_key_from_the_middle_preserves_the_remaining_rows_nested_state() {
    let mut tree = tree_for(&[(1, 100), (2, 200), (3, 300)]);

    diff(&mut tree, &[(1, 999), (3, 999)]);

    assert_eq!(tree.children.len(), 2);
    assert_eq!(nested_marker_state(&tree, 0), 100);
    assert_eq!(nested_marker_state(&tree, 1), 300);
}

#[test]
fn replacing_every_key_gives_every_row_fresh_nested_state() {
    let mut tree = tree_for(&[(1, 100), (2, 200)]);

    diff(&mut tree, &[(10, 111), (20, 222)]);

    assert_eq!(nested_marker_state(&tree, 0), 111);
    assert_eq!(nested_marker_state(&tree, 1), 222);
}

#[test]
fn a_keyed_columns_layout_still_succeeds_with_nested_rows() {
    let renderer = taim_futures::futures::executor::block_on(
        <Renderer as taim_core::renderer::Headless>::new(
            taim_core::renderer::Settings::default(),
            Some("tiny-skia"),
        ),
    )
    .expect("tiny-skia renderer must be available");

    let mut column: Column<'_, u32, (), Theme, Renderer> =
        Column::from_vecs(vec![1, 2, 3], vec![row(1), row(2), row(3)]);

    let mut tree = widget::Tree::new(&column as &dyn Widget<(), Theme, Renderer>);
    tree.diff(&mut column as &mut dyn Widget<(), Theme, Renderer>);

    let limits = taim_core::layout::Limits::new(Size::ZERO, Size::new(200.0, 200.0));
    let node = Widget::<(), Theme, Renderer>::layout(&mut column, &mut tree, &renderer, &limits);

    assert_eq!(node.children().len(), 3);
    assert_eq!(node.bounds().x, Point::ORIGIN.x);
}

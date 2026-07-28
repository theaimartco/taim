//! Overlapping, clipped, transformed, and captured path tests for
//! `aura_runtime::hit_test`.
//!
//! These tests exercise the linear hit-test oracle
//! (`aura_runtime::hit_test::hit_test`) two ways:
//!
//! - Directly against real widget layouts via [`Widget::hit_test`]
//!   (Phase 2's per-widget hit-test method, defaulted to a rectangular
//!   bounds check), for the "does this point register as hit at all"
//!   question a real application asks every frame.
//! - Through `aura_core::widget::visit::Widgets` -- the same adapter
//!   `UserInterface::hit_test` uses -- to prove the path-producing
//!   oracle agrees with those same widgets' bounds, since
//!   `hit_test::hit_test`'s own unit tests only exercise a synthetic
//!   `Node` type, not real `aura_core::Widget`s.
//!
//! These used to walk a hand-built `Shape` mirroring the real tree,
//! because `Widget` had no way to enumerate children; each test
//! therefore constructed its widgets *twice* and trusted the two copies
//! to match. `Widget::visit_children` removed the need, so what is
//! walked here is now the tree itself.

use aura::advanced::Layout;
use aura::advanced::widget::{self, Widget};
use aura::{Element, Length, Point, Rectangle, Size, Theme, Vector};
use aura_core::widget::visit;
use aura_renderer::Renderer;
use aura_runtime::hit_test;

fn headless_renderer() -> Renderer {
    aura_futures::futures::executor::block_on(
        <Renderer as aura::advanced::renderer::Headless>::new(
            aura::advanced::renderer::Settings::default(),
            Some("tiny-skia"),
        ),
    )
    .expect("tiny-skia renderer must be available")
}

/// A simple fixed-size leaf widget for building test trees.
struct Probe {
    size: f32,
}

impl<Message> Widget<Message, Theme, Renderer> for Probe {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.size), Length::Fixed(self.size))
    }

    fn layout(
        &mut self,
        _tree: &mut widget::Tree,
        _renderer: &Renderer,
        _limits: &aura::advanced::layout::Limits,
    ) -> aura::advanced::layout::Node {
        aura::advanced::layout::Node::new(Size::new(self.size, self.size))
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &aura::advanced::renderer::Style,
        _layout: Layout<'_>,
        _cursor: aura::advanced::mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }
}

fn probe(size: f32) -> Element<'static, ()> {
    Element::new(Probe { size })
}

/// Walks `root`'s real widget tree, laid out, and returns the [`Path`]
/// to the deepest widget containing `point`.
///
/// [`Path`]: hit_test::Path
fn path_to(root: &mut Element<'_, ()>, point: Point) -> Option<hit_test::Path> {
    let renderer = headless_renderer();

    let mut tree = widget::Tree::new(root.as_widget());
    tree.diff(root.as_widget_mut());

    let limits = aura::advanced::layout::Limits::new(Size::ZERO, Size::new(200.0, 200.0));
    let node = root.as_widget_mut().layout(&mut tree, &renderer, &limits);

    hit_test::hit_test(
        &mut visit::Widgets::new(root.as_widget_mut(), &mut tree, Layout::new(&node)),
        point,
    )
}

#[test]
fn overlapping_siblings_resolve_to_the_topmost_via_the_oracle() {
    use aura::widget::stack;

    let mut root: Element<'_, ()> = stack([probe(50.0), probe(50.0)]).into();

    let path =
        path_to(&mut root, Point::new(10.0, 10.0)).expect("the overlapping region must be hit");

    assert_eq!(
        path.indices(),
        &[1],
        "the later (topmost) sibling must win, matching aura's own back-to-front draw order"
    );
}

#[test]
fn a_point_outside_every_widget_is_not_hit() {
    let renderer = headless_renderer();

    let mut root: Element<'_, ()> = probe(50.0);

    let mut tree = widget::Tree::new(root.as_widget());
    tree.diff(root.as_widget_mut());

    let limits = aura::advanced::layout::Limits::new(Size::ZERO, Size::new(200.0, 200.0));
    let node = root.as_widget_mut().layout(&mut tree, &renderer, &limits);
    let layout = Layout::new(&node);

    let hit = root
        .as_widget()
        .hit_test(&tree, layout, Point::new(500.0, 500.0));

    assert!(!hit);

    assert_eq!(path_to(&mut root, Point::new(500.0, 500.0)), None);
}

#[test]
fn a_translated_layout_hit_tests_at_its_new_position_not_the_old_one() {
    let renderer = headless_renderer();

    let mut root: Element<'_, ()> = probe(50.0);

    let mut tree = widget::Tree::new(root.as_widget());
    tree.diff(root.as_widget_mut());

    let limits = aura::advanced::layout::Limits::new(Size::ZERO, Size::new(200.0, 200.0));
    let node = root
        .as_widget_mut()
        .layout(&mut tree, &renderer, &limits)
        .translate(Vector::new(100.0, 100.0));
    let layout = Layout::new(&node);

    assert!(
        !root
            .as_widget()
            .hit_test(&tree, layout, Point::new(10.0, 10.0)),
        "the pre-translation position must no longer be hit"
    );

    assert!(
        root.as_widget()
            .hit_test(&tree, layout, Point::new(110.0, 110.0)),
        "the post-translation position must be hit"
    );
}

#[test]
fn content_clipped_outside_a_scrollables_bounds_is_not_hit() {
    use aura::widget::{container, scrollable};

    let renderer = headless_renderer();

    // A `scrollable` clips its content to its own bounds; content laid
    // out far beyond those bounds should not register as hit through
    // the scrollable's own `hit_test` (which, defaulted from Phase 2,
    // checks the scrollable's own layout bounds -- not its unclipped
    // content's).
    let mut root: Element<'_, ()> = scrollable(container(probe(50.0)).height(1000.0))
        .height(100.0)
        .into();

    let mut tree = widget::Tree::new(root.as_widget());
    tree.diff(root.as_widget_mut());

    let limits = aura::advanced::layout::Limits::new(Size::ZERO, Size::new(200.0, 200.0));
    let node = root.as_widget_mut().layout(&mut tree, &renderer, &limits);
    let layout = Layout::new(&node);

    let far_below_visible_area = Point::new(10.0, 500.0);

    assert!(
        !root
            .as_widget()
            .hit_test(&tree, layout, far_below_visible_area),
        "content scrolled/clipped out of view must not register as hit \
         against the scrollable's own bounds"
    );
}

#[test]
fn a_nested_child_reached_through_two_levels_returns_the_full_path() {
    use aura::widget::row;

    let mut root: Element<'_, ()> =
        row([row([probe(10.0), probe(10.0)]).into(), probe(10.0)]).into();

    // The first inner row's second probe sits at x offset 10.0 (past
    // the first 10.0-wide probe), well inside the outer row's first
    // slot.
    let point_in_nested_child = Point::new(15.0, 5.0);

    let path = path_to(&mut root, point_in_nested_child)
        .expect("point should land inside the nested row's second probe");

    assert_eq!(path.indices(), &[0, 1]);
}

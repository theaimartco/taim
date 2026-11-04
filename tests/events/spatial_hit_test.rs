//! Compares `taim_runtime::hit_test::Index` against the exhaustive
//! linear oracle (`taim_runtime::hit_test::hit_test`) over real widget
//! trees, complementing `hit_test.rs`'s own unit tests (which only
//! exercise a synthetic `Node` type, not real `taim_core::Widget`s).
//!
//! Walks the real tree via `taim_core::widget::visit::Widgets`, the same
//! adapter `UserInterface::hit_test` uses. This file used to walk a
//! hand-built `Shape` mirroring the tree, and so constructed every
//! widget twice and trusted the copies to match.
//!
//! `Index` is documented as provably as correct as the oracle by
//! construction (same recursive descent, pruned via the grid rather
//! than substituted for a separately-trusted heuristic); this suite is
//! the "index always agrees with exhaustive hit testing" half of
//! `phase.md`'s own Phase 8 exit gate, proven against genuine widget
//! layouts (overlapping siblings, nested rows) rather than only the
//! grid-of-leaves stress test `hit_test.rs` runs internally.

type Element<'a, M, T = taim_core::Theme, R = taim_renderer::Renderer> =
    taim_core::Element<'a, M, T, R>;
use taim_core::Layout;
use taim_core::widget::visit;
use taim_core::widget::{self, Widget};
use taim_core::{Length, Point, Rectangle, Size, Theme};
use taim_renderer::Renderer;
use taim_runtime::hit_test::{self, Index};

fn headless_renderer() -> Renderer {
    taim_futures::futures::executor::block_on(<Renderer as taim_core::renderer::Headless>::new(
        taim_core::renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .expect("tiny-skia renderer must be available")
}

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
        _limits: &taim_core::layout::Limits,
    ) -> taim_core::layout::Node {
        taim_core::layout::Node::new(Size::new(self.size, self.size))
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &taim_core::renderer::Style,
        _layout: Layout<'_>,
        _cursor: taim_core::mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }
}

fn probe(size: f32) -> Element<'static, ()> {
    Element::new(Probe { size })
}

/// A laid-out widget tree, kept alive so a `visit::Widgets` can be
/// re-borrowed from it for each query.
///
/// `visit::Widgets` borrows mutably (a `lazy` widget must be able to
/// build its children), so unlike the `Copy` adapter this file used to
/// carry, one cannot be held across queries -- hence the closure.
struct Tree<'a> {
    root: Element<'a, ()>,
    tree: widget::Tree,
    node: taim_core::layout::Node,
}

impl<'a> Tree<'a> {
    fn new(root: impl Into<Element<'a, ()>>, limits: Size) -> Self {
        let renderer = headless_renderer();
        let mut root = root.into();

        let mut tree = widget::Tree::new(root.as_widget());
        tree.diff(root.as_widget_mut());

        let node = root.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &taim_core::layout::Limits::new(Size::ZERO, limits),
        );

        Self { root, tree, node }
    }

    fn visit(&mut self) -> visit::Widgets<'_, (), Theme, Renderer> {
        visit::Widgets::new(
            self.root.as_widget_mut(),
            &mut self.tree,
            Layout::new(&self.node),
        )
    }

    fn oracle(&mut self, point: Point) -> Option<hit_test::Path> {
        hit_test::hit_test(&mut self.visit(), point)
    }

    fn index(&mut self) -> Index {
        Index::build(&mut self.visit())
    }

    fn index_with_cell_size(&mut self, cell_size: f32) -> Index {
        Index::build_with_cell_size(&mut self.visit(), cell_size)
    }

    fn via(&mut self, index: &Index, point: Point) -> Option<hit_test::Path> {
        index.hit_test(&mut self.visit(), point)
    }
}

/// Asserts `Index::hit_test` and the oracle agree at every point in a
/// dense grid spanning `bounds`.
fn assert_index_matches_oracle_everywhere(tree: &mut Tree<'_>, bounds: Rectangle, step: f32) {
    let index = tree.index();

    let mut x = bounds.x;
    while x <= bounds.x + bounds.width {
        let mut y = bounds.y;
        while y <= bounds.y + bounds.height {
            let point = Point::new(x, y);

            let expected = tree.oracle(point);
            let actual = tree.via(&index, point);

            assert_eq!(
                actual, expected,
                "Index disagreed with the oracle at {point:?}"
            );

            y += step;
        }
        x += step;
    }
}

#[test]
fn the_index_agrees_with_the_oracle_over_overlapping_siblings() {
    use taim_widget::stack;

    let mut tree = Tree::new(
        stack([probe(50.0), probe(50.0), probe(50.0)]),
        Size::new(200.0, 200.0),
    );

    assert_index_matches_oracle_everywhere(
        &mut tree,
        Rectangle::new(Point::new(-10.0, -10.0), Size::new(220.0, 220.0)),
        5.0,
    );
}

#[test]
fn the_index_agrees_with_the_oracle_over_a_dense_grid_of_rows() {
    use taim_widget::row;

    let mut tree = Tree::new(
        row((0..20).map(|_| probe(10.0))).spacing(2),
        Size::new(1000.0, 100.0),
    );

    assert_index_matches_oracle_everywhere(
        &mut tree,
        Rectangle::new(Point::new(-5.0, -5.0), Size::new(260.0, 20.0)),
        1.0,
    );
}

#[test]
fn the_index_agrees_with_the_oracle_over_a_nested_two_level_tree() {
    use taim_widget::row;

    let mut tree = Tree::new(
        row([row([probe(10.0), probe(10.0)]).into(), probe(10.0)]),
        Size::new(200.0, 200.0),
    );

    assert_index_matches_oracle_everywhere(
        &mut tree,
        Rectangle::new(Point::new(-5.0, -5.0), Size::new(40.0, 20.0)),
        1.0,
    );
}

#[test]
fn the_index_agrees_with_the_oracle_using_a_smaller_custom_cell_size() {
    use taim_widget::stack;

    let mut tree = Tree::new(stack([probe(50.0), probe(50.0)]), Size::new(200.0, 200.0));

    // A cell size much smaller than the widget bounds (forcing several
    // grid cells per leaf) must agree with the oracle exactly as much
    // as the default cell size does.
    let index = tree.index_with_cell_size(8.0);

    let mut x = -10.0;
    while x <= 210.0 {
        let mut y = -10.0;
        while y <= 210.0 {
            let point = Point::new(x, y);

            assert_eq!(tree.via(&index, point), tree.oracle(point));

            y += 5.0;
        }
        x += 5.0;
    }
}

#[test]
fn a_point_outside_every_widget_agrees_as_none() {
    let mut tree = Tree::new(probe(50.0), Size::new(200.0, 200.0));

    let index = tree.index();
    let far_away = Point::new(10_000.0, 10_000.0);

    assert_eq!(tree.via(&index, far_away), None);
    assert_eq!(tree.oracle(far_away), None);
}

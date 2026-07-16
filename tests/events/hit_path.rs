//! Overlapping, clipped, transformed, and captured path tests for
//! `iced_runtime::hit_test`.
//!
//! These tests exercise the linear hit-test oracle
//! (`iced_runtime::hit_test::hit_test`) two ways:
//!
//! - Directly against real widget layouts via [`Widget::hit_test`]
//!   (Phase 2's per-widget hit-test method, defaulted to a rectangular
//!   bounds check), for the "does this point register as hit at all"
//!   question a real application asks every frame.
//! - Through the [`Adapter`] wrapper implementing
//!   `iced_runtime::hit_test::Hoverable`, to prove the path-producing
//!   oracle itself agrees with those same widgets' bounds -- since
//!   `hit_test::hit_test`'s own unit tests only exercise a synthetic
//!   `Node` type, not real `iced_core::Widget`s.

use iced::advanced::Layout;
use iced::advanced::widget::{self, Widget};
use iced::{Element, Length, Point, Rectangle, Size, Theme, Vector};
use iced_renderer::Renderer;
use iced_runtime::hit_test::{self, Hoverable};

fn headless_renderer() -> Renderer {
    iced_futures::futures::executor::block_on(
        <Renderer as iced::advanced::renderer::Headless>::new(
            iced::advanced::renderer::Settings::default(),
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
        _limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        iced::advanced::layout::Node::new(Size::new(self.size, self.size))
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &iced::advanced::renderer::Style,
        _layout: Layout<'_>,
        _cursor: iced::advanced::mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }
}

fn probe(size: f32) -> Element<'static, ()> {
    Element::new(Probe { size })
}

/// A second, independent tree of widgets mirroring the shape of the
/// real [`Element`] tree consumed by `row`/`stack`/etc. below.
///
/// `Element` wraps `Box<dyn Widget>` and is not `Clone`, so the tests
/// cannot simply clone the tree they hand to the widget combinators;
/// instead they build this parallel, equally-cheap description (since
/// `probe` is a pure constructor) for [`Adapter`] to walk.
struct Shape<'a> {
    widget: &'a Element<'a, ()>,
    children: Vec<Shape<'a>>,
}

impl<'a> Shape<'a> {
    fn leaf(widget: &'a Element<'a, ()>) -> Self {
        Self {
            widget,
            children: Vec::new(),
        }
    }

    fn branch(widget: &'a Element<'a, ()>, children: Vec<Shape<'a>>) -> Self {
        Self { widget, children }
    }
}

/// Adapts a real `(&dyn Widget, &Tree, Layout)` triple to
/// `hit_test::Hoverable`, so the path-producing oracle can be exercised
/// against genuine widget bounds instead of only the synthetic `Node`
/// type used in `runtime/src/hit_test.rs`'s own unit tests.
///
/// Children are supplied explicitly via [`Shape`] rather than
/// discovered generically, since `Widget` has no public "get my
/// children as widgets" accessor; the tests below build a `Shape`
/// mirroring each root [`Element`]'s actual children, recursively.
#[derive(Clone, Copy)]
struct Adapter<'a> {
    tree: &'a widget::Tree,
    layout: Layout<'a>,
    shape: &'a Shape<'a>,
}

impl<'a> Hoverable for Adapter<'a> {
    fn contains(&self, point: Point) -> bool {
        self.shape
            .widget
            .as_widget()
            .hit_test(self.tree, self.layout, point)
    }

    fn child_count(&self) -> usize {
        self.shape.children.len()
    }

    fn child(&self, index: usize) -> (Self, Layout<'_>) {
        let child_layout = self
            .layout
            .children()
            .nth(index)
            .expect("index must be within child_count");

        let adapter = Adapter {
            tree: &self.tree.children[index],
            layout: child_layout,
            shape: &self.shape.children[index],
        };

        (adapter, child_layout)
    }
}

#[test]
fn overlapping_siblings_resolve_to_the_topmost_via_the_oracle() {
    use iced::widget::stack;

    let renderer = headless_renderer();

    // Built twice (once to inspect via `Adapter`/`Shape`, once consumed
    // by `stack`): `Element` holds a `Box<dyn Widget>` and is not
    // `Clone`, but `probe` is a pure, deterministic constructor, so the
    // two sets are equivalent.
    let children = [probe(50.0), probe(50.0)];
    let mut root: Element<'_, ()> = stack([probe(50.0), probe(50.0)]).into();

    let mut tree = widget::Tree::new(root.as_widget());
    tree.diff(root.as_widget_mut());

    let limits = iced::advanced::layout::Limits::new(Size::ZERO, Size::new(200.0, 200.0));
    let node = root.as_widget_mut().layout(&mut tree, &renderer, &limits);
    let layout = Layout::new(&node);

    let shape = Shape::branch(&root, children.iter().map(Shape::leaf).collect());

    let adapter = Adapter {
        tree: &tree,
        layout,
        shape: &shape,
    };

    let path = hit_test::hit_test(&adapter, Point::new(10.0, 10.0))
        .expect("the overlapping region must be hit");

    assert_eq!(
        path.indices(),
        &[1],
        "the later (topmost) sibling must win, matching iced's own back-to-front draw order"
    );
}

#[test]
fn a_point_outside_every_widget_is_not_hit() {
    let renderer = headless_renderer();

    let mut root: Element<'_, ()> = probe(50.0);

    let mut tree = widget::Tree::new(root.as_widget());
    tree.diff(root.as_widget_mut());

    let limits = iced::advanced::layout::Limits::new(Size::ZERO, Size::new(200.0, 200.0));
    let node = root.as_widget_mut().layout(&mut tree, &renderer, &limits);
    let layout = Layout::new(&node);

    let hit = root
        .as_widget()
        .hit_test(&tree, layout, Point::new(500.0, 500.0));

    assert!(!hit);

    let shape = Shape::leaf(&root);

    let adapter = Adapter {
        tree: &tree,
        layout,
        shape: &shape,
    };

    assert_eq!(hit_test::hit_test(&adapter, Point::new(500.0, 500.0)), None);
}

#[test]
fn a_translated_layout_hit_tests_at_its_new_position_not_the_old_one() {
    let renderer = headless_renderer();

    let mut root: Element<'_, ()> = probe(50.0);

    let mut tree = widget::Tree::new(root.as_widget());
    tree.diff(root.as_widget_mut());

    let limits = iced::advanced::layout::Limits::new(Size::ZERO, Size::new(200.0, 200.0));
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
    use iced::widget::{container, scrollable};

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

    let limits = iced::advanced::layout::Limits::new(Size::ZERO, Size::new(200.0, 200.0));
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
    use iced::widget::row;

    let renderer = headless_renderer();

    // Built twice for the same reason as above: `Element` is not
    // `Clone`, so the tree consumed by `row` and the tree kept for the
    // `Shape` mirror are two independent, equivalent constructions.
    let inner_left_a = probe(10.0);
    let inner_left_b = probe(10.0);
    let inner_left: Element<'_, ()> = row([probe(10.0), probe(10.0)]).into();
    let outer_right: Element<'_, ()> = probe(10.0);

    let mut root: Element<'_, ()> =
        row([row([probe(10.0), probe(10.0)]).into(), probe(10.0)]).into();

    let mut tree = widget::Tree::new(root.as_widget());
    tree.diff(root.as_widget_mut());

    let limits = iced::advanced::layout::Limits::new(Size::ZERO, Size::new(200.0, 200.0));
    let node = root.as_widget_mut().layout(&mut tree, &renderer, &limits);
    let layout = Layout::new(&node);

    // The first inner row's second probe sits at x offset 10.0 (past
    // the first 10.0-wide probe), well inside the outer row's first
    // slot.
    let point_in_nested_child = Point::new(15.0, 5.0);

    let shape = Shape::branch(
        &root,
        vec![
            Shape::branch(
                &inner_left,
                vec![Shape::leaf(&inner_left_a), Shape::leaf(&inner_left_b)],
            ),
            Shape::leaf(&outer_right),
        ],
    );

    let adapter = Adapter {
        tree: &tree,
        layout,
        shape: &shape,
    };

    let path = hit_test::hit_test(&adapter, point_in_nested_child)
        .expect("point should land inside the nested row's second probe");

    assert_eq!(path.indices(), &[0, 1]);
}

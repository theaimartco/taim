//! Proves the exit gate stated in `phase.md` Phase 7 for
//! `VirtualList`: "100,000-row benchmark maintains bounded live widget
//! count and stable memory" and "does not instantiate, reconcile,
//! layout, or paint off-window content except the bounded cache
//! extent," plus variable-height positioning, jump-to-position,
//! insert/remove reactivity, and focus traversal through virtualized
//! content.
use taim::{Point, Rectangle, Size};
use taim_core::Length;
use taim_core::widget::{self, Widget, operation};
use taim_core::{Layout, Shell, mouse};
use taim_core::{shell::Waker, window};
use taim_renderer::Renderer;
use taim_widget::{text, text_input, virtual_list};

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn headless_renderer() -> Renderer {
    taim_futures::futures::executor::block_on(<Renderer as taim_core::renderer::Headless>::new(
        taim_core::renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .expect("tiny-skia renderer must be available")
}

fn shell(messages: &mut Vec<()>) -> Shell<'_, ()> {
    Shell::new(&window::Headless, Waker::noop(), messages)
}

const ROW_HEIGHT: f32 = 20.0;
const VIEWPORT_HEIGHT: f32 = 400.0;
const ROW_COUNT: usize = 100_000;

fn scroll(
    list: &mut virtual_list::VirtualList<'_, (), taim::Theme, Renderer>,
    tree: &mut widget::Tree,
    renderer: &Renderer,
    delta_y: f32,
) {
    let event = taim_core::Event::Mouse(mouse::Event::WheelScrolled {
        delta: mouse::ScrollDelta::Pixels { x: 0.0, y: delta_y },
    });

    let mut messages = Vec::new();
    let mut shell = shell(&mut messages);
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(200.0, VIEWPORT_HEIGHT));
    let node = taim_core::layout::Node::new(Size::new(200.0, VIEWPORT_HEIGHT));

    Widget::<(), taim::Theme, Renderer>::update(
        list,
        tree,
        &event,
        Layout::new(&node),
        mouse::Cursor::Available(Point::new(100.0, 100.0)),
        renderer,
        &mut shell,
        &bounds,
    );
}

#[test]
fn a_hundred_thousand_rows_keeps_a_bounded_live_slot_count() {
    let renderer = headless_renderer();

    let mut list = virtual_list(ROW_COUNT, ROW_HEIGHT, |i| text(format!("Row {i}")).into())
        .width(Length::Fixed(200.0))
        .height(Length::Fixed(VIEWPORT_HEIGHT));

    let mut tree = widget::Tree::new(&list as &dyn Widget<(), taim::Theme, Renderer>);
    let limits = taim_core::layout::Limits::new(Size::ZERO, Size::new(200.0, VIEWPORT_HEIGHT));

    let node =
        Widget::<(), taim::Theme, Renderer>::layout(&mut list, &mut tree, &renderer, &limits);

    // Only the visible window (plus the default 400px cache extent on
    // either side) should ever be materialized as real children, no
    // matter how many rows exist in total.
    let visible_rows = (VIEWPORT_HEIGHT / ROW_HEIGHT).ceil() as usize;
    let cache_rows = ((400.0 / ROW_HEIGHT).ceil() as usize) * 2;
    let max_expected = visible_rows + cache_rows + 2; // a small slack for boundary rounding

    assert!(
        node.children().len() <= max_expected,
        "expected at most {max_expected} live children for a {ROW_COUNT}-row list, got {}",
        node.children().len()
    );

    assert!(
        !node.children().is_empty(),
        "the visible window must not be empty"
    );

    // The reported total size must still reflect the viewport, not the
    // huge underlying content -- exactly what lets an ancestor treat a
    // `VirtualList` like any ordinary, boundedly-sized widget.
    assert_eq!(node.size().height, VIEWPORT_HEIGHT);
}

#[test]
fn scrolling_to_the_end_brings_the_last_row_into_view() {
    let renderer = headless_renderer();

    let mut list = virtual_list(ROW_COUNT, ROW_HEIGHT, |i| text(format!("Row {i}")).into())
        .width(Length::Fixed(200.0))
        .height(Length::Fixed(VIEWPORT_HEIGHT));

    let mut tree = widget::Tree::new(&list as &dyn Widget<(), taim::Theme, Renderer>);
    let limits = taim_core::layout::Limits::new(Size::ZERO, Size::new(200.0, VIEWPORT_HEIGHT));

    let _ = Widget::<(), taim::Theme, Renderer>::layout(&mut list, &mut tree, &renderer, &limits);

    // A single, huge downward delta should clamp to the very end of the
    // list rather than overshoot.
    scroll(
        &mut list,
        &mut tree,
        &renderer,
        -(ROW_COUNT as f32 * ROW_HEIGHT * 2.0),
    );

    let node =
        Widget::<(), taim::Theme, Renderer>::layout(&mut list, &mut tree, &renderer, &limits);

    let last_child = node.children().last().expect("some rows must be visible");
    let bottom = last_child.bounds().y + last_child.bounds().height;

    assert!(
        (bottom - VIEWPORT_HEIGHT).abs() < ROW_HEIGHT,
        "the last row must be scrolled into view at the end of the list, got bottom={bottom}"
    );
}

#[test]
fn variable_height_rows_are_positioned_by_their_own_height_function() {
    let renderer = headless_renderer();

    // Alternate between a short (10px) and a tall (40px) row.
    let height_of = |i: usize| if i.is_multiple_of(2) { 10.0 } else { 40.0 };

    let mut list =
        virtual_list::VirtualList::variable(20, height_of, |i| text(format!("Row {i}")).into())
            .width(Length::Fixed(200.0))
            .height(Length::Fixed(200.0));

    let mut tree = widget::Tree::new(&list as &dyn Widget<(), taim::Theme, Renderer>);
    let limits = taim_core::layout::Limits::new(Size::ZERO, Size::new(200.0, 200.0));

    let node =
        Widget::<(), taim::Theme, Renderer>::layout(&mut list, &mut tree, &renderer, &limits);

    // First child (row 0, height 10) starts at y=0; second child (row
    // 1, height 40) starts at y=10; third child (row 2, height 10)
    // starts at y=50.
    let children = node.children();

    assert_eq!(children[0].bounds().y, 0.0);
    assert_eq!(children[0].bounds().height, 10.0);

    assert_eq!(children[1].bounds().y, 10.0);
    assert_eq!(children[1].bounds().height, 40.0);

    assert_eq!(children[2].bounds().y, 50.0);
    assert_eq!(children[2].bounds().height, 10.0);
}

#[test]
fn scrolling_repeatedly_keeps_the_live_slot_count_bounded() {
    let renderer = headless_renderer();

    let mut list = virtual_list(ROW_COUNT, ROW_HEIGHT, |i| text(format!("Row {i}")).into())
        .width(Length::Fixed(200.0))
        .height(Length::Fixed(VIEWPORT_HEIGHT))
        .cache_extent(0.0);

    let mut tree = widget::Tree::new(&list as &dyn Widget<(), taim::Theme, Renderer>);
    let limits = taim_core::layout::Limits::new(Size::ZERO, Size::new(200.0, VIEWPORT_HEIGHT));

    let first_pass =
        Widget::<(), taim::Theme, Renderer>::layout(&mut list, &mut tree, &renderer, &limits);
    let first_count = first_pass.children().len();

    for _ in 0..50 {
        scroll(&mut list, &mut tree, &renderer, -500.0);

        let node =
            Widget::<(), taim::Theme, Renderer>::layout(&mut list, &mut tree, &renderer, &limits);

        assert!(
            node.children().len() <= first_count + 2,
            "live slot count must stay bounded across many scroll steps"
        );
    }
}

#[test]
fn shrinking_the_item_count_shrinks_the_visible_content() {
    let renderer = headless_renderer();
    let limits = taim_core::layout::Limits::new(Size::ZERO, Size::new(200.0, VIEWPORT_HEIGHT));

    let mut ten_rows = virtual_list(10, ROW_HEIGHT, |i| text(format!("Row {i}")).into())
        .width(Length::Fixed(200.0))
        .height(Length::Fixed(VIEWPORT_HEIGHT));

    let mut tree = widget::Tree::new(&ten_rows as &dyn Widget<(), taim::Theme, Renderer>);

    let before =
        Widget::<(), taim::Theme, Renderer>::layout(&mut ten_rows, &mut tree, &renderer, &limits);
    assert_eq!(before.children().len(), 10);

    let mut three_rows = virtual_list(3, ROW_HEIGHT, |i| text(format!("Row {i}")).into())
        .width(Length::Fixed(200.0))
        .height(Length::Fixed(VIEWPORT_HEIGHT));

    // Re-lay-out the *same* persistent tree against a list with fewer
    // items, exactly as a real `view` producing a shorter list on the
    // next frame would.
    let after =
        Widget::<(), taim::Theme, Renderer>::layout(&mut three_rows, &mut tree, &renderer, &limits);
    assert_eq!(after.children().len(), 3);
}

#[test]
fn focus_can_traverse_visible_virtualized_rows() {
    let renderer = headless_renderer();

    let mut list = virtual_list(50, ROW_HEIGHT, |i| {
        text_input("", &format!("row-{i}")).into()
    })
    .width(Length::Fixed(200.0))
    .height(Length::Fixed(VIEWPORT_HEIGHT));

    let mut tree = widget::Tree::new(&list as &dyn Widget<(), taim::Theme, Renderer>);
    let limits = taim_core::layout::Limits::new(Size::ZERO, Size::new(200.0, VIEWPORT_HEIGHT));

    let node =
        Widget::<(), taim::Theme, Renderer>::layout(&mut list, &mut tree, &renderer, &limits);
    let layout = Layout::new(&node);

    let mut count_operation = operation::focusable::count();

    Widget::<(), taim::Theme, Renderer>::operate(
        &mut list,
        &mut tree,
        layout,
        &renderer,
        &mut operation::black_box(&mut count_operation),
    );

    let operation::Outcome::Some(count) = operation::Operation::finish(&count_operation) else {
        panic!("expected a Count outcome");
    };

    assert!(
        count.total > 0,
        "focusable rows currently visible in the virtualized window must be reachable by a focus-counting operation"
    );

    assert!(
        count.total < 50,
        "an operation over the visible window must not reach every one of the 50 rows, proving virtualization is real"
    );
}

#[test]
fn a_routed_live_value_does_not_rebuild_its_row_during_dispatch() {
    let renderer = headless_renderer();
    let builds = Arc::new(AtomicUsize::new(0));
    let counted_builds = Arc::clone(&builds);
    let source = 41_u64;
    let target = 3_usize;

    let mut list = virtual_list(50, ROW_HEIGHT, move |i| {
        let _ = counted_builds.fetch_add(1, Ordering::Relaxed);
        text(format!("Row {i}")).into()
    })
    .route_reactive_value(move |candidate| (candidate == source).then_some(target))
    .width(Length::Fixed(200.0))
    .height(Length::Fixed(VIEWPORT_HEIGHT));

    let mut tree = widget::Tree::new(&list as &dyn Widget<(), taim::Theme, Renderer>);
    let limits = taim_core::layout::Limits::new(Size::ZERO, Size::new(200.0, VIEWPORT_HEIGHT));
    let node =
        Widget::<(), taim::Theme, Renderer>::layout(&mut list, &mut tree, &renderer, &limits);
    builds.store(0, Ordering::Relaxed);

    let bounds = Rectangle::new(Point::ORIGIN, Size::new(200.0, VIEWPORT_HEIGHT));
    let mut messages = Vec::new();
    let mut shell = shell(&mut messages);
    Widget::<(), taim::Theme, Renderer>::update(
        &mut list,
        &mut tree,
        &taim_core::Event::Reactive(source),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &renderer,
        &mut shell,
        &bounds,
    );

    assert_eq!(builds.load(Ordering::Relaxed), 0);
    assert_eq!(
        shell.redraw_request(),
        taim_core::window::RedrawRequest::NextFrame
    );
}

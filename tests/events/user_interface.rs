//! `UserInterface::hit_test` / `hit_test_index` — Phase 3's hit-testing,
//! reached the way an application reaches it.
//!
//! `hit_test::Hoverable` used to require `fn child(&self, i) -> (Self,
//! Layout)`, a shape only a tree with indexable children could satisfy.
//! No `Widget` has those, so **every implementor in the workspace was a
//! test type**, and the two test files that claimed to cover "real widget
//! trees" walked a hand-built `Shape` mirroring the tree rather than the
//! tree.
//!
//! `UserInterface::route_pointer` accordingly documented that its caller
//! supplies a `hit_test::Path` "already found via
//! `hit_test::Index::hit_test`" — which nothing could produce. Gesture
//! routing was wired to nothing.
//!
//! These tests call the public methods an application calls.

use aura::widget::{column, container, row, space, stack};
use aura::{Element, Point, Size, Theme};
use aura_renderer::Renderer;
use aura_runtime::user_interface::{Cache, UserInterface};

type Message = &'static str;

const WINDOW: Size = Size {
    width: 400.0,
    height: 400.0,
};

fn headless_renderer() -> Renderer {
    aura_futures::futures::executor::block_on(
        <Renderer as aura::advanced::renderer::Headless>::new(
            aura::advanced::renderer::Settings::default(),
            Some("tiny-skia"),
        ),
    )
    .expect("tiny-skia renderer must be available")
}

fn ui(view: Element<'_, Message, Theme, Renderer>) -> UserInterface<'_, Message, Theme, Renderer> {
    let mut renderer = headless_renderer();

    UserInterface::build(view, WINDOW, Cache::new(), &mut renderer)
}

/// A leaf: `space` has no children, so a path to it is unambiguous.
///
/// (A `button` would not do here — its label is a child widget, so the
/// deepest hit inside one is the *text*, and every path would carry a
/// trailing index for it.)
fn sized(width: f32, height: f32) -> Element<'static, Message, Theme, Renderer> {
    space().width(width).height(height).into()
}

#[test]
fn a_real_application_can_finally_hit_test_its_own_widgets() {
    // The regression this exists to prevent: before
    // `UserInterface::hit_test`, there was no method to call, and no way
    // for any caller to build a `Hoverable` over a real tree.
    let mut ui = ui(row![sized(50.0, 50.0), sized(50.0, 50.0)].into());

    assert_eq!(
        ui.hit_test(Point::new(10.0, 10.0))
            .expect("the first leaf must be hit")
            .indices(),
        &[0],
        "a point inside the first child must resolve to it"
    );

    assert_eq!(
        ui.hit_test(Point::new(60.0, 10.0))
            .expect("the second leaf must be hit")
            .indices(),
        &[1]
    );
}

#[test]
fn a_point_outside_everything_hits_nothing() {
    let mut ui = ui(sized(50.0, 50.0));

    assert_eq!(ui.hit_test(Point::new(10_000.0, 10_000.0)), None);
}

#[test]
fn overlapping_siblings_resolve_to_the_topmost() {
    // The convention `Widget::overlay` and event dispatch already
    // follow: the one drawn last is on top, so it wins.
    let mut ui = ui(stack![sized(50.0, 50.0), sized(50.0, 50.0)].into());

    assert_eq!(
        ui.hit_test(Point::new(10.0, 10.0))
            .expect("the overlapping region must be hit")
            .indices(),
        &[1],
        "the later sibling is drawn on top and must win"
    );
}

#[test]
fn a_nested_child_returns_its_full_path() {
    let mut ui = ui(column![row![sized(20.0, 20.0), sized(20.0, 20.0)]].into());

    let path = ui
        .hit_test(Point::new(30.0, 10.0))
        .expect("the nested second leaf must be hit");

    assert_eq!(
        path.indices(),
        &[0, 1],
        "the path must reach the nested child, not stop at its parent"
    );
}

#[test]
fn the_index_agrees_with_the_oracle_across_a_real_view() {
    // `phase.md`'s exit gate ("spatial index always agrees with
    // exhaustive hit testing"), now over a tree an application would
    // actually build rather than a hand-mirrored shape.
    let mut ui = ui(container(column![
        row![sized(30.0, 30.0), sized(30.0, 30.0)],
        stack![sized(40.0, 40.0), sized(20.0, 20.0)],
    ])
    .into());

    let index = ui.hit_test_index();

    let mut x = -5.0;
    while x <= 120.0 {
        let mut y = -5.0;
        while y <= 120.0 {
            let point = Point::new(x, y);

            assert_eq!(
                index.hit_test(&mut ui.visit(), point),
                ui.hit_test(point),
                "the index disagreed with the oracle at {point:?}"
            );

            y += 3.0;
        }
        x += 3.0;
    }
}

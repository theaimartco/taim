//! The `app_bar` widget: slot layout, collapse, and container semantics.
//!
//! Realises `aura_design::components::app_bar::AppBar`. That contract was
//! *corrected* during Phase 11 — it originally claimed `ACTIVATE` with no
//! focusable target behind it — so the most important thing here is that
//! the widget is a container: no actions, no focus, affordances delegated
//! to real controls in its slots.

use aura::advanced::widget::{self, Widget};
use aura::advanced::{Layout, layout};
use aura::widget::{app_bar, icon_button, space, text};
use aura::{Element, Rectangle, Size, Theme};
use aura_core::semantics::Role;
use aura_renderer::Renderer;
use aura_widget::app_bar as app_bar_mod;

type Message = &'static str;

const WIDTH: f32 = 400.0;

fn headless_renderer() -> Renderer {
    aura_futures::futures::executor::block_on(
        <Renderer as aura::advanced::renderer::Headless>::new(
            aura::advanced::renderer::Settings::default(),
            Some("tiny-skia"),
        ),
    )
    .expect("tiny-skia renderer must be available")
}

struct Harness<'a> {
    widget: app_bar_mod::AppBar<'a, Message, Theme, Renderer>,
    tree: widget::Tree,
    node: layout::Node,
}

impl<'a> Harness<'a> {
    fn new(widget: app_bar_mod::AppBar<'a, Message, Theme, Renderer>) -> Self {
        let mut widget = widget;
        let renderer = headless_renderer();

        let mut tree = widget::Tree::new(&widget as &dyn Widget<Message, Theme, Renderer>);
        tree.diff(&mut widget as &mut dyn Widget<Message, Theme, Renderer>);

        let node = Widget::<Message, Theme, Renderer>::layout(
            &mut widget,
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, Size::new(WIDTH, 400.0)),
        );

        Self { widget, tree, node }
    }

    fn bounds(&self) -> Rectangle {
        Layout::new(&self.node).bounds()
    }

    fn slot_bounds(&self) -> Vec<Rectangle> {
        Layout::new(&self.node)
            .children()
            .map(|child| child.bounds())
            .collect()
    }

    fn semantics(&self) -> Option<aura_core::semantics::Node> {
        Widget::<Message, Theme, Renderer>::semantics(
            &self.widget,
            &self.tree,
            Layout::new(&self.node),
        )
    }
}

fn title() -> Element<'static, Message, Theme, Renderer> {
    text("Inbox").into()
}

// ---------------------------------------------------------------------
// The container contract
// ---------------------------------------------------------------------

#[test]
fn an_app_bar_is_a_container_with_no_actions_and_no_focus() {
    // The corrected contract: a bar that claimed ACTIVATE would be
    // advertising an action with no focusable, touch-sized target.
    let harness = Harness::new(app_bar(title()).leading(icon_button('<', "Back").on_press("back")));

    let node = harness.semantics().expect("must report semantics");

    assert_eq!(node.role, Role::Group);
    assert!(
        node.actions.is_empty(),
        "a container must claim no actions of its own"
    );
    assert!(node.state.is_empty());
}

// ---------------------------------------------------------------------
// Height
// ---------------------------------------------------------------------

#[test]
fn each_kind_reserves_the_height_the_contract_declares() {
    use aura_design::components::app_bar::AppBarKind;

    for (kind, contract) in [
        (app_bar_mod::Kind::Standard, AppBarKind::Standard),
        (app_bar_mod::Kind::Large, AppBarKind::Large),
        (app_bar_mod::Kind::Collapsing, AppBarKind::Collapsing),
    ] {
        assert_eq!(
            kind.expanded_height(),
            contract.expanded_height(),
            "{kind:?}: widget and contract must agree on expanded height"
        );
        assert_eq!(
            kind.collapsed_height(),
            contract.collapsed_height(),
            "{kind:?}: widget and contract must agree on collapsed height"
        );
    }
}

#[test]
fn a_standard_bar_is_a_single_row() {
    let harness = Harness::new(app_bar(title()));
    assert_eq!(harness.bounds().height, 64.0);
}

#[test]
fn a_large_bar_reserves_room_for_a_display_title() {
    let harness = Harness::new(app_bar(title()).kind(app_bar_mod::Kind::Large));
    assert_eq!(harness.bounds().height, 112.0);
}

#[test]
fn only_a_collapsing_bar_shrinks() {
    let collapsing = Harness::new(
        app_bar(title())
            .kind(app_bar_mod::Kind::Collapsing)
            .collapse(1.0),
    );
    assert_eq!(collapsing.bounds().height, 64.0);

    // A Large bar keeps its height however far the content scrolls.
    let large = Harness::new(
        app_bar(title())
            .kind(app_bar_mod::Kind::Large)
            .collapse(1.0),
    );
    assert_eq!(large.bounds().height, 112.0);

    let standard = Harness::new(app_bar(title()).collapse(1.0));
    assert_eq!(standard.bounds().height, 64.0);
}

#[test]
fn collapse_interpolates_and_clamps() {
    let half = Harness::new(
        app_bar(title())
            .kind(app_bar_mod::Kind::Collapsing)
            .collapse(0.5),
    );
    assert_eq!(half.bounds().height, 88.0, "halfway between 112 and 64");

    // Out-of-range progress clamps rather than inverting the bar.
    let over = Harness::new(
        app_bar(title())
            .kind(app_bar_mod::Kind::Collapsing)
            .collapse(5.0),
    );
    assert_eq!(over.bounds().height, 64.0);

    let under = Harness::new(
        app_bar(title())
            .kind(app_bar_mod::Kind::Collapsing)
            .collapse(-5.0),
    );
    assert_eq!(under.bounds().height, 112.0);
}

// ---------------------------------------------------------------------
// Slot layout
// ---------------------------------------------------------------------

#[test]
fn actions_are_packed_against_the_right_edge() {
    let harness = Harness::new(
        app_bar(title())
            .leading(space().width(40.0).height(40.0))
            .action(space().width(40.0).height(40.0))
            .action(space().width(40.0).height(40.0)),
    );

    let slots = harness.slot_bounds();
    assert_eq!(slots.len(), 4, "leading, title, and two actions");

    let last = slots[3];
    let second_last = slots[2];

    assert!(
        last.x > second_last.x,
        "actions must be laid out left to right in the order they were added"
    );
    assert!(
        last.x + last.width <= WIDTH,
        "the outermost action must stay inside the bar"
    );
}

#[test]
fn actions_do_not_move_when_the_title_grows() {
    // What keeps a bar's controls from shuffling as you navigate between
    // screens with different titles.
    let short = Harness::new(app_bar(text("a")).action(space().width(40.0).height(40.0)));
    let long = Harness::new(
        app_bar(text("a considerably longer screen title"))
            .action(space().width(40.0).height(40.0)),
    );

    let short_action = *short.slot_bounds().last().unwrap();
    let long_action = *long.slot_bounds().last().unwrap();

    assert_eq!(
        short_action.x, long_action.x,
        "an action's position must not depend on the title's width"
    );
}

#[test]
fn the_leading_slot_precedes_the_title() {
    let harness = Harness::new(app_bar(title()).leading(space().width(40.0).height(40.0)));

    let slots = harness.slot_bounds();
    assert_eq!(slots.len(), 2);
    assert!(slots[0].x < slots[1].x);
}

#[test]
fn a_bare_bar_has_only_its_title() {
    let harness = Harness::new(app_bar(title()));
    assert_eq!(harness.slot_bounds().len(), 1);
}

#[test]
fn a_large_titles_row_rises_as_the_bar_collapses() {
    // The whole point of a collapsing bar: the title travels from its own
    // row up to the action row.
    let expanded = Harness::new(
        app_bar(title())
            .kind(app_bar_mod::Kind::Collapsing)
            .collapse(0.0),
    );
    let collapsed = Harness::new(
        app_bar(title())
            .kind(app_bar_mod::Kind::Collapsing)
            .collapse(1.0),
    );

    let expanded_title = expanded.slot_bounds()[0];
    let collapsed_title = collapsed.slot_bounds()[0];

    assert!(
        collapsed_title.y < expanded_title.y,
        "the title must rise as the bar collapses: {} -> {}",
        expanded_title.y,
        collapsed_title.y
    );
}

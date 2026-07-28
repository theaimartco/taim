//! The `list_tile` widget: slot layout, behavior, and semantics.
//!
//! Realises `aura_design::components::list_tile::ListTile`. The contract
//! says a tile's height comes from its line count and that a
//! non-interactive tile is presentation; this proves the widget does both.

use aura::advanced::widget::{self, Widget};
use aura::advanced::{Layout, Shell, layout, mouse};
use aura::widget::{list_tile, space, text};
use aura::{Element, Event, Point, Rectangle, Size, Theme};
use aura_core::semantics::{Actions, Role, State};
use aura_core::{shell::Waker, window};
use aura_renderer::Renderer;
use aura_widget::list_tile as list_tile_mod;

type Message = &'static str;

const WIDTH: f32 = 320.0;

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
    widget: list_tile_mod::ListTile<'a, Message, Theme, Renderer>,
    tree: widget::Tree,
    node: layout::Node,
    renderer: Renderer,
}

impl<'a> Harness<'a> {
    fn new(widget: list_tile_mod::ListTile<'a, Message, Theme, Renderer>) -> Self {
        let mut widget = widget;
        let renderer = headless_renderer();

        let mut tree = widget::Tree::new(&widget as &dyn Widget<Message, Theme, Renderer>);
        tree.diff(&mut widget as &mut dyn Widget<Message, Theme, Renderer>);

        let node = Widget::<Message, Theme, Renderer>::layout(
            &mut widget,
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, Size::new(WIDTH, 200.0)),
        );

        Self {
            widget,
            tree,
            node,
            renderer,
        }
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

    fn dispatch(&mut self, event: Event, cursor: Point) -> Vec<Message> {
        let mut messages = Vec::new();
        let mut shell = Shell::new(&window::Headless, Waker::noop(), &mut messages);

        Widget::<Message, Theme, Renderer>::update(
            &mut self.widget,
            &mut self.tree,
            &event,
            Layout::new(&self.node),
            mouse::Cursor::Available(cursor),
            &self.renderer,
            &mut shell,
            &Rectangle::new(Point::ORIGIN, Size::new(WIDTH, 200.0)),
        );

        messages
    }

    fn tap(&mut self, at: Point) -> Vec<Message> {
        let _ = self.dispatch(
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            at,
        );
        self.dispatch(
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            at,
        )
    }

    fn semantics(&self) -> Option<aura_core::semantics::Node> {
        Widget::<Message, Theme, Renderer>::semantics(
            &self.widget,
            &self.tree,
            Layout::new(&self.node),
        )
    }
}

fn body() -> Element<'static, Message, Theme, Renderer> {
    text("Inbox").into()
}

// ---------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------

#[test]
fn a_tiles_height_comes_from_its_line_count_not_its_content() {
    // The rhythm guarantee: rows share a height regardless of what they
    // hold, which is what makes a list scannable and a virtualized list
    // able to compute positions without measuring.
    for (lines, expected) in [
        (list_tile_mod::Lines::One, 48.0),
        (list_tile_mod::Lines::Two, 64.0),
        (list_tile_mod::Lines::Three, 80.0),
    ] {
        let harness = Harness::new(list_tile(body()).lines(lines));

        assert_eq!(
            harness.bounds().height,
            expected,
            "{lines:?} must reserve {expected}px"
        );
    }
}

#[test]
fn the_declared_heights_match_the_aura_design_contract() {
    // Two sources, one set of numbers: a drift here would mean the
    // contract and the widget disagree about what a list looks like.
    use aura_design::components::list_tile::ListLines;

    assert_eq!(list_tile_mod::Lines::One.height(), ListLines::One.height());
    assert_eq!(list_tile_mod::Lines::Two.height(), ListLines::Two.height());
    assert_eq!(
        list_tile_mod::Lines::Three.height(),
        ListLines::Three.height()
    );
}

#[test]
fn a_bare_tile_has_exactly_one_slot() {
    let harness = Harness::new(list_tile(body()));
    assert_eq!(harness.slot_bounds().len(), 1);
}

#[test]
fn leading_and_trailing_slots_are_laid_out_around_the_content() {
    let harness = Harness::new(
        list_tile(body())
            .leading(space().width(24.0).height(24.0))
            .trailing(space().width(16.0).height(16.0)),
    );

    let slots = harness.slot_bounds();
    assert_eq!(slots.len(), 3, "leading, content, trailing");

    let (leading, content, trailing) = (slots[0], slots[1], slots[2]);

    assert!(
        leading.x < content.x,
        "the leading slot must precede the content"
    );
    assert!(
        content.x + content.width <= trailing.x + f32::EPSILON,
        "the content must not overlap the trailing slot"
    );
}

#[test]
fn the_trailing_slot_is_anchored_to_the_right_edge() {
    // What keeps a column of chevrons aligned down a list, rather than
    // each one sitting wherever its row's text happened to end.
    let short = Harness::new(list_tile(text("a")).trailing(space().width(16.0).height(16.0)));
    let long = Harness::new(
        list_tile(text("a much longer label indeed")).trailing(space().width(16.0).height(16.0)),
    );

    let short_trailing = *short.slot_bounds().last().unwrap();
    let long_trailing = *long.slot_bounds().last().unwrap();

    assert_eq!(
        short_trailing.x, long_trailing.x,
        "a trailing slot must not move with the content's width"
    );
    assert!(
        short_trailing.x + short_trailing.width <= WIDTH,
        "the trailing slot must stay inside the row"
    );
}

#[test]
fn slots_are_vertically_centred_in_the_row() {
    let harness = Harness::new(
        list_tile(body())
            .lines(list_tile_mod::Lines::Three)
            .leading(space().width(24.0).height(24.0)),
    );

    let row = harness.bounds();
    let leading = harness.slot_bounds()[0];

    let leading_centre = leading.y + leading.height / 2.0;
    let row_centre = row.y + row.height / 2.0;

    assert!(
        (leading_centre - row_centre).abs() < 0.5,
        "a slot must sit on the row's optical centre: {leading_centre} vs {row_centre}"
    );
}

// ---------------------------------------------------------------------
// Behavior
// ---------------------------------------------------------------------

#[test]
fn an_interactive_tile_activates_on_tap() {
    let mut harness = Harness::new(list_tile(body()).on_press("opened"));

    assert_eq!(harness.tap(Point::new(10.0, 10.0)), vec!["opened"]);
}

#[test]
fn a_presentation_tile_never_activates() {
    let mut harness: Harness<'_> = Harness::new(list_tile(body()));

    assert!(
        harness.tap(Point::new(10.0, 10.0)).is_empty(),
        "a tile with no on_press must be inert"
    );
}

// ---------------------------------------------------------------------
// Semantics
// ---------------------------------------------------------------------

#[test]
fn an_interactive_tile_reports_as_a_list_item() {
    let harness = Harness::new(list_tile(body()).on_press("opened"));
    let node = harness.semantics().expect("must report semantics");

    assert_eq!(node.role, Role::ListItem);
    assert!(node.actions.contains(Actions::ACTIVATE));
    assert!(node.actions.contains(Actions::FOCUS));
    assert!(node.state.contains(State::FOCUSABLE));
}

#[test]
fn a_presentation_tile_reports_as_static_text_and_takes_no_focus() {
    // Claiming ListItem for a row nobody can act on would put it in a
    // screen reader's list navigation for no reason.
    let harness: Harness<'_> = Harness::new(list_tile(body()));
    let node = harness.semantics().unwrap();

    assert_eq!(node.role, Role::StaticText);
    assert!(node.actions.is_empty());
    assert!(!node.state.contains(State::FOCUSABLE));
}

#[test]
fn a_selected_tile_reports_its_selection() {
    let harness = Harness::new(list_tile(body()).on_press("opened").selected(true));
    let node = harness.semantics().unwrap();

    assert!(node.state.contains(State::SELECTED));
}

#[test]
fn an_interactive_tile_meets_the_minimum_touch_target() {
    use aura_design::components::MIN_TOUCH_TARGET;

    let harness = Harness::new(list_tile(body()).on_press("opened"));

    assert!(
        harness.bounds().height >= MIN_TOUCH_TARGET,
        "even a one-line tile must be tappable: {}",
        harness.bounds().height
    );
}

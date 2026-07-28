//! The `icon_button` widget: behavior, semantics, and Aura theming.
//!
//! `aura_design::components::icon_button::IconButton` states the contract
//! ("an icon-only control's accessible name is mandatory"). This proves
//! the real widget honors it, and behaves the way a button must.

use aura::advanced::widget::{self, Widget};
use aura::advanced::{Layout, Shell, layout, mouse};
use aura::widget::icon_button;
use aura::{Event, Point, Rectangle, Size, Theme};
use aura_core::semantics::{Actions, Role, State};
use aura_core::{shell::Waker, window};
use aura_renderer::Renderer;
use aura_widget::icon_button as icon_button_mod;

type Message = &'static str;

const BOUNDS: Size = Size {
    width: 60.0,
    height: 60.0,
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

/// A laid-out icon button, ready to receive events.
struct Harness<'a> {
    widget: icon_button_mod::IconButton<'a, Message, Theme, Renderer>,
    tree: widget::Tree,
    node: layout::Node,
    renderer: Renderer,
}

impl<'a> Harness<'a> {
    fn new(widget: icon_button_mod::IconButton<'a, Message, Theme, Renderer>) -> Self {
        let mut widget = widget;
        let renderer = headless_renderer();

        let mut tree = widget::Tree::new(&widget as &dyn Widget<Message, Theme, Renderer>);
        tree.diff(&mut widget as &mut dyn Widget<Message, Theme, Renderer>);

        let node = Widget::<Message, Theme, Renderer>::layout(
            &mut widget,
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, BOUNDS),
        );

        Self {
            widget,
            tree,
            node,
            renderer,
        }
    }

    /// Dispatches an event with the cursor at `cursor`.
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
            &Rectangle::new(Point::ORIGIN, BOUNDS),
        );

        messages
    }

    fn press(&mut self, at: Point) -> Vec<Message> {
        self.dispatch(
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            at,
        )
    }

    fn release(&mut self, at: Point) -> Vec<Message> {
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

fn inside() -> Point {
    Point::new(10.0, 10.0)
}

fn outside() -> Point {
    Point::new(500.0, 500.0)
}

// ---------------------------------------------------------------------
// Behavior
// ---------------------------------------------------------------------

#[test]
fn a_press_and_release_over_the_button_activates_it() {
    let mut harness = Harness::new(icon_button('x', "Close").on_press("closed"));

    assert!(
        harness.press(inside()).is_empty(),
        "press alone does nothing"
    );
    assert_eq!(harness.release(inside()), vec!["closed"]);
}

#[test]
fn releasing_away_from_the_button_cancels_the_press() {
    // Dragging off a button is how a user takes back a press they thought
    // better of. Activating anyway would be a trap.
    let mut harness = Harness::new(icon_button('x', "Close").on_press("closed"));

    let _ = harness.press(inside());
    assert!(
        harness.release(outside()).is_empty(),
        "a release outside the control must not activate it"
    );
}

#[test]
fn a_release_with_no_prior_press_does_nothing() {
    // The press may have begun in another widget entirely.
    let mut harness = Harness::new(icon_button('x', "Close").on_press("closed"));

    assert!(harness.release(inside()).is_empty());
}

#[test]
fn a_disabled_button_never_activates() {
    let mut harness: Harness<'_> = Harness::new(icon_button('x', "Close"));

    let _ = harness.press(inside());
    assert!(
        harness.release(inside()).is_empty(),
        "a button with no on_press must be inert"
    );
}

#[test]
fn a_cancelled_touch_does_not_activate() {
    // FingerLost is the platform taking the contact away (a call arrived,
    // a system gesture won). It must not count as a tap.
    let mut harness = Harness::new(icon_button('x', "Close").on_press("closed"));

    let _ = harness.dispatch(
        Event::Touch(aura_core::touch::Event::FingerPressed {
            id: aura_core::touch::Finger(0),
            position: inside(),
            metadata: aura_core::pointer::Metadata::NONE,
        }),
        inside(),
    );

    let _ = harness.dispatch(
        Event::Touch(aura_core::touch::Event::FingerLost {
            id: aura_core::touch::Finger(0),
            position: inside(),
            metadata: aura_core::pointer::Metadata::NONE,
        }),
        inside(),
    );

    assert!(
        harness.release(inside()).is_empty(),
        "a lost contact must not activate the control"
    );
}

// ---------------------------------------------------------------------
// The accessible-name contract
// ---------------------------------------------------------------------

#[test]
fn an_icon_button_always_reports_its_accessible_name() {
    // The contract this widget exists to make unbreakable: the label is a
    // required constructor argument, so there is no way to build one
    // without a name.
    let harness = Harness::new(icon_button('x', "Close").on_press("closed"));

    let node = harness.semantics().expect("must report semantics");

    assert_eq!(node.role, Role::Button);
    assert_eq!(
        node.name.as_deref(),
        Some("Close"),
        "an icon-only control's name is the only thing a screen reader can announce"
    );
    assert!(node.actions.contains(Actions::ACTIVATE));
    assert!(node.actions.contains(Actions::FOCUS));
    assert!(node.state.contains(State::FOCUSABLE));
}

#[test]
fn a_disabled_icon_button_keeps_its_name_but_offers_nothing() {
    let harness: Harness<'_> = Harness::new(icon_button('x', "Close"));

    let node = harness.semantics().unwrap();

    assert!(node.state.contains(State::DISABLED));
    assert!(node.actions.is_empty());
    assert_eq!(
        node.name.as_deref(),
        Some("Close"),
        "a disabled control must still say what it is"
    );
}

// ---------------------------------------------------------------------
// Touch target
// ---------------------------------------------------------------------

#[test]
fn an_icon_button_meets_the_minimum_touch_target() {
    use aura_design::components::MIN_TOUCH_TARGET;

    let harness = Harness::new(icon_button('x', "Close").on_press("closed"));
    let bounds = Layout::new(&harness.node).bounds();

    assert!(bounds.width >= MIN_TOUCH_TARGET);
    assert!(bounds.height >= MIN_TOUCH_TARGET);
}

#[test]
fn an_undersized_icon_button_is_clamped_up_to_the_touch_target() {
    use aura_design::components::MIN_TOUCH_TARGET;

    // A caller asking for a 12px button is asking for something nobody can
    // reliably hit. The control refuses rather than obeying.
    let harness = Harness::new(icon_button('x', "Close").size(12.0).on_press("closed"));
    let bounds = Layout::new(&harness.node).bounds();

    assert!(
        bounds.height >= MIN_TOUCH_TARGET,
        "an undersized icon button must be clamped up, not honored: {}",
        bounds.height
    );
}

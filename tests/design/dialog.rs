//! The `dialog` widget: dismissal, inertness, semantics, and the Phase 13
//! focus trap end-to-end.
//!
//! Realises `taim_design::components::dialog::Dialog`. The contract says a
//! modal that does not trap focus is a bug, not a variant; this proves the
//! real widget reports what a real `taim_accessibility::Focus` needs to
//! enforce that, and that the two compose.

use taim_accessibility::focus::{Focus, Scope};
use taim_accessibility::tree::Tree as SemanticTree;

use taim::{Point, Rectangle, Size, Theme};
use taim_core::Event;
use taim_core::keyboard;
use taim_core::semantics::{Actions, Role};
use taim_core::widget::{self, Widget};
use taim_core::{Layout, Shell, layout, mouse};
use taim_core::{shell::Waker, window};
use taim_renderer::Renderer;
use taim_widget::Element;
use taim_widget::dialog as dialog_mod;
use taim_widget::{button, dialog, text};

type Message = &'static str;

const WINDOW: Size = Size {
    width: 800.0,
    height: 600.0,
};

fn headless_renderer() -> Renderer {
    taim_futures::futures::executor::block_on(<Renderer as taim_core::renderer::Headless>::new(
        taim_core::renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .expect("tiny-skia renderer must be available")
}

struct Harness<'a> {
    widget: dialog_mod::Dialog<'a, Message, Theme, Renderer>,
    tree: widget::Tree,
    node: layout::Node,
    renderer: Renderer,
}

impl<'a> Harness<'a> {
    fn new(widget: dialog_mod::Dialog<'a, Message, Theme, Renderer>) -> Self {
        let mut widget = widget;
        let renderer = headless_renderer();

        let mut tree = widget::Tree::new(&widget as &dyn Widget<Message, Theme, Renderer>);
        tree.diff(&mut widget as &mut dyn Widget<Message, Theme, Renderer>);

        let node = Widget::<Message, Theme, Renderer>::layout(
            &mut widget,
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, WINDOW),
        );

        Self {
            widget,
            tree,
            node,
            renderer,
        }
    }

    /// The scrim's bounds: the whole area the dialog covers.
    fn scrim(&self) -> Rectangle {
        Layout::new(&self.node).bounds()
    }

    /// The dialog surface's bounds.
    fn surface(&self) -> Rectangle {
        Layout::new(&self.node)
            .children()
            .next()
            .expect("a dialog always lays out a surface")
            .bounds()
    }

    fn dispatch(&mut self, event: Event, cursor: Point) -> (Vec<Message>, bool) {
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
            &Rectangle::new(Point::ORIGIN, WINDOW),
        );

        let captured = shell.is_event_captured();

        (messages, captured)
    }

    fn press(&mut self, at: Point) -> (Vec<Message>, bool) {
        self.dispatch(
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            at,
        )
    }

    fn escape(&mut self) -> (Vec<Message>, bool) {
        self.dispatch(
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                modified_key: keyboard::Key::Named(keyboard::key::Named::Escape),
                physical_key: keyboard::key::Physical::Unidentified(
                    keyboard::key::NativeCode::Unidentified,
                ),
                location: keyboard::Location::Standard,
                modifiers: keyboard::Modifiers::empty(),
                text: None,
                repeat: false,
            }),
            Point::new(10.0, 10.0),
        )
    }

    fn semantics(&self) -> Option<taim_core::semantics::Node> {
        Widget::<Message, Theme, Renderer>::semantics(
            &self.widget,
            &self.tree,
            Layout::new(&self.node),
        )
    }
}

fn body() -> Element<'static, Message, Theme, Renderer> {
    text("Are you sure?").into()
}

/// A point on the scrim, well away from the centred surface.
fn on_scrim() -> Point {
    Point::new(5.0, 5.0)
}

fn on_surface(harness: &Harness<'_>) -> Point {
    harness.surface().center()
}

// ---------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------

#[test]
fn a_dialog_covers_everything_it_is_stacked_over() {
    let harness = Harness::new(dialog(body()));

    assert_eq!(
        harness.scrim(),
        Rectangle::new(Point::ORIGIN, WINDOW),
        "the scrim must fill the space, or the page behind stays reachable"
    );
}

#[test]
fn the_surface_is_centred_and_never_larger_than_the_window() {
    let harness = Harness::new(dialog(body()));

    let scrim = harness.scrim();
    let surface = harness.surface();

    let surface_centre = surface.center();
    let scrim_centre = scrim.center();

    assert!((surface_centre.x - scrim_centre.x).abs() < 0.5);
    assert!((surface_centre.y - scrim_centre.y).abs() < 0.5);

    assert!(surface.width <= scrim.width);
    assert!(
        surface.height <= scrim.height,
        "a dialog taller than the window would put its own actions off screen"
    );
}

#[test]
fn the_surface_respects_its_minimum_size() {
    // From taim_design::components::dialog: 280x120.
    let harness = Harness::new(dialog(text("ok")));
    let surface = harness.surface();

    assert!(surface.width >= 280.0, "got {}", surface.width);
    assert!(surface.height >= 120.0, "got {}", surface.height);
}

#[test]
fn the_surface_respects_its_max_width() {
    let harness = Harness::new(dialog(body()).max_width(320.0));

    assert!(harness.surface().width <= 320.0);
}

// ---------------------------------------------------------------------
// Dismissal
// ---------------------------------------------------------------------

#[test]
fn escape_dismisses_a_dismissible_dialog() {
    let mut harness = Harness::new(dialog(body()).on_dismiss("dismissed"));

    let (messages, captured) = harness.escape();

    assert_eq!(messages, vec!["dismissed"]);
    assert!(captured, "Escape must not also reach the page behind");
}

#[test]
fn tapping_the_scrim_dismisses_by_default() {
    let mut harness = Harness::new(dialog(body()).on_dismiss("dismissed"));

    assert_eq!(harness.press(on_scrim()).0, vec!["dismissed"]);
}

#[test]
fn tapping_the_surface_never_dismisses() {
    // Clicking the dialog itself is not "clicking outside it".
    let mut harness = Harness::new(dialog(body()).on_dismiss("dismissed"));
    let at = on_surface(&harness);

    assert!(
        harness.press(at).0.is_empty(),
        "a tap on the dialog must not dismiss it"
    );
}

#[test]
fn scrim_dismissal_can_be_refused_while_escape_still_works() {
    // A destructive confirmation may reasonably want a mis-tap outside it
    // to do nothing -- but Escape must still work, or the dialog would be
    // unescapable by keyboard.
    let mut harness = Harness::new(
        dialog(body())
            .on_dismiss("dismissed")
            .dismiss_on_scrim(false),
    );

    assert!(
        harness.press(on_scrim()).0.is_empty(),
        "the scrim must be inert when dismiss_on_scrim is off"
    );
    assert_eq!(harness.escape().0, vec!["dismissed"]);
}

#[test]
fn a_non_dismissible_dialog_ignores_escape_and_the_scrim() {
    let mut harness: Harness<'_> = Harness::new(dialog(body()));

    assert!(harness.escape().0.is_empty());
    assert!(harness.press(on_scrim()).0.is_empty());
}

// ---------------------------------------------------------------------
// Inertness of the page behind
// ---------------------------------------------------------------------

#[test]
fn a_dialog_swallows_pointer_events_so_the_page_behind_is_inert() {
    // The page is not merely covered: a click that lands on the scrim must
    // not reach whatever it happens to be covering.
    let mut harness: Harness<'_> = Harness::new(dialog(body()));

    let (_, captured) = harness.press(on_scrim());

    assert!(
        captured,
        "a modal must capture pointer events, or the page behind stays live"
    );
}

#[test]
fn a_dialogs_own_content_still_receives_its_clicks() {
    // The dialog must not swallow its own buttons.
    let mut harness = Harness::new(dialog(button(text("OK")).on_press("ok")));
    let at = on_surface(&harness);

    assert_eq!(
        harness.press(at).0,
        Vec::<Message>::new(),
        "press alone publishes nothing"
    );

    let (messages, _) = harness.dispatch(
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        at,
    );

    assert_eq!(messages, vec!["ok"], "the dialog's own button must work");
}

// ---------------------------------------------------------------------
// Semantics
// ---------------------------------------------------------------------

#[test]
fn a_dialog_reports_as_a_window_that_can_be_dismissed() {
    let harness = Harness::new(dialog(body()).on_dismiss("dismissed"));
    let node = harness.semantics().expect("must report semantics");

    assert_eq!(node.role, Role::Window);
    assert!(node.actions.contains(Actions::DISMISS));
}

#[test]
fn a_non_dismissible_dialog_offers_no_dismiss_action() {
    let harness: Harness<'_> = Harness::new(dialog(body()));
    let node = harness.semantics().unwrap();

    assert_eq!(node.role, Role::Window);
    assert!(
        node.actions.is_empty(),
        "offering DISMISS on a dialog that ignores it would be a lie"
    );
}

#[test]
fn a_dialogs_semantic_bounds_are_the_surface_not_the_scrim() {
    // A screen reader pointing at the dialog must point at the dialog, not
    // at the whole window.
    let harness = Harness::new(dialog(body()).on_dismiss("dismissed"));
    let node = harness.semantics().unwrap();

    assert_eq!(node.bounds, harness.surface());
    assert_ne!(node.bounds, harness.scrim());
}

// ---------------------------------------------------------------------
// The Phase 13 focus trap, end-to-end
// ---------------------------------------------------------------------

#[test]
fn a_dialog_declares_itself_modal_so_a_caller_can_trap_focus() {
    let harness = Harness::new(dialog(body()).on_dismiss("dismissed"));

    assert!(
        harness.widget.is_modal(),
        "a dialog that did not own focus would let Tab wander into the inert page"
    );
    assert!(harness.widget.is_dismissible());
}

#[test]
fn a_real_dialogs_node_can_own_a_real_focus_trap() {
    // The integration Phase 13 exists for: the widget reports a Window
    // node, and `taim_accessibility::Focus` confines Tab to it.
    let dialog_harness = Harness::new(dialog(body()).on_dismiss("dismissed"));
    let dialog_node = dialog_harness.semantics().unwrap();
    let dialog_id = dialog_node.id.clone();

    // A page behind, and the dialog with a control inside it.
    let behind = taim_core::semantics::Node::new(
        taim_core::widget::Id::unique(),
        Rectangle::new(Point::ORIGIN, Size::new(100.0, 40.0)),
        Role::Button,
    )
    .with_actions(Actions::ACTIVATE | Actions::FOCUS);
    let behind_id = behind.id.clone();

    let inside = taim_core::semantics::Node::new(
        taim_core::widget::Id::unique(),
        Rectangle::new(Point::new(400.0, 300.0), Size::new(80.0, 40.0)),
        Role::Button,
    )
    .with_actions(Actions::ACTIVATE | Actions::FOCUS);
    let inside_id = inside.id.clone();

    let tree = SemanticTree::from_forest(&[behind, dialog_node.with_child(inside)]);

    let mut focus = Focus::new();
    assert!(
        focus.focus(&tree, &behind_id),
        "the page is reachable first"
    );

    focus.push_scope(
        &tree,
        Scope {
            owner: dialog_id,
            trapped: true,
        },
    );

    assert_eq!(
        focus.focused(),
        Some(&inside_id),
        "opening the dialog must autofocus inside it"
    );

    // Tab stays inside, however many times.
    for _ in 0..5 {
        let _ = focus.next(&tree);
        assert_eq!(
            focus.focused(),
            Some(&inside_id),
            "Tab must not escape the dialog into the page behind"
        );
    }

    assert!(
        !focus.focus(&tree, &behind_id),
        "the page behind must be unreachable while the dialog is open"
    );

    // Closing restores focus to whatever opened it.
    focus.pop_scope(&tree);
    assert_eq!(focus.focused(), Some(&behind_id));
    assert!(!focus.is_trapped());
}

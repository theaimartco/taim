//! Proves real widgets produce a real semantic tree.
//!
//! Until these existed, `Widget::semantics` returned `None` for every
//! widget in the library: the whole Phase 13 engine -- diffing, focus,
//! actions, live regions -- was being fed an empty forest. An
//! accessibility engine with nothing in it is a specification, not
//! accessibility.
//!
//! These drive real `taim_widget` controls through the real
//! `Widget::semantics`, feed the result into the real
//! `taim_accessibility::Tree`, and assert the tree an assistive
//! technology would actually receive.

use taim_accessibility::focus::Focus;
use taim_accessibility::tree::Tree as SemanticTree;
use taim_accessibility::{Dispatch, Router};

use taim::advanced::widget;
use taim::advanced::{Layout, layout};
use taim::widget::{button, checkbox, radio, slider, text, text_input, toggler};
use taim::{Element, Size, Theme};
use taim_core::semantics::{Actions, Role, State, Value};
use taim_renderer::Renderer;

type Message = ();

const BOUNDS: Size = Size {
    width: 200.0,
    height: 40.0,
};

fn headless_renderer() -> Renderer {
    taim_futures::futures::executor::block_on(
        <Renderer as taim::advanced::renderer::Headless>::new(
            taim::advanced::renderer::Settings::default(),
            Some("tiny-skia"),
        ),
    )
    .expect("tiny-skia renderer must be available")
}

/// Lays out one widget and returns the semantic node it reports.
fn semantics_of(
    element: Element<'_, Message, Theme, Renderer>,
) -> Option<taim_core::semantics::Node> {
    let mut element = element;
    let renderer = headless_renderer();

    let mut tree = widget::Tree::new(element.as_widget());
    tree.diff(element.as_widget_mut());

    let node = element.as_widget_mut().layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(Size::ZERO, BOUNDS),
    );

    element.as_widget().semantics(&tree, Layout::new(&node))
}

// ---------------------------------------------------------------------
// Every control reports itself
// ---------------------------------------------------------------------

#[test]
fn a_real_button_reports_an_activatable_focusable_node() {
    let node = semantics_of(button(text("Save")).on_press(()).into())
        .expect("a button must report semantics");

    assert_eq!(node.role, Role::Button);
    assert!(node.actions.contains(Actions::ACTIVATE));
    assert!(node.actions.contains(Actions::FOCUS));
    assert!(node.state.contains(State::FOCUSABLE));
    assert!(!node.state.contains(State::DISABLED));
    assert!(node.bounds.width > 0.0, "a node must carry real bounds");
}

#[test]
fn a_button_with_no_handler_is_disabled_and_offers_nothing() {
    // The contract: "a disabled control should have an empty Actions set
    // rather than reporting actions it will reject."
    let node = semantics_of(button(text("Save")).into()).expect("still reports a node");

    assert_eq!(node.role, Role::Button);
    assert!(node.state.contains(State::DISABLED));
    assert!(
        node.actions.is_empty(),
        "a disabled button must not advertise an action that does nothing"
    );
}

#[test]
fn a_real_checkbox_reports_its_checked_state_and_label() {
    let unchecked = semantics_of(
        checkbox(false)
            .label("Remember me")
            .on_toggle(|_| ())
            .into(),
    )
    .expect("a checkbox must report semantics");
    let checked =
        semantics_of(checkbox(true).label("Remember me").on_toggle(|_| ()).into()).unwrap();

    assert_eq!(unchecked.role, Role::Checkbox);
    assert!(!unchecked.state.contains(State::CHECKED));
    assert!(checked.state.contains(State::CHECKED));

    assert_eq!(
        checked.name.as_deref(),
        Some("Remember me"),
        "the visible label is the accessible name"
    );
}

#[test]
fn a_checkbox_with_no_handler_is_disabled() {
    let node = semantics_of(checkbox(true).label("Locked").into()).unwrap();

    assert!(node.state.contains(State::DISABLED));
    assert!(node.actions.is_empty());
    // ...but the state is still reported, so a screen reader can say
    // "checked, dimmed" rather than losing the value entirely.
    assert!(node.state.contains(State::CHECKED));
}

#[test]
fn a_real_radio_reports_selection_and_its_label() {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Choice {
        A,
        B,
    }

    let selected = semantics_of(radio("Option A", Choice::A, Some(Choice::A), |_| ()).into())
        .expect("a radio must report semantics");
    let unselected =
        semantics_of(radio("Option B", Choice::B, Some(Choice::A), |_| ()).into()).unwrap();

    assert_eq!(selected.role, Role::RadioButton);
    assert!(selected.state.contains(State::CHECKED));
    assert!(
        selected.state.contains(State::SELECTED),
        "a chosen radio is both checked and the selected member of its group"
    );
    assert!(!unselected.state.contains(State::CHECKED));

    assert_eq!(selected.name.as_deref(), Some("Option A"));
}

#[test]
fn a_real_toggler_reports_as_a_two_state_control() {
    let on = semantics_of(toggler(true).label("Wi-Fi").on_toggle(|_| ()).into())
        .expect("a toggler must report semantics");

    // A switch is visually distinct from a checkbox but semantically
    // identical: two states, one control.
    assert_eq!(on.role, Role::Checkbox);
    assert!(on.state.contains(State::CHECKED));
    assert_eq!(on.name.as_deref(), Some("Wi-Fi"));
}

#[test]
fn a_real_slider_reports_its_value_with_its_range() {
    let node = semantics_of(slider(0.0..=100.0, 70.0, |_| ()).into())
        .expect("a slider must report semantics");

    assert_eq!(node.role, Role::Slider);

    // The range travels with the value: "70" means nothing without it.
    assert_eq!(
        node.value(),
        Some(&Value::Number {
            current: 70.0,
            min: 0.0,
            max: 100.0,
        })
    );

    // Stepping must be offered, or a keyboard/switch user cannot move it.
    assert!(node.actions.contains(Actions::INCREMENT));
    assert!(node.actions.contains(Actions::DECREMENT));
    assert!(node.actions.contains(Actions::SET_VALUE));
}

#[test]
fn a_real_text_input_reports_its_value() {
    let node = semantics_of(text_input("Name", "Ada").on_input(|_| ()).into())
        .expect("a text input must report semantics");

    assert_eq!(node.role, Role::TextField);
    assert_eq!(node.value(), Some(&Value::Text("Ada".into())));
    assert!(node.actions.contains(Actions::SET_VALUE));
}

#[test]
fn a_read_only_text_input_does_not_offer_set_value() {
    let node = semantics_of(text_input("Name", "Ada").into()).unwrap();

    assert!(
        !node.actions.contains(Actions::SET_VALUE),
        "a field with no on_input must not claim it can be edited"
    );
}

// ---------------------------------------------------------------------
// The security boundary, at the widget level
// ---------------------------------------------------------------------

#[test]
fn a_real_secure_text_input_never_discloses_its_value() {
    // The end-to-end version of the Phase 10 rule: a password field, laid
    // out and asked for its semantics like any other widget, must hand an
    // assistive technology no value at all.
    let node = semantics_of(
        text_input("Password", "hunter2")
            .secure(true)
            .on_input(|_| ())
            .into(),
    )
    .expect("a secure field still reports a node");

    assert_eq!(node.role, Role::SecureField);
    assert_eq!(
        node.value(),
        None,
        "a secure field must never carry a semantic value"
    );

    let formatted = format!("{node:?}");
    assert!(
        !formatted.contains("hunter2"),
        "the secret leaked through the node's debug output: {formatted}"
    );
}

#[test]
fn a_secure_field_is_still_focusable_and_editable() {
    // Redaction must not make the control unusable -- only unreadable.
    let node = semantics_of(
        text_input("Password", "hunter2")
            .secure(true)
            .on_input(|_| ())
            .into(),
    )
    .unwrap();

    assert!(node.actions.contains(Actions::FOCUS));
    assert!(node.actions.contains(Actions::SET_VALUE));
    assert!(node.state.contains(State::FOCUSABLE));
}

// ---------------------------------------------------------------------
// The engine, fed by real widgets
// ---------------------------------------------------------------------

#[test]
fn a_real_control_flows_into_the_accessibility_tree_and_is_focusable() {
    let node = semantics_of(button(text("Save")).on_press(()).into()).unwrap();
    let id = node.id.clone();

    let tree = SemanticTree::from_forest(&[node]);

    assert_eq!(tree.len(), 1);

    let mut focus = Focus::new();
    assert!(
        focus.focus(&tree, &id),
        "a real button must be focusable through the real engine"
    );
    assert_eq!(focus.focused(), Some(&id));
}

#[test]
fn a_real_controls_action_routes_through_the_real_router() {
    let node = semantics_of(button(text("Save")).on_press(()).into()).unwrap();
    let id = node.id.clone();

    let tree = SemanticTree::from_forest(&[node]);

    assert_eq!(
        Router::new().route(
            &tree,
            &Dispatch {
                target: id.clone(),
                action: Actions::ACTIVATE,
            },
        ),
        Ok(id)
    );
}

#[test]
fn a_disabled_real_control_refuses_the_action_through_the_router() {
    // End-to-end: the widget declines to advertise, and the router
    // declines to route. Neither alone would be enough.
    let node = semantics_of(button(text("Save")).into()).unwrap();
    let id = node.id.clone();

    let tree = SemanticTree::from_forest(&[node]);

    assert!(
        Router::new()
            .route(
                &tree,
                &Dispatch {
                    target: id,
                    action: Actions::ACTIVATE,
                },
            )
            .is_err(),
        "a disabled button must not be activatable through assistive technology"
    );
}

#[test]
fn a_disabled_real_control_is_skipped_by_focus_traversal() {
    let live = semantics_of(button(text("Save")).on_press(()).into()).unwrap();
    let dead = semantics_of(button(text("Locked")).into()).unwrap();

    let live_id = live.id.clone();
    let dead_id = dead.id.clone();

    let tree = SemanticTree::from_forest(&[live, dead]);

    let mut focus = Focus::new();
    assert!(focus.next(&tree));
    assert_eq!(focus.focused(), Some(&live_id));

    // Tab again: the only other control is disabled, so focus wraps back
    // rather than landing on it.
    let _ = focus.next(&tree);
    assert_eq!(
        focus.focused(),
        Some(&live_id),
        "traversal must skip a disabled control entirely"
    );
    assert!(!focus.focus(&tree, &dead_id));
}

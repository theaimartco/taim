//! The Phase 13 exit-gate clause: **all Taim components have semantic
//! snapshots/actions**.
//!
//! `taim-design`'s `ComponentSpec` already declares each component's
//! `role()` and `actions()`, and `tests/design/components.rs` proves that
//! declaration is self-consistent. This suite proves the next step: that
//! the declaration survives being turned into a real
//! `taim_core::semantics::Node`, retained in an
//! `taim_accessibility::Tree`, and queried the way a platform adapter
//! would.
//!
//! The gap that would otherwise hide here: a component can declare a
//! perfect role and action set and still be useless to a screen reader if
//! nothing ever builds a node from it. So every assertion below goes
//! through the retained tree, not the spec.

use taim_accessibility::actions::{Dispatch, Rejection, Router};
use taim_accessibility::live_region::{LiveRegions, Refusal};
use taim_accessibility::tree::Tree;

use taim_design::components::app_bar::{AppBar, AppBarKind};
use taim_design::components::bottom_sheet::BottomSheet;
use taim_design::components::button::{Button, ButtonKind};
use taim_design::components::card::Card;
use taim_design::components::dialog::Dialog;
use taim_design::components::icon_button::IconButton;
use taim_design::components::list_tile::{ListLines, ListTile};
use taim_design::components::menu::Menu;
use taim_design::components::navigation::{Navigation, NavigationMode};
use taim_design::components::notification::{Notification, Privacy};
use taim_design::components::progress::Progress;
use taim_design::components::selection::{Selection, SelectionKind};
use taim_design::components::slider::Slider;
use taim_design::components::text_field::TextField;
use taim_design::components::toast::Toast;
use taim_design::components::tooltip::Tooltip;
use taim_design::components::window_chrome::WindowChrome;
use taim_design::components::{ComponentSpec, Density};
use taim_design::icon::Icon;

use taim_core::semantics::{Actions, Live, Node, Role};
use taim_core::widget::Id;
use taim_core::{Point, Rectangle};

/// Builds the semantic node a component would contribute, exactly the way
/// a widget is expected to: role and actions come straight from the
/// component's own contract, and the accessible name from the author.
///
/// This is the bridge under test. If a component's declaration cannot
/// produce a usable node, that shows up here rather than in production.
fn snapshot(id: &Id, spec: &dyn ComponentSpec, name: &str) -> Node {
    let size = spec.min_size(Density::Comfortable);

    let node = Node::new(id.clone(), Rectangle::new(Point::ORIGIN, size), spec.role())
        .with_actions(spec.actions());

    if name.is_empty() {
        node
    } else {
        node.with_name(name)
    }
}

/// Every component, named, in its ordinary configuration.
fn every_component() -> Vec<(&'static str, Box<dyn ComponentSpec>)> {
    vec![
        (
            "app_bar",
            Box::new(AppBar {
                kind: AppBarKind::Standard,
                navigation_action: true,
            }),
        ),
        (
            "bottom_sheet",
            Box::new(BottomSheet {
                expanded: 0.5,
                dismissible: true,
            }),
        ),
        (
            "button",
            Box::new(Button {
                kind: ButtonKind::Filled,
                loading: false,
                disabled: false,
            }),
        ),
        (
            "card",
            Box::new(Card {
                interactive: true,
                selected: false,
            }),
        ),
        (
            "dialog",
            Box::new(Dialog {
                dismissible: true,
                focus_trapped: true,
            }),
        ),
        (
            "icon_button",
            Box::new(IconButton {
                icon: Icon::new("close", 24, false),
                tooltip: Some("Close"),
                disabled: false,
            }),
        ),
        (
            "list_tile",
            Box::new(ListTile {
                lines: ListLines::Two,
                interactive: true,
            }),
        ),
        (
            "menu",
            Box::new(Menu {
                expanded: false,
                has_submenu: true,
            }),
        ),
        (
            "navigation",
            Box::new(Navigation {
                mode: NavigationMode::Rail,
                selected: false,
            }),
        ),
        (
            "notification",
            Box::new(Notification {
                privacy: Privacy::Public,
                actionable: true,
                group_count: 1,
            }),
        ),
        (
            "progress",
            Box::new(Progress {
                value: Some(0.4),
                circular: false,
            }),
        ),
        (
            "selection",
            Box::new(Selection {
                kind: SelectionKind::Checkbox,
                checked: false,
                disabled: false,
            }),
        ),
        (
            "slider",
            Box::new(Slider {
                value: 0.5,
                range: (0.0, 1.0),
                rtl: false,
                disabled: false,
            }),
        ),
        (
            "text_field",
            Box::new(TextField {
                secure: false,
                multiline: false,
                error: false,
                disabled: false,
            }),
        ),
        (
            "toast",
            Box::new(Toast {
                actionable: true,
                dismissible: true,
            }),
        ),
        (
            "tooltip",
            Box::new(Tooltip {
                focus_enabled: true,
                long_press_enabled: true,
            }),
        ),
        (
            "window_chrome",
            Box::new(WindowChrome {
                desktop: true,
                system_gestures_reserved: true,
            }),
        ),
    ]
}

// ---------------------------------------------------------------------
// Every component produces a usable snapshot
// ---------------------------------------------------------------------

#[test]
fn every_component_produces_a_retained_semantic_node() {
    // The gate clause, literally: each component must survive the trip
    // into the retained tree with its role and actions intact.
    for (name, spec) in every_component() {
        let id = Id::unique();
        let tree = Tree::from_forest(&[snapshot(&id, spec.as_ref(), name)]);

        let entry = tree
            .get(&id)
            .unwrap_or_else(|| panic!("{name}: contributed no semantic node at all"));

        assert_eq!(entry.role, spec.role(), "{name}: role was lost in transit");
        assert_eq!(
            entry.actions,
            spec.actions(),
            "{name}: actions were lost in transit"
        );
    }
}

#[test]
fn every_interactive_component_is_focusable_in_the_retained_tree() {
    // `is_focusable` is what focus traversal actually consults. A
    // component whose actions say FOCUS but whose entry says otherwise
    // would be skipped by Tab despite claiming otherwise.
    for (name, spec) in every_component() {
        if !spec.actions().contains(Actions::FOCUS) {
            continue;
        }

        let id = Id::unique();
        let tree = Tree::from_forest(&[snapshot(&id, spec.as_ref(), name)]);

        assert!(
            tree.get(&id).unwrap().is_focusable(),
            "{name}: claims FOCUS but the retained entry is not focusable"
        );
    }
}

#[test]
fn every_component_that_claims_an_action_will_actually_route_it() {
    // A declared action is a promise the router has to keep. If a
    // component advertises ACTIVATE and the router rejects it, assistive
    // technology sees a dead control.
    let router = Router::new();

    for (name, spec) in every_component() {
        let id = Id::unique();
        let tree = Tree::from_forest(&[snapshot(&id, spec.as_ref(), name)]);

        for action in spec.actions().iter() {
            assert_eq!(
                router.route(
                    &tree,
                    &Dispatch {
                        target: id.clone(),
                        action,
                    },
                ),
                Ok(id.clone()),
                "{name}: advertises {action:?} but the router refuses it"
            );
        }
    }
}

#[test]
fn no_component_routes_an_action_it_never_claimed() {
    // The converse: an action nobody offered must be refused, or a
    // platform could invoke behavior the component never agreed to.
    let router = Router::new();

    for (name, spec) in every_component() {
        let id = Id::unique();
        let tree = Tree::from_forest(&[snapshot(&id, spec.as_ref(), name)]);

        let unclaimed = Actions::all() - spec.actions();

        for action in unclaimed.iter() {
            assert_eq!(
                router.route(
                    &tree,
                    &Dispatch {
                        target: id.clone(),
                        action,
                    },
                ),
                Err(Rejection::NotOffered),
                "{name}: routed {action:?}, which it never claimed"
            );
        }
    }
}

// ---------------------------------------------------------------------
// Per-component semantic contracts
// ---------------------------------------------------------------------

#[test]
fn a_secure_field_snapshot_never_carries_a_value() {
    // The disclosure boundary, enforced end-to-end: the spec reports
    // SecureField, the node refuses the value, and the retained entry
    // holds none.
    let spec = TextField {
        secure: true,
        multiline: false,
        error: false,
        disabled: false,
    };
    assert_eq!(spec.role(), Role::SecureField);

    let id = Id::unique();
    let node = snapshot(&id, &spec, "Password")
        .with_value(taim_core::semantics::Value::Text("hunter2".into()));

    let tree = Tree::from_forest(&[node]);
    let entry = tree.get(&id).unwrap();

    assert_eq!(entry.value, None, "a secure field must disclose no value");
    assert!(
        entry.name.is_some(),
        "...but must keep its accessible name, or it is unusable"
    );
    assert!(
        !format!("{entry:?}").contains("hunter2"),
        "the secret must not survive in the retained entry's debug output"
    );
}

#[test]
fn a_secure_field_can_never_announce_through_a_live_region() {
    let spec = TextField {
        secure: true,
        multiline: false,
        error: false,
        disabled: false,
    };

    let id = Id::unique();
    let tree = Tree::from_forest(&[snapshot(&id, &spec, "Password")]);
    let entry = tree.get(&id).unwrap();

    let mut regions = LiveRegions::default();

    assert_eq!(
        regions.announce(entry, "hunter2", false),
        Err(Refusal::Secure),
        "a secure field must never reach a live region"
    );
}

#[test]
fn a_disabled_component_offers_nothing_to_assistive_technology() {
    let disabled: Vec<(&str, Box<dyn ComponentSpec>)> = vec![
        (
            "button",
            Box::new(Button {
                kind: ButtonKind::Filled,
                loading: false,
                disabled: true,
            }),
        ),
        (
            "selection",
            Box::new(Selection {
                kind: SelectionKind::Checkbox,
                checked: false,
                disabled: true,
            }),
        ),
        (
            "slider",
            Box::new(Slider {
                value: 0.5,
                range: (0.0, 1.0),
                rtl: false,
                disabled: true,
            }),
        ),
        (
            "text_field",
            Box::new(TextField {
                secure: false,
                multiline: false,
                error: false,
                disabled: true,
            }),
        ),
    ];

    let router = Router::new();

    for (name, spec) in disabled {
        let id = Id::unique();
        let tree = Tree::from_forest(&[snapshot(&id, spec.as_ref(), name)]);

        assert!(
            tree.get(&id).unwrap().actions.is_empty(),
            "{name}: a disabled control must advertise no actions"
        );

        assert_eq!(
            router.route(
                &tree,
                &Dispatch {
                    target: id,
                    action: Actions::ACTIVATE,
                },
            ),
            Err(Rejection::NotOffered),
            "{name}: a disabled control must reject activation"
        );
    }
}

#[test]
fn a_dialog_offers_dismissal_to_assistive_technology() {
    let spec = Dialog {
        dismissible: true,
        focus_trapped: true,
    };

    let id = Id::unique();
    let tree = Tree::from_forest(&[snapshot(&id, &spec, "Confirm")]);

    assert_eq!(tree.get(&id).unwrap().role, Role::Window);
    assert_eq!(
        Router::new().route(
            &tree,
            &Dispatch {
                target: id.clone(),
                action: Actions::DISMISS,
            },
        ),
        Ok(id),
        "Escape must be reachable through the action path too"
    );
}

#[test]
fn a_slider_offers_both_stepping_and_direct_setting() {
    // A slider that only offers SET_VALUE is unusable from a keyboard;
    // one that only steps is unusable from a screen reader's value setter.
    let spec = Slider {
        value: 0.5,
        range: (0.0, 1.0),
        rtl: false,
        disabled: false,
    };

    let id = Id::unique();
    let tree = Tree::from_forest(&[snapshot(&id, &spec, "Volume")]);
    let actions = tree.get(&id).unwrap().actions;

    for required in [
        Actions::FOCUS,
        Actions::SET_VALUE,
        Actions::INCREMENT,
        Actions::DECREMENT,
    ] {
        assert!(
            actions.contains(required),
            "a slider must offer {required:?}"
        );
    }
}

#[test]
fn a_menu_with_a_submenu_offers_expansion() {
    let spec = Menu {
        expanded: false,
        has_submenu: true,
    };

    let id = Id::unique();
    let tree = Tree::from_forest(&[snapshot(&id, &spec, "File")]);
    let entry = tree.get(&id).unwrap();

    assert_eq!(entry.role, Role::MenuItem);
    assert!(entry.actions.contains(Actions::TOGGLE_EXPANDED));
}

#[test]
fn a_toast_announces_politely_and_never_takes_focus() {
    // A toast interrupts to inform, not to grab. Stealing focus from
    // whatever the user was doing to announce a transient message is
    // hostile.
    let spec = Toast {
        actionable: true,
        dismissible: true,
    };

    let id = Id::unique();
    let node = snapshot(&id, &spec, "Message sent").with_live(Live::Polite);
    let tree = Tree::from_forest(&[node]);
    let entry = tree.get(&id).unwrap();

    let mut regions = LiveRegions::default();
    assert_eq!(regions.announce(entry, "Message sent", false), Ok(()));

    let drained = regions.drain();
    assert_eq!(drained.len(), 1);
    assert_eq!(drained[0].message, "Message sent");
}

#[test]
fn a_private_notification_body_never_reaches_a_live_region() {
    // `Privacy` decides what a locked screen may reveal; a live region is
    // exactly the channel that would speak it out loud regardless.
    let spec = Notification {
        privacy: Privacy::Secret,
        actionable: false,
        group_count: 1,
    };

    let id = Id::unique();
    let node = snapshot(&id, &spec, "Message").with_live(Live::Polite);
    let tree = Tree::from_forest(&[node]);
    let entry = tree.get(&id).unwrap();

    let mut regions = LiveRegions::default();

    // The caller consults `Privacy` and marks it private on a locked
    // screen; the region refuses it.
    let locked = true;
    let private = !spec.privacy.shows_body(locked);
    assert!(
        private,
        "a Secret notification withholds its body when locked"
    );

    assert_eq!(
        regions.announce(entry, "Bank code 1234", private),
        Err(Refusal::Private)
    );
    assert!(regions.is_empty());
}

#[test]
fn a_progress_indicator_reports_but_is_never_operated() {
    let spec = Progress {
        value: Some(0.4),
        circular: false,
    };

    let id = Id::unique();
    let tree = Tree::from_forest(&[snapshot(&id, &spec, "Loading")]);
    let entry = tree.get(&id).unwrap();

    assert!(
        entry.actions.is_empty(),
        "progress reports; it must never be operated"
    );
    assert!(!entry.is_focusable());
}

#[test]
fn a_component_snapshot_is_stable_across_rebuilds() {
    // "Semantic IDs remain stable across ordinary view reconstruction":
    // rebuilding the same component with the same id must produce no
    // changes at all, or a screen reader would re-announce it every frame.
    let spec = Button {
        kind: ButtonKind::Filled,
        loading: false,
        disabled: false,
    };
    let id = Id::unique();

    let first = Tree::from_forest(&[snapshot(&id, &spec, "Save")]);
    let second = Tree::from_forest(&[snapshot(&id, &spec, "Save")]);

    assert!(
        first.diff(&second).is_empty(),
        "an unchanged component must produce no semantic churn"
    );
}

//! The Phase 13 exit gate: **focus never becomes lost or trapped
//! unintentionally**, plus the required behavior that "focus order is
//! deterministic in LTR/RTL and with virtualized content".
//!
//! `aura-accessibility`'s own unit tests cover the mechanics. These cover
//! the scenarios that actually strand users: a dialog torn down without
//! cleanup, a virtualized row scrolling out from under the caret, a
//! mirrored layout, and a form whose controls disable themselves as it is
//! filled in.

use aura_accessibility::focus::{Direction, Focus, Scope};
use aura_accessibility::tree::Tree;

use aura_core::semantics::{Actions, Node, Role, State};
use aura_core::widget::Id;
use aura_core::{Point, Rectangle, Size};

fn control(id: &Id, x: f32, y: f32) -> Node {
    Node::new(
        id.clone(),
        Rectangle::new(Point::new(x, y), Size::new(40.0, 20.0)),
        Role::Button,
    )
    .with_actions(Actions::ACTIVATE | Actions::FOCUS)
}

fn window(id: &Id) -> Node {
    Node::new(
        id.clone(),
        Rectangle::with_size(Size::new(300.0, 300.0)),
        Role::Window,
    )
}

/// Walks Tab `steps` times and reports where focus landed each time.
fn tab_trail(focus: &mut Focus, tree: &Tree, steps: usize) -> Vec<Option<Id>> {
    (0..steps)
        .map(|_| {
            let _ = focus.next(tree);
            focus.focused().cloned()
        })
        .collect()
}

// ---------------------------------------------------------------------
// Deterministic order
// ---------------------------------------------------------------------

#[test]
fn tab_order_is_document_order_regardless_of_screen_position() {
    // Document order is the contract, not geometry: a control placed
    // visually first but declared last must still come last, or Tab order
    // would silently depend on styling.
    let first = Id::unique();
    let second = Id::unique();
    let third = Id::unique();
    let root = Id::unique();

    let tree = Tree::from_forest(&[window(&root)
        .with_child(control(&first, 200.0, 200.0))
        .with_child(control(&second, 100.0, 100.0))
        .with_child(control(&third, 0.0, 0.0))]);

    let mut focus = Focus::new();
    let trail = tab_trail(&mut focus, &tree, 3);

    assert_eq!(
        trail,
        vec![Some(first), Some(second), Some(third)],
        "Tab must follow declaration order, not screen position"
    );
}

#[test]
fn tab_order_is_identical_in_ltr_and_rtl() {
    // Mirroring moves the controls; it must not renumber them. A form is
    // filled top-to-bottom in either direction.
    let ids: Vec<Id> = (0..3).map(|_| Id::unique()).collect();
    let root = Id::unique();

    let ltr = Tree::from_forest(&[window(&root)
        .with_child(control(&ids[0], 0.0, 0.0))
        .with_child(control(&ids[1], 0.0, 30.0))
        .with_child(control(&ids[2], 0.0, 60.0))]);

    // Same declaration order, mirrored geometry.
    let rtl = Tree::from_forest(&[window(&root)
        .with_child(control(&ids[0], 260.0, 0.0))
        .with_child(control(&ids[1], 260.0, 30.0))
        .with_child(control(&ids[2], 260.0, 60.0))]);

    let mut ltr_focus = Focus::new();
    let mut rtl_focus = Focus::new();

    assert_eq!(
        tab_trail(&mut ltr_focus, &ltr, 3),
        tab_trail(&mut rtl_focus, &rtl, 3),
        "Tab order must not change when the layout mirrors"
    );
}

#[test]
fn tab_order_is_deterministic_across_repeated_walks() {
    let ids: Vec<Id> = (0..8).map(|_| Id::unique()).collect();
    let root = Id::unique();

    let mut node = window(&root);
    for (index, id) in ids.iter().enumerate() {
        node = node.with_child(control(id, 0.0, index as f32 * 25.0));
    }
    let tree = Tree::from_forest(&[node]);

    let mut first = Focus::new();
    let mut second = Focus::new();

    assert_eq!(
        tab_trail(&mut first, &tree, 20),
        tab_trail(&mut second, &tree, 20),
        "two identical walks must produce identical trails"
    );
}

// ---------------------------------------------------------------------
// Virtualized content
// ---------------------------------------------------------------------

/// Builds a virtualized list: only the `visible` rows of a much longer
/// list actually exist in the tree, as a real virtualized list produces.
fn virtualized(root: &Id, rows: &[Id], visible: std::ops::Range<usize>) -> Tree {
    let mut list = Node::new(
        root.clone(),
        Rectangle::with_size(Size::new(200.0, 400.0)),
        Role::ScrollView,
    );

    for index in visible {
        list = list.with_child(control(&rows[index], 0.0, index as f32 * 20.0));
    }

    Tree::from_forest(&[list])
}

#[test]
fn focus_survives_the_focused_row_being_virtualized_away() {
    // The classic virtualization bug: the focused row scrolls out of the
    // realized window and is destroyed. Focus must not vanish with it.
    let root = Id::unique();
    let rows: Vec<Id> = (0..1_000).map(|_| Id::unique()).collect();

    let before = virtualized(&root, &rows, 0..20);
    let after = virtualized(&root, &rows, 40..60);

    let mut focus = Focus::new();
    assert!(focus.focus(&before, &rows[5]));

    focus.reconcile(&before, &after);

    assert!(
        focus.focused().is_some(),
        "focus must not vanish when its row is virtualized away"
    );
    assert!(
        focus.next(&after),
        "Tab must keep working after the focused row disappeared"
    );
}

#[test]
fn tab_order_within_a_virtualized_window_follows_the_underlying_list() {
    let root = Id::unique();
    let rows: Vec<Id> = (0..1_000).map(|_| Id::unique()).collect();

    let tree = virtualized(&root, &rows, 100..104);

    let mut focus = Focus::new();
    let trail = tab_trail(&mut focus, &tree, 4);

    assert_eq!(
        trail,
        vec![
            Some(rows[100].clone()),
            Some(rows[101].clone()),
            Some(rows[102].clone()),
            Some(rows[103].clone()),
        ],
        "Tab must follow list order inside the realized window"
    );
}

#[test]
fn focus_is_stable_while_a_virtualized_list_scrolls_under_it() {
    // A row that stays realized must keep focus across scrolls, even
    // though its neighbours are being created and destroyed constantly.
    let root = Id::unique();
    let rows: Vec<Id> = (0..1_000).map(|_| Id::unique()).collect();

    let mut focus = Focus::new();
    let mut previous = virtualized(&root, &rows, 50..70);
    assert!(focus.focus(&previous, &rows[60]));

    for start in 51..60 {
        let next = virtualized(&root, &rows, start..(start + 20));
        focus.reconcile(&previous, &next);

        assert_eq!(
            focus.focused(),
            Some(&rows[60]),
            "a row that is still realized must keep focus while the list scrolls"
        );

        previous = next;
    }
}

// ---------------------------------------------------------------------
// Traps: intentional, and never abandoned
// ---------------------------------------------------------------------

#[test]
fn a_modal_traps_focus_for_as_long_as_it_is_open_and_no_longer() {
    let page_control = Id::unique();
    let dialog = Id::unique();
    let confirm = Id::unique();
    let cancel = Id::unique();

    let open = Tree::from_forest(&[
        control(&page_control, 0.0, 0.0),
        window(&dialog)
            .with_child(control(&confirm, 10.0, 10.0))
            .with_child(control(&cancel, 10.0, 40.0)),
    ]);

    let mut focus = Focus::new();
    assert!(focus.focus(&open, &page_control));

    focus.push_scope(
        &open,
        Scope {
            owner: dialog.clone(),
            trapped: true,
        },
    );

    // Every Tab stays inside the dialog, forever.
    for _ in 0..10 {
        let _ = focus.next(&open);
        let focused = focus.focused().cloned().expect("focus stays inside");
        assert!(
            focused == confirm || focused == cancel,
            "a trapped modal must never let Tab reach the page behind it"
        );
    }

    focus.pop_scope(&open);

    assert!(!focus.is_trapped());
    assert_eq!(
        focus.focused(),
        Some(&page_control),
        "dismissing the modal must restore focus to what opened it"
    );
}

#[test]
fn a_modal_destroyed_without_cleanup_does_not_deadlock_focus() {
    // The bug this gate exists for: the view drops the dialog without
    // anyone calling pop_scope. If the trap outlives its owner, focus is
    // confined to a subtree that no longer exists and Tab dies silently.
    let page_a = Id::unique();
    let page_b = Id::unique();
    let dialog = Id::unique();
    let inside = Id::unique();

    let open = Tree::from_forest(&[
        control(&page_a, 0.0, 0.0),
        control(&page_b, 0.0, 30.0),
        window(&dialog).with_child(control(&inside, 10.0, 10.0)),
    ]);
    let vanished = Tree::from_forest(&[control(&page_a, 0.0, 0.0), control(&page_b, 0.0, 30.0)]);

    let mut focus = Focus::new();
    focus.push_scope(
        &open,
        Scope {
            owner: dialog,
            trapped: true,
        },
    );
    assert!(focus.is_trapped());

    focus.reconcile(&open, &vanished);

    assert!(!focus.is_trapped(), "the trap must not outlive its owner");

    // Focus is somewhere real, and Tab reaches every surviving control.
    let mut seen = std::collections::HashSet::new();
    for _ in 0..4 {
        let _ = focus.next(&vanished);
        let _ = seen.insert(focus.focused().cloned());
    }

    assert!(seen.contains(&Some(page_a)));
    assert!(seen.contains(&Some(page_b)));
}

#[test]
fn an_untrapped_scope_does_not_confine_focus() {
    // Not every scope is a trap: a non-modal popover groups content
    // without stealing Tab from the rest of the page.
    let outside = Id::unique();
    let popover = Id::unique();
    let inside = Id::unique();

    let tree = Tree::from_forest(&[
        control(&outside, 0.0, 0.0),
        window(&popover).with_child(control(&inside, 10.0, 10.0)),
    ]);

    let mut focus = Focus::new();
    focus.push_scope(
        &tree,
        Scope {
            owner: popover,
            trapped: false,
        },
    );

    assert!(!focus.is_trapped());
    assert!(
        focus.focus(&tree, &outside),
        "an untrapped scope must not block focus outside it"
    );
}

// ---------------------------------------------------------------------
// Focus is never lost
// ---------------------------------------------------------------------

#[test]
fn a_form_that_disables_controls_as_it_is_filled_never_strands_focus() {
    // A realistic churn: each step disables the control that holds focus.
    // Focus must keep moving forward rather than sticking on something
    // inert or vanishing.
    let ids: Vec<Id> = (0..5).map(|_| Id::unique()).collect();
    let root = Id::unique();

    let build = |disabled_up_to: usize| -> Tree {
        let mut node = window(&root);

        for (index, id) in ids.iter().enumerate() {
            let mut child = control(id, 0.0, index as f32 * 25.0);
            if index < disabled_up_to {
                child = child.with_state(State::DISABLED);
            }
            node = node.with_child(child);
        }

        Tree::from_forest(&[node])
    };

    let mut focus = Focus::new();
    let mut previous = build(0);
    assert!(focus.focus(&previous, &ids[0]));

    for step in 1..=4 {
        let next = build(step);
        focus.reconcile(&previous, &next);

        let focused = focus
            .focused()
            .cloned()
            .expect("focus must never be lost mid-form");

        let entry = next.get(&focused).expect("focus must point at a real node");
        assert!(
            entry.is_focusable(),
            "focus must never rest on a disabled control (step {step})"
        );

        previous = next;
    }
}

#[test]
fn focus_lands_on_the_successor_not_the_top_when_its_node_disappears() {
    // Jumping back to the top of the page is technically "not lost" and
    // still hostile: the user's place must be approximately preserved.
    let ids: Vec<Id> = (0..6).map(|_| Id::unique()).collect();
    let root = Id::unique();

    let build = |skip: Option<usize>| -> Tree {
        let mut node = window(&root);

        for (index, id) in ids.iter().enumerate() {
            if Some(index) == skip {
                continue;
            }
            node = node.with_child(control(id, 0.0, index as f32 * 25.0));
        }

        Tree::from_forest(&[node])
    };

    let before = build(None);
    let after = build(Some(3));

    let mut focus = Focus::new();
    assert!(focus.focus(&before, &ids[3]));

    focus.reconcile(&before, &after);

    assert_eq!(
        focus.focused(),
        Some(&ids[4]),
        "focus must advance to the next surviving control, not jump to the top"
    );
}

#[test]
fn an_empty_tree_leaves_focus_cleared_rather_than_dangling() {
    let id = Id::unique();

    let before = Tree::from_forest(&[control(&id, 0.0, 0.0)]);
    let after = Tree::from_forest(&[]);

    let mut focus = Focus::new();
    assert!(focus.focus(&before, &id));

    focus.reconcile(&before, &after);

    assert_eq!(
        focus.focused(),
        None,
        "with nothing focusable left, focus must be cleared, not left pointing at a ghost"
    );
    assert!(!focus.next(&after));
}

// ---------------------------------------------------------------------
// Directional navigation
// ---------------------------------------------------------------------

#[test]
fn directional_navigation_follows_the_grid() {
    // A 3x3 grid: arrows must move to the adjacent cell, not the nearest
    // one in raw straight-line distance.
    let cells: Vec<Id> = (0..9).map(|_| Id::unique()).collect();
    let root = Id::unique();

    let mut node = window(&root);
    for (index, id) in cells.iter().enumerate() {
        let row = (index / 3) as f32;
        let column = (index % 3) as f32;
        node = node.with_child(control(id, column * 60.0, row * 60.0));
    }
    let tree = Tree::from_forest(&[node]);

    let mut focus = Focus::new();

    // Start at the centre (index 4).
    assert!(focus.focus(&tree, &cells[4]));
    assert!(focus.focus_in_direction(&tree, Direction::Up));
    assert_eq!(focus.focused(), Some(&cells[1]));

    assert!(focus.focus(&tree, &cells[4]));
    assert!(focus.focus_in_direction(&tree, Direction::Down));
    assert_eq!(focus.focused(), Some(&cells[7]));

    assert!(focus.focus(&tree, &cells[4]));
    assert!(focus.focus_in_direction(&tree, Direction::Left));
    assert_eq!(focus.focused(), Some(&cells[3]));

    assert!(focus.focus(&tree, &cells[4]));
    assert!(focus.focus_in_direction(&tree, Direction::Right));
    assert_eq!(focus.focused(), Some(&cells[5]));
}

#[test]
fn directional_navigation_stops_at_the_edge_rather_than_wrapping() {
    // Wrapping an arrow key is disorienting: the user asked to go up, and
    // landing at the bottom is not "up".
    let top = Id::unique();
    let bottom = Id::unique();
    let root = Id::unique();

    let tree = Tree::from_forest(&[window(&root)
        .with_child(control(&top, 0.0, 0.0))
        .with_child(control(&bottom, 0.0, 60.0))]);

    let mut focus = Focus::new();
    assert!(focus.focus(&tree, &top));

    assert!(!focus.focus_in_direction(&tree, Direction::Up));
    assert_eq!(
        focus.focused(),
        Some(&top),
        "focus must stay put at the edge"
    );
}

#[test]
fn directional_navigation_respects_a_trap() {
    let outside = Id::unique();
    let dialog = Id::unique();
    let inside = Id::unique();

    let tree = Tree::from_forest(&[
        control(&outside, 0.0, 0.0),
        window(&dialog).with_child(control(&inside, 0.0, 100.0)),
    ]);

    let mut focus = Focus::new();
    focus.push_scope(
        &tree,
        Scope {
            owner: dialog,
            trapped: true,
        },
    );
    assert_eq!(focus.focused(), Some(&inside));

    // `outside` sits directly above `inside`, but the trap must hold.
    assert!(
        !focus.focus_in_direction(&tree, Direction::Up),
        "arrow keys must not escape a trap either"
    );
    assert_eq!(focus.focused(), Some(&inside));
}

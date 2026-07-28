//! The incremental-vs-full-rebuild oracle.
//!
//! `phase.md` Phase 13's exit gate is "incremental semantic output
//! matches full rebuild oracle". That is a *property*, not a list of
//! examples, so it is tested as one:
//!
//! > For any two trees `a` and `b`, applying `a.diff(&b)` to `a` must
//! > produce a tree equal to `b`.
//!
//! `Tree::from_forest` is the oracle: a complete rebuild that references
//! nothing previous. `diff` + `apply` is the incremental path. The matrix
//! below generates every *ordered pair* of a set of mutations and checks
//! the property on each, so the guarantee does not rest on whichever
//! cases happened to occur to whoever wrote the test.

use aura_accessibility::tree::{Change, Tree};

use aura_core::semantics::{Actions, Live, Node, Role, State, Value};
use aura_core::widget::Id;
use aura_core::{Point, Rectangle, Size};

/// A fixed cast of identities, so a mutation can refer to the same node
/// across two independently-built forests.
struct Cast {
    root: Id,
    a: Id,
    b: Id,
    c: Id,
    grandchild: Id,
    newcomer: Id,
}

impl Cast {
    fn new() -> Self {
        Self {
            root: Id::unique(),
            a: Id::unique(),
            b: Id::unique(),
            c: Id::unique(),
            grandchild: Id::unique(),
            newcomer: Id::unique(),
        }
    }
}

fn at(id: &Id, role: Role, x: f32, y: f32) -> Node {
    Node::new(
        id.clone(),
        Rectangle::new(Point::new(x, y), Size::new(10.0, 10.0)),
        role,
    )
}

/// One way a tree can differ from the baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mutation {
    /// Nothing changes.
    None,
    /// A leaf's label changes.
    Rename,
    /// A leaf's state flags change.
    Restate,
    /// A leaf's value changes.
    Revalue,
    /// A leaf stops offering its actions.
    Disarm,
    /// A leaf moves on screen, changing nothing else.
    Move,
    /// A leaf is removed.
    Remove,
    /// A leaf is added.
    Add,
    /// Two siblings swap position.
    Reorder,
    /// A leaf's role changes.
    Rerole,
    /// A leaf becomes a live region.
    Enliven,
    /// A branch with a descendant is removed wholesale.
    Prune,
    /// Everything is removed.
    Clear,
}

impl Mutation {
    const ALL: [Self; 13] = [
        Self::None,
        Self::Rename,
        Self::Restate,
        Self::Revalue,
        Self::Disarm,
        Self::Move,
        Self::Remove,
        Self::Add,
        Self::Reorder,
        Self::Rerole,
        Self::Enliven,
        Self::Prune,
        Self::Clear,
    ];
}

/// Builds a forest under `mutation`.
///
/// The baseline is a root with three children, one of which has a
/// grandchild -- enough depth for pruning and reordering to mean
/// something.
fn forest(cast: &Cast, mutation: Mutation) -> Vec<Node> {
    if mutation == Mutation::Clear {
        return Vec::new();
    }

    let alpha = |name: &str, actions: Actions| {
        at(&cast.a, Role::Button, 0.0, 0.0)
            .with_name(name)
            .with_actions(actions)
            .with_child(at(&cast.grandchild, Role::StaticText, 2.0, 2.0).with_name("nested"))
    };

    let beta = |role: Role, state: State, y: f32, live: Option<Live>| {
        let node = at(&cast.b, role, 0.0, y)
            .with_name("Beta")
            .with_state(state);

        match live {
            Some(live) => node.with_live(live),
            None => node,
        }
    };

    let gamma = |current: f64| {
        at(&cast.c, Role::Slider, 0.0, 40.0).with_value(Value::Number {
            current,
            min: 0.0,
            max: 10.0,
        })
    };

    let default_actions = Actions::ACTIVATE | Actions::FOCUS;

    let leaf_a = match mutation {
        Mutation::Rename => alpha("Alpha renamed", default_actions),
        Mutation::Disarm => alpha("Alpha", Actions::empty()),
        _ => alpha("Alpha", default_actions),
    };

    let leaf_b = match mutation {
        Mutation::Restate => beta(
            Role::Checkbox,
            State::FOCUSABLE | State::CHECKED,
            20.0,
            None,
        ),
        Mutation::Move => beta(Role::Checkbox, State::FOCUSABLE, 99.0, None),
        Mutation::Rerole => beta(Role::RadioButton, State::FOCUSABLE, 20.0, None),
        Mutation::Enliven => beta(
            Role::Checkbox,
            State::FOCUSABLE,
            20.0,
            Some(Live::Assertive),
        ),
        _ => beta(Role::Checkbox, State::FOCUSABLE, 20.0, None),
    };

    let leaf_c = match mutation {
        Mutation::Revalue => gamma(9.0),
        _ => gamma(5.0),
    };

    let root = at(&cast.root, Role::Group, 0.0, 0.0);

    let root = match mutation {
        Mutation::Remove => root.with_child(leaf_a).with_child(leaf_c),
        Mutation::Add => root
            .with_child(leaf_a)
            .with_child(leaf_b)
            .with_child(leaf_c)
            .with_child(at(&cast.newcomer, Role::Button, 0.0, 60.0).with_name("New")),
        Mutation::Reorder => root
            .with_child(leaf_b)
            .with_child(leaf_a)
            .with_child(leaf_c),
        // Drops the branch that owns the grandchild: the subtree goes too.
        Mutation::Prune => root.with_child(leaf_b).with_child(leaf_c),
        _ => root
            .with_child(leaf_a)
            .with_child(leaf_b)
            .with_child(leaf_c),
    };

    vec![root]
}

// ---------------------------------------------------------------------
// The exit gate
// ---------------------------------------------------------------------

#[test]
fn applying_a_diff_always_equals_a_full_rebuild() {
    // The property, over every ordered pair of mutations: 13 x 13 = 169
    // transitions, each to and from every other.
    let cast = Cast::new();

    for from in Mutation::ALL {
        for to in Mutation::ALL {
            let before = Tree::from_forest(&forest(&cast, from));
            let after = Tree::from_forest(&forest(&cast, to));

            let changes = before.diff(&after);

            let mut incremental = before.clone();
            incremental.apply(&changes);

            assert_eq!(
                incremental, after,
                "diff from {from:?} to {to:?} did not reproduce a full rebuild"
            );
        }
    }
}

#[test]
fn a_diff_against_an_identical_tree_is_always_empty() {
    // The other half: if nothing changed, an adapter must hear nothing.
    let cast = Cast::new();

    for mutation in Mutation::ALL {
        let a = Tree::from_forest(&forest(&cast, mutation));
        let b = Tree::from_forest(&forest(&cast, mutation));

        assert!(
            a.diff(&b).is_empty(),
            "{mutation:?} produced spurious changes against an identical tree"
        );
    }
}

#[test]
fn a_diff_is_deterministic_for_every_transition() {
    // Hash iteration order must never reach an adapter: announcements
    // would vary run to run.
    let cast = Cast::new();

    for from in Mutation::ALL {
        for to in Mutation::ALL {
            let before = Tree::from_forest(&forest(&cast, from));
            let after = Tree::from_forest(&forest(&cast, to));

            assert_eq!(
                before.diff(&after),
                before.diff(&after),
                "diff from {from:?} to {to:?} is not deterministic"
            );
        }
    }
}

#[test]
fn a_diff_round_trips_losslessly_in_both_directions() {
    // Applying a diff and then its inverse must return the original. A
    // diff that quietly loses information is caught here even when the
    // forward direction happens to look right.
    let cast = Cast::new();

    for mutation in Mutation::ALL {
        let original = Tree::from_forest(&forest(&cast, Mutation::None));
        let mutated = Tree::from_forest(&forest(&cast, mutation));

        let mut there = original.clone();
        there.apply(&original.diff(&mutated));
        assert_eq!(
            there, mutated,
            "forward diff to {mutation:?} lost information"
        );

        let mut back = there.clone();
        back.apply(&mutated.diff(&original));
        assert_eq!(
            back, original,
            "reverse diff from {mutation:?} lost information"
        );
    }
}

// ---------------------------------------------------------------------
// The change vocabulary means what it says
// ---------------------------------------------------------------------

#[test]
fn a_pure_scroll_produces_only_geometry_changes() {
    // The cost claim: a scroll moves everything and re-announces nothing.
    let cast = Cast::new();

    let before = Tree::from_forest(&forest(&cast, Mutation::None));
    let scrolled: Vec<Node> = forest(&cast, Mutation::None)
        .into_iter()
        .map(shift_down)
        .collect();
    let after = Tree::from_forest(&scrolled);

    let changes = before.diff(&after);

    assert!(!changes.is_empty(), "the scroll must be reported at all");
    assert!(
        changes
            .iter()
            .all(|change| matches!(change, Change::Moved { .. })),
        "a scroll must produce only geometry updates, never re-announcements: {changes:?}"
    );
}

/// Rebuilds `node` shifted down, preserving every other property --
/// which is what makes the result a *pure* geometry change.
fn shift_down(node: Node) -> Node {
    let mut moved = Node::new(
        node.id.clone(),
        Rectangle::new(
            Point::new(node.bounds.x, node.bounds.y + 40.0),
            Size::new(node.bounds.width, node.bounds.height),
        ),
        node.role,
    )
    .with_state(node.state)
    .with_actions(node.actions);

    if let Some(name) = &node.name {
        moved = moved.with_name(name.clone());
    }
    if let Some(value) = node.value() {
        moved = moved.with_value(value.clone());
    }
    if let Some(live) = node.live {
        moved = moved.with_live(live);
    }

    for child in node.children {
        moved = moved.with_child(shift_down(child));
    }

    moved
}

#[test]
fn pruning_a_branch_removes_its_descendants_too() {
    let cast = Cast::new();

    let before = Tree::from_forest(&forest(&cast, Mutation::None));
    let after = Tree::from_forest(&forest(&cast, Mutation::Prune));

    assert!(before.get(&cast.grandchild).is_some());
    assert!(
        after.get(&cast.grandchild).is_none(),
        "pruning a branch must take its descendants with it, not orphan them"
    );

    let removed: Vec<Id> = before
        .diff(&after)
        .into_iter()
        .filter_map(|change| match change {
            Change::Removed(id) => Some(id),
            _ => None,
        })
        .collect();

    assert!(removed.contains(&cast.a), "the pruned branch is removed");
    assert!(
        removed.contains(&cast.grandchild),
        "the grandchild must be removed too"
    );
}

#[test]
fn reordering_siblings_reports_a_reorder_rather_than_churn() {
    let cast = Cast::new();

    let before = Tree::from_forest(&forest(&cast, Mutation::None));
    let after = Tree::from_forest(&forest(&cast, Mutation::Reorder));

    let changes = before.diff(&after);

    assert!(
        changes
            .iter()
            .any(|change| matches!(change, Change::Reordered { .. })),
        "swapping siblings must report a reorder: {changes:?}"
    );
    assert!(
        !changes
            .iter()
            .any(|change| matches!(change, Change::Added(_) | Change::Removed(_))),
        "reordering must not look like nodes appearing and vanishing: {changes:?}"
    );
}

#[test]
fn a_relabelled_node_reports_an_update_not_a_replacement() {
    let cast = Cast::new();

    let before = Tree::from_forest(&forest(&cast, Mutation::None));
    let after = Tree::from_forest(&forest(&cast, Mutation::Rename));

    let changes = before.diff(&after);

    assert_eq!(
        changes.len(),
        1,
        "only the renamed node changed: {changes:?}"
    );
    assert!(matches!(&changes[0], Change::Updated { id, .. } if *id == cast.a));
}

#[test]
fn clearing_the_tree_removes_everything_and_rebuilding_restores_it() {
    let cast = Cast::new();

    let populated = Tree::from_forest(&forest(&cast, Mutation::None));
    let empty = Tree::from_forest(&forest(&cast, Mutation::Clear));

    assert!(empty.is_empty());

    let teardown = populated.diff(&empty);
    assert_eq!(teardown.len(), populated.len());
    assert!(
        teardown
            .iter()
            .all(|change| matches!(change, Change::Removed(_))),
        "tearing down must be removals only"
    );

    let mut rebuilt = empty.clone();
    rebuilt.apply(&empty.diff(&populated));
    assert_eq!(rebuilt, populated);
}

#[test]
fn a_secure_field_carries_no_value_through_a_diff() {
    // The redaction must survive the incremental path, not just a
    // rebuild: an Added/Updated change carries a whole Entry.
    let id = Id::unique();

    let secure = at(&id, Role::SecureField, 0.0, 0.0)
        .with_value(Value::Text("hunter2".into()))
        .with_name("Password");

    let before = Tree::from_forest(&[]);
    let after = Tree::from_forest(&[secure]);

    let changes = before.diff(&after);
    assert_eq!(changes.len(), 1);

    let Change::Added(entry) = &changes[0] else {
        panic!("expected the field to be added: {changes:?}");
    };

    assert_eq!(entry.value, None, "a secure field must diff with no value");
    assert!(
        !format!("{changes:?}").contains("hunter2"),
        "the secret leaked through the change set"
    );
}

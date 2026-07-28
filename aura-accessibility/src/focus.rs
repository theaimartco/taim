//! Focus traversal, scopes/traps, restoration, and directional movement.
//!
//! `phase.md` Phase 13's exit gate says focus must "never become lost or
//! trapped unintentionally", and its required behavior adds that focus
//! order must be "deterministic in LTR/RTL and with virtualized content".
//! Those are two failures with one cause -- focus is state that outlives
//! the tree it points into -- so both are handled here:
//!
//! - **Lost.** The node holding focus is removed (a dialog closes, a
//!   virtualized row scrolls out of the cache). [`Focus::reconcile`]
//!   never leaves focus dangling: it walks to the nearest surviving
//!   position rather than dropping to nothing.
//! - **Trapped.** A [`Scope`] deliberately confines focus (a modal must
//!   not leak Tab into the inert content behind it). The trap is
//!   *intentional* while the scope is active and must vanish completely
//!   when its owner does -- an abandoned trap is the classic
//!   accessibility deadlock.

use crate::node::Entry;
use crate::tree::Tree;

use aura_core::widget::Id;
use aura_core::{Point, Rectangle};

/// A direction for spatial focus movement (arrow keys, a gamepad d-pad,
/// switch-style scanning).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// A focus scope: a subtree focus may be confined to.
///
/// A scope is identified by the node that owns it; membership is
/// ancestry, so a scope automatically covers whatever the owner contains,
/// including content that did not exist when the scope opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    /// The node that owns (and roots) this scope.
    pub owner: Id,
    /// Whether focus may leave this scope.
    ///
    /// `true` for a modal dialog: Tab from the last control wraps back to
    /// the first rather than escaping to the page behind.
    pub trapped: bool,
}

/// Retained focus state for one interface.
///
/// Focus is deliberately *not* stored on the tree: the tree is rebuilt
/// and diffed constantly, and focus has to survive that. It is keyed on
/// stable [`Id`], which is what makes "semantic IDs remain stable across
/// ordinary view reconstruction" worth having.
#[derive(Debug, Clone, Default)]
pub struct Focus {
    focused: Option<Id>,
    scopes: Vec<Scope>,
    /// What to return focus to when each scope is dismissed.
    ///
    /// A stack, not a single slot: dialogs open dialogs, and restoring
    /// the wrong one is as disorienting as restoring nothing.
    restore: Vec<Option<Id>>,
}

impl Focus {
    /// Empty focus state: nothing focused, no scopes.
    pub fn new() -> Self {
        Self::default()
    }

    /// The currently focused node, if any.
    pub fn focused(&self) -> Option<&Id> {
        self.focused.as_ref()
    }

    /// The innermost active scope, if any.
    pub fn scope(&self) -> Option<&Scope> {
        self.scopes.last()
    }

    /// Whether focus is currently confined.
    pub fn is_trapped(&self) -> bool {
        self.scopes.last().is_some_and(|scope| scope.trapped)
    }

    /// Focuses `id`, if it exists and can take focus. Returns whether
    /// focus moved.
    ///
    /// Refusing a disabled or hidden node is not a nicety: landing focus
    /// somewhere invisible is exactly how focus gets "lost" from the
    /// user's point of view.
    pub fn focus(&mut self, tree: &Tree, id: &Id) -> bool {
        let Some(entry) = tree.get(id) else {
            return false;
        };

        if !entry.is_focusable() || !self.is_within_active_scope(tree, entry) {
            return false;
        }

        self.focused = Some(id.clone());
        true
    }

    /// Clears focus entirely.
    pub fn clear(&mut self) {
        self.focused = None;
    }

    /// Opens a scope, remembering what to restore focus to, and moves
    /// focus to the scope's first focusable node.
    ///
    /// Autofocus is not optional for a trapped scope: opening a modal and
    /// leaving focus behind it means the very first Tab appears to do
    /// nothing.
    pub fn push_scope(&mut self, tree: &Tree, scope: Scope) {
        self.restore.push(self.focused.clone());

        let owner = scope.owner.clone();
        self.scopes.push(scope);

        // A scope with nothing focusable inside must not strand focus on
        // content the trap has just made unreachable.
        self.focused = self.first_focusable_within(tree, &owner);
    }

    /// Closes the innermost scope and restores focus to whatever held it
    /// when that scope opened.
    ///
    /// If that node is gone (the view changed while the dialog was open),
    /// focus falls back to the first focusable node rather than nothing --
    /// restoring to nowhere is the same failure as losing focus.
    pub fn pop_scope(&mut self, tree: &Tree) {
        let _ = self.scopes.pop();

        let previous = self.restore.pop().flatten();

        self.focused = previous
            .filter(|id| tree.get(id).is_some_and(Entry::is_focusable))
            .or_else(|| self.first_focusable(tree));
    }

    /// Moves focus to the next focusable node in traversal order,
    /// wrapping within the active scope. Returns whether focus moved.
    pub fn next(&mut self, tree: &Tree) -> bool {
        self.step(tree, true)
    }

    /// Moves focus to the previous focusable node in traversal order,
    /// wrapping within the active scope.
    pub fn previous(&mut self, tree: &Tree) -> bool {
        self.step(tree, false)
    }

    fn step(&mut self, tree: &Tree, forward: bool) -> bool {
        let candidates = self.focusable_in_order(tree);

        if candidates.is_empty() {
            // Nothing focusable: clear rather than keep pointing at
            // something that can no longer take focus.
            self.focused = None;
            return false;
        }

        let current = self
            .focused
            .as_ref()
            .and_then(|id| candidates.iter().position(|entry| entry.id == *id));

        let next_index = match current {
            None if forward => 0,
            None => candidates.len() - 1,
            Some(index) if forward => (index + 1) % candidates.len(),
            Some(index) => (index + candidates.len() - 1) % candidates.len(),
        };

        let next = candidates[next_index].id.clone();
        let moved = self.focused.as_ref() != Some(&next);
        self.focused = Some(next);

        moved
    }

    /// Moves focus to the nearest focusable node in `direction`.
    ///
    /// "Nearest" is measured from the focused node's centre, considering
    /// only candidates genuinely in that direction, and weighting
    /// cross-axis distance so a control directly below wins over one far
    /// off to the side but marginally closer in a straight line. That
    /// weighting is what makes arrow navigation follow the layout rather
    /// than the arithmetic.
    ///
    /// Direction is *spatial*, so it needs no RTL special-casing: in a
    /// mirrored layout the controls themselves have moved, and `Left`
    /// still means left.
    pub fn focus_in_direction(&mut self, tree: &Tree, direction: Direction) -> bool {
        let Some(current) = self
            .focused
            .as_ref()
            .and_then(|id| tree.get(id))
            .map(|entry| entry.bounds)
        else {
            // Nothing focused: the first candidate is a better answer
            // than refusing to move at all.
            return self.next(tree);
        };

        let origin = centre(current);

        let best = self
            .focusable_in_order(tree)
            .into_iter()
            .filter(|entry| entry.bounds != current)
            .filter_map(|entry| {
                let target = centre(entry.bounds);

                let (along, across) = match direction {
                    Direction::Up => (origin.y - target.y, (target.x - origin.x).abs()),
                    Direction::Down => (target.y - origin.y, (target.x - origin.x).abs()),
                    Direction::Left => (origin.x - target.x, (target.y - origin.y).abs()),
                    Direction::Right => (target.x - origin.x, (target.y - origin.y).abs()),
                };

                // Strictly in the requested direction, or not a candidate.
                (along > 0.0).then_some((entry, along + across * 2.0))
            })
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(entry, _)| entry.id.clone());

        match best {
            Some(id) => {
                let moved = self.focused.as_ref() != Some(&id);
                self.focused = Some(id);
                moved
            }
            None => false,
        }
    }

    /// Reconciles focus against a tree that has changed.
    ///
    /// This is the "focus is never lost" guarantee. If the focused node
    /// survived and can still take focus, nothing happens. If it did not,
    /// focus moves to the nearest surviving position **in traversal
    /// order** -- the node that would have come next -- rather than
    /// vanishing or jumping back to the top.
    ///
    /// Call after every [`Tree::diff`]/apply cycle.
    pub fn reconcile(&mut self, previous: &Tree, next: &Tree) {
        // A scope whose owner is gone is dismissed: an abandoned trap
        // would confine focus to a subtree that no longer exists, which
        // is the deadlock this module exists to prevent.
        while self
            .scopes
            .last()
            .is_some_and(|scope| next.get(&scope.owner).is_none())
        {
            let _ = self.scopes.pop();
            let _ = self.restore.pop();
        }

        let Some(focused) = self.focused.clone() else {
            return;
        };

        if next
            .get(&focused)
            .is_some_and(|entry| entry.is_focusable() && self.is_within_active_scope(next, entry))
        {
            return;
        }

        // The focused node is gone or no longer focusable. Find where it
        // *was* and take the first surviving focusable node at or after
        // that position.
        let previous_order = previous.get(&focused).map(|entry| entry.order.clone());

        let successor = previous_order.and_then(|order| {
            self.focusable_in_order(next)
                .into_iter()
                .find(|entry| entry.order >= order)
                .map(|entry| entry.id.clone())
        });

        self.focused = successor.or_else(|| self.first_focusable(next));
    }

    /// Whether `entry` is inside the innermost trapped scope (or there is
    /// no trap).
    fn is_within_active_scope(&self, tree: &Tree, entry: &Entry) -> bool {
        let Some(scope) = self.scopes.last() else {
            return true;
        };

        if !scope.trapped || entry.id == scope.owner {
            return true;
        }

        tree.get(&scope.owner)
            .is_some_and(|owner| owner.order.is_ancestor_of(&entry.order))
    }

    /// Every focusable node inside the active scope, in traversal order.
    fn focusable_in_order<'a>(&self, tree: &'a Tree) -> Vec<&'a Entry> {
        tree.in_order()
            .into_iter()
            .filter(|entry| entry.is_focusable())
            .filter(|entry| self.is_within_active_scope(tree, entry))
            .collect()
    }

    fn first_focusable(&self, tree: &Tree) -> Option<Id> {
        self.focusable_in_order(tree)
            .first()
            .map(|entry| entry.id.clone())
    }

    fn first_focusable_within(&self, tree: &Tree, owner: &Id) -> Option<Id> {
        let owner_order = tree.get(owner)?.order.clone();

        tree.in_order()
            .into_iter()
            .filter(|entry| entry.is_focusable())
            .find(|entry| entry.id == *owner || owner_order.is_ancestor_of(&entry.order))
            .map(|entry| entry.id.clone())
    }
}

fn centre(bounds: Rectangle) -> Point {
    Point::new(
        bounds.x + bounds.width / 2.0,
        bounds.y + bounds.height / 2.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use aura_core::Size;
    use aura_core::semantics::{Actions, Node, Role, State};

    fn button(id: &Id, x: f32, y: f32) -> Node {
        Node::new(
            id.clone(),
            Rectangle::new(Point::new(x, y), Size::new(20.0, 20.0)),
            Role::Button,
        )
        .with_actions(Actions::ACTIVATE | Actions::FOCUS)
    }

    #[test]
    fn tab_walks_traversal_order_and_wraps() {
        let a = Id::unique();
        let b = Id::unique();
        let tree = Tree::from_forest(&[button(&a, 0.0, 0.0), button(&b, 0.0, 30.0)]);

        let mut focus = Focus::new();

        assert!(focus.next(&tree));
        assert_eq!(focus.focused(), Some(&a));
        assert!(focus.next(&tree));
        assert_eq!(focus.focused(), Some(&b));

        assert!(focus.next(&tree));
        assert_eq!(
            focus.focused(),
            Some(&a),
            "Tab must wrap, not fall off the end"
        );
    }

    #[test]
    fn shift_tab_walks_backward_and_wraps() {
        let a = Id::unique();
        let b = Id::unique();
        let tree = Tree::from_forest(&[button(&a, 0.0, 0.0), button(&b, 0.0, 30.0)]);

        let mut focus = Focus::new();

        assert!(focus.previous(&tree));
        assert_eq!(
            focus.focused(),
            Some(&b),
            "backward from nothing starts at the end"
        );

        assert!(focus.previous(&tree));
        assert_eq!(focus.focused(), Some(&a));
        assert!(focus.previous(&tree));
        assert_eq!(focus.focused(), Some(&b));
    }

    #[test]
    fn a_disabled_node_is_skipped_entirely() {
        let a = Id::unique();
        let off = Id::unique();
        let b = Id::unique();

        let tree = Tree::from_forest(&[
            button(&a, 0.0, 0.0),
            button(&off, 0.0, 30.0).with_state(State::DISABLED),
            button(&b, 0.0, 60.0),
        ]);

        let mut focus = Focus::new();
        assert!(focus.focus(&tree, &a));
        assert!(focus.next(&tree));

        assert_eq!(
            focus.focused(),
            Some(&b),
            "Tab must step over a disabled control, not land on it"
        );
        assert!(
            !focus.focus(&tree, &off),
            "a disabled node must refuse focus"
        );
    }

    #[test]
    fn a_trapped_scope_confines_tab_and_wraps_inside_it() {
        let outside = Id::unique();
        let dialog = Id::unique();
        let first = Id::unique();
        let last = Id::unique();

        let tree = Tree::from_forest(&[
            button(&outside, 0.0, 0.0),
            Node::new(
                dialog.clone(),
                Rectangle::with_size(Size::new(100.0, 100.0)),
                Role::Window,
            )
            .with_child(button(&first, 10.0, 10.0))
            .with_child(button(&last, 10.0, 40.0)),
        ]);

        let mut focus = Focus::new();
        focus.push_scope(
            &tree,
            Scope {
                owner: dialog,
                trapped: true,
            },
        );

        assert_eq!(
            focus.focused(),
            Some(&first),
            "a modal must autofocus inside"
        );
        assert!(focus.is_trapped());

        assert!(focus.next(&tree));
        assert_eq!(focus.focused(), Some(&last));

        assert!(focus.next(&tree));
        assert_eq!(
            focus.focused(),
            Some(&first),
            "a trapped scope must not leak focus to the content behind it"
        );

        assert!(
            !focus.focus(&tree, &outside),
            "the trap must hold against a direct focus request too"
        );
    }

    #[test]
    fn closing_a_scope_restores_focus_to_where_it_was() {
        let opener = Id::unique();
        let dialog = Id::unique();
        let inside = Id::unique();

        let tree = Tree::from_forest(&[
            button(&opener, 0.0, 0.0),
            Node::new(
                dialog.clone(),
                Rectangle::with_size(Size::new(100.0, 100.0)),
                Role::Window,
            )
            .with_child(button(&inside, 10.0, 10.0)),
        ]);

        let mut focus = Focus::new();
        assert!(focus.focus(&tree, &opener));

        focus.push_scope(
            &tree,
            Scope {
                owner: dialog,
                trapped: true,
            },
        );
        assert_eq!(focus.focused(), Some(&inside));

        focus.pop_scope(&tree);

        assert_eq!(
            focus.focused(),
            Some(&opener),
            "closing a dialog must return focus to whatever opened it"
        );
        assert!(!focus.is_trapped(), "the trap must be gone");
    }

    #[test]
    fn nested_scopes_restore_in_reverse_order() {
        let opener = Id::unique();
        let outer = Id::unique();
        let outer_control = Id::unique();
        let inner = Id::unique();
        let inner_control = Id::unique();

        let tree = Tree::from_forest(&[
            button(&opener, 0.0, 0.0),
            Node::new(
                outer.clone(),
                Rectangle::with_size(Size::new(200.0, 200.0)),
                Role::Window,
            )
            .with_child(button(&outer_control, 10.0, 10.0))
            .with_child(
                Node::new(
                    inner.clone(),
                    Rectangle::with_size(Size::new(100.0, 100.0)),
                    Role::Window,
                )
                .with_child(button(&inner_control, 20.0, 20.0)),
            ),
        ]);

        let mut focus = Focus::new();
        assert!(focus.focus(&tree, &opener));

        focus.push_scope(
            &tree,
            Scope {
                owner: outer,
                trapped: true,
            },
        );
        assert_eq!(focus.focused(), Some(&outer_control));

        focus.push_scope(
            &tree,
            Scope {
                owner: inner,
                trapped: true,
            },
        );
        assert_eq!(focus.focused(), Some(&inner_control));

        focus.pop_scope(&tree);
        assert_eq!(
            focus.focused(),
            Some(&outer_control),
            "the inner dialog must restore to the outer one, not the page"
        );

        focus.pop_scope(&tree);
        assert_eq!(focus.focused(), Some(&opener));
    }

    #[test]
    fn focus_is_never_lost_when_the_focused_node_disappears() {
        let a = Id::unique();
        let doomed = Id::unique();
        let b = Id::unique();

        let before = Tree::from_forest(&[
            button(&a, 0.0, 0.0),
            button(&doomed, 0.0, 30.0),
            button(&b, 0.0, 60.0),
        ]);
        let after = Tree::from_forest(&[button(&a, 0.0, 0.0), button(&b, 0.0, 60.0)]);

        let mut focus = Focus::new();
        assert!(focus.focus(&before, &doomed));

        focus.reconcile(&before, &after);

        assert_eq!(
            focus.focused(),
            Some(&b),
            "focus must move to the next surviving node, not vanish"
        );
    }

    #[test]
    fn focus_survives_the_focused_node_merely_becoming_disabled() {
        let a = Id::unique();
        let b = Id::unique();

        let before = Tree::from_forest(&[button(&a, 0.0, 0.0), button(&b, 0.0, 30.0)]);
        let after = Tree::from_forest(&[
            button(&a, 0.0, 0.0).with_state(State::DISABLED),
            button(&b, 0.0, 30.0),
        ]);

        let mut focus = Focus::new();
        assert!(focus.focus(&before, &a));

        focus.reconcile(&before, &after);

        assert_eq!(
            focus.focused(),
            Some(&b),
            "focus must leave a control that just became disabled"
        );
    }

    #[test]
    fn an_abandoned_trap_is_dismissed_when_its_owner_disappears() {
        // The deadlock: a dialog is torn down without pop_scope. If the
        // trap survived, focus would be confined to a subtree that no
        // longer exists and Tab would do nothing forever.
        //
        // Two surviving controls, not one: with a single candidate Tab
        // correctly wraps to itself and reports "did not move", which
        // would make the recovery assertion below unfalsifiable.
        let outside = Id::unique();
        let also_outside = Id::unique();
        let dialog = Id::unique();
        let inside = Id::unique();

        let before = Tree::from_forest(&[
            button(&outside, 0.0, 0.0),
            button(&also_outside, 0.0, 30.0),
            Node::new(
                dialog.clone(),
                Rectangle::with_size(Size::new(100.0, 100.0)),
                Role::Window,
            )
            .with_child(button(&inside, 10.0, 10.0)),
        ]);
        let after =
            Tree::from_forest(&[button(&outside, 0.0, 0.0), button(&also_outside, 0.0, 30.0)]);

        let mut focus = Focus::new();
        focus.push_scope(
            &before,
            Scope {
                owner: dialog,
                trapped: true,
            },
        );
        assert!(focus.is_trapped());
        assert_eq!(focus.focused(), Some(&inside));

        focus.reconcile(&before, &after);

        assert!(
            !focus.is_trapped(),
            "a trap whose owner is gone must be dismissed, not deadlock focus"
        );
        assert!(
            focus.focused().is_some(),
            "focus must land somewhere real, not vanish with the dialog"
        );

        // Tab genuinely moves again, across the whole surviving tree.
        assert!(focus.next(&after), "Tab must work again");
    }

    #[test]
    fn directional_focus_prefers_the_control_in_that_direction() {
        let origin = Id::unique();
        let below = Id::unique();
        let right = Id::unique();

        let tree = Tree::from_forest(&[
            button(&origin, 100.0, 100.0),
            button(&below, 100.0, 200.0),
            button(&right, 200.0, 100.0),
        ]);

        let mut focus = Focus::new();

        assert!(focus.focus(&tree, &origin));
        assert!(focus.focus_in_direction(&tree, Direction::Down));
        assert_eq!(focus.focused(), Some(&below));

        assert!(focus.focus(&tree, &origin));
        assert!(focus.focus_in_direction(&tree, Direction::Right));
        assert_eq!(focus.focused(), Some(&right));
    }

    #[test]
    fn directional_focus_refuses_to_move_with_nothing_in_that_direction() {
        let origin = Id::unique();
        let below = Id::unique();

        let tree =
            Tree::from_forest(&[button(&origin, 100.0, 100.0), button(&below, 100.0, 200.0)]);

        let mut focus = Focus::new();
        assert!(focus.focus(&tree, &origin));

        assert!(
            !focus.focus_in_direction(&tree, Direction::Up),
            "nothing above: focus must stay put rather than wrap somewhere surprising"
        );
        assert_eq!(focus.focused(), Some(&origin));
    }

    #[test]
    fn a_scope_with_nothing_focusable_does_not_strand_focus_behind_it() {
        let outside = Id::unique();
        let empty = Id::unique();

        let tree = Tree::from_forest(&[
            button(&outside, 0.0, 0.0),
            Node::new(
                empty.clone(),
                Rectangle::with_size(Size::new(50.0, 50.0)),
                Role::Window,
            ),
        ]);

        let mut focus = Focus::new();
        assert!(focus.focus(&tree, &outside));

        focus.push_scope(
            &tree,
            Scope {
                owner: empty,
                trapped: true,
            },
        );

        assert_eq!(
            focus.focused(),
            None,
            "focus must not stay on content the trap has made unreachable"
        );
    }
}

//! What a screen reader sees.
//!
//! # Why this panel is not a tree view of the widget tree
//!
//! The semantic forest is **not** the widget tree with different labels.
//! A `column` of forty rows is forty widgets and, correctly, *nothing* to
//! a screen reader — transparent containers hoist their children rather
//! than announcing themselves. A `button("Save")` is one semantic node
//! whose name came from a `text` child two levels down.
//!
//! So this panel shows the forest as an adapter receives it, and the gap
//! between the two trees is the finding. A developer who expects their
//! dialog to be announced and sees nothing here has learned something a
//! widget inspector cannot tell them.
//!
//! # What it deliberately refuses to show
//!
//! A [`Role::SecureField`]'s value. Not because the panel filters it — the
//! panel could not filter it if it wanted to, because `Node::value()`
//! returns `None` for a secure field on the read path. See
//! `aura_core::security`. This panel inherits that and states it, so a
//! developer does not go looking for a bug in the inspector.
//!
//! [`Role::SecureField`]: aura_core::semantics::Role::SecureField

use aura::widget::{Column, column, container, row, scrollable, text};
use aura::{Element, Length};
use aura_core::security::{Audience, Privacy};
use aura_core::semantics::{Actions, Node, Role, State};

/// Renders the semantic forest.
pub fn view<'a, Message: 'a>(forest: &[Node]) -> Element<'a, Message> {
    if forest.is_empty() {
        return container(
            column![
                text("Nothing is announced.").size(16),
                text(
                    "An empty forest is not necessarily a bug \u{2014} a view of pure layout \
                     announces nothing. It is a bug if anything here is a control."
                )
                .size(12),
            ]
            .spacing(6),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into();
    }

    let mut rows = column![].spacing(2);
    let mut counts = Counts::default();

    for node in forest {
        rows = walk(node, 0, rows, &mut counts);
    }

    column![
        text(counts.describe()).size(13),
        scrollable(rows).height(Length::Fill),
    ]
    .spacing(8)
    .into()
}

/// What the forest adds up to.
#[derive(Debug, Default, PartialEq)]
struct Counts {
    nodes: usize,
    unnamed: usize,
    focusable: usize,
    actionable: usize,
}

impl Counts {
    fn record(&mut self, node: &Node) {
        self.nodes += 1;

        if names_matter(node.role) && node.name.is_none() {
            self.unnamed += 1;
        }

        if node.state.contains(State::FOCUSABLE) {
            self.focusable += 1;
        }

        if !node.actions.is_empty() {
            self.actionable += 1;
        }
    }

    fn describe(&self) -> String {
        let unnamed = if self.unnamed > 0 {
            // Called out rather than buried in a count: an unnamed control
            // announces as "button" and nothing else, which is the single
            // most common accessibility defect there is.
            format!("  \u{00b7}  {} UNNAMED", self.unnamed)
        } else {
            String::new()
        };

        format!(
            "{} nodes \u{00b7} {} focusable \u{00b7} {} actionable{unnamed}",
            self.nodes, self.focusable, self.actionable
        )
    }
}

/// Whether a node of this role is defective without a name.
///
/// A `Group` with no name is ordinary. A `Button` with no name is a
/// control a screen-reader user cannot identify.
fn names_matter(role: Role) -> bool {
    matches!(
        role,
        Role::Button
            | Role::Checkbox
            | Role::RadioButton
            | Role::Slider
            | Role::TextField
            | Role::SecureField
            | Role::Link
            | Role::Tab
            | Role::MenuItem
    )
}

fn walk<'a, Message: 'a>(
    node: &Node,
    depth: usize,
    rows: Column<'a, Message>,
    counts: &mut Counts,
) -> Column<'a, Message> {
    counts.record(node);

    let mut rows = rows.push(node_row(node, depth));

    for child in &node.children {
        rows = walk(child, depth + 1, rows, counts);
    }

    rows
}

fn node_row<'a, Message: 'a>(node: &Node, depth: usize) -> Element<'a, Message> {
    let indent = "  ".repeat(depth);

    let name = match &node.name {
        Some(name) => format!("\u{201c}{name}\u{201d}"),
        None if names_matter(node.role) => "UNNAMED".to_owned(),
        None => String::new(),
    };

    row![
        text(format!("{indent}{:?}", node.role)).size(12),
        text(name).size(12),
        text(describe_state(node.state)).size(11),
        text(describe_actions(node.actions)).size(11),
        text(describe_value(node)).size(11),
    ]
    .spacing(10)
    .into()
}

/// What a node discloses as its value.
///
/// A [`Role::SecureField`] shows `<withheld>` rather than nothing at all.
/// "Nothing" and "something you may not see" must be distinguishable, or a
/// developer goes hunting for a bug in the inspector.
fn describe_value(node: &Node) -> String {
    match node.value() {
        Some(value) => format!("{value:?}"),
        None if node.role.privacy() != Privacy::Public => {
            debug_assert!(
                !node.role.privacy().allows(Audience::Semantics),
                "a role that allows semantics should have disclosed its value"
            );

            "<withheld>".to_owned()
        }
        None => String::new(),
    }
}

fn describe_state(state: State) -> String {
    let mut flags = Vec::new();

    for (flag, name) in [
        (State::DISABLED, "disabled"),
        (State::SELECTED, "selected"),
        (State::FOCUSED, "focused"),
        (State::FOCUSABLE, "focusable"),
        (State::CHECKED, "checked"),
        (State::INDETERMINATE, "mixed"),
        (State::EXPANDED, "expanded"),
        (State::HIDDEN, "hidden"),
        (State::BUSY, "busy"),
    ] {
        if state.contains(flag) {
            flags.push(name);
        }
    }

    flags.join(",")
}

fn describe_actions(actions: Actions) -> String {
    let mut names = Vec::new();

    for (flag, name) in [
        (Actions::ACTIVATE, "activate"),
        (Actions::FOCUS, "focus"),
        (Actions::INCREMENT, "increment"),
        (Actions::DECREMENT, "decrement"),
        (Actions::SET_VALUE, "set"),
        (Actions::TOGGLE_EXPANDED, "expand"),
        (Actions::SCROLL_INTO_VIEW, "scroll"),
        (Actions::DISMISS, "dismiss"),
    ] {
        if actions.contains(flag) {
            names.push(name);
        }
    }

    names.join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use aura_core::semantics::Value;
    use aura_core::widget::Id;
    use aura_core::{Point, Rectangle, Size};

    fn node(role: Role) -> Node {
        Node::new(
            Id::unique(),
            Rectangle::new(Point::ORIGIN, Size::new(10.0, 10.0)),
            role,
        )
    }

    #[test]
    fn an_unnamed_control_is_called_out() {
        // The single most common accessibility defect there is: a control
        // that announces as "button" and nothing else.
        let mut counts = Counts::default();
        counts.record(&node(Role::Button));

        assert_eq!(counts.unnamed, 1);
        assert!(
            counts.describe().contains("UNNAMED"),
            "{}",
            counts.describe()
        );
    }

    #[test]
    fn an_unnamed_group_is_not_a_defect() {
        // A container with no name is ordinary. Flagging it would bury the
        // real finding under noise.
        let mut counts = Counts::default();
        counts.record(&node(Role::Group));

        assert_eq!(counts.unnamed, 0);
        assert!(!counts.describe().contains("UNNAMED"));
    }

    #[test]
    fn a_named_control_is_not_flagged() {
        let mut counts = Counts::default();
        counts.record(&node(Role::Button).with_name("Save"));

        assert_eq!(counts.unnamed, 0);
    }

    #[test]
    fn a_secure_fields_value_shows_as_withheld_not_as_absent() {
        // "Nothing" and "something you may not see" must be
        // distinguishable, or a developer goes hunting for a bug in the
        // inspector.
        let secure = node(Role::SecureField).with_value(Value::Text("hunter2".into()));

        assert_eq!(describe_value(&secure), "<withheld>");
    }

    #[test]
    fn the_panel_cannot_disclose_a_secret_even_if_it_tried() {
        // Not the panel's discipline -- the framework's. `Node::value()`
        // returns `None` for a secure field on the read path, so there is
        // nothing here to filter.
        let secure = node(Role::SecureField).with_value(Value::Text("hunter2".into()));

        assert!(!describe_value(&secure).contains("hunter2"));
        assert_eq!(secure.value(), None);
    }

    #[test]
    fn an_ordinary_value_is_shown() {
        let field = node(Role::TextField).with_value(Value::Text("hello".into()));

        assert!(describe_value(&field).contains("hello"));
    }

    #[test]
    fn a_node_with_nothing_to_say_says_nothing() {
        assert_eq!(describe_value(&node(Role::Group)), "");
        assert_eq!(describe_state(State::empty()), "");
        assert_eq!(describe_actions(Actions::empty()), "");
    }

    #[test]
    fn state_and_actions_are_named_not_numbered() {
        // A developer reading `State(0x18)` learns nothing.
        let state = describe_state(State::FOCUSABLE | State::CHECKED);

        assert!(state.contains("focusable"), "{state}");
        assert!(state.contains("checked"), "{state}");

        let actions = describe_actions(Actions::ACTIVATE | Actions::FOCUS);

        assert!(actions.contains("activate"), "{actions}");
    }

    #[test]
    fn counts_walk_the_whole_forest_including_children() {
        // A button's name lives on the button; its `text` child is a node
        // too, and a count that stopped at the roots would report a
        // fraction of the tree.
        let mut counts = Counts::default();
        let mut forest = node(Role::Button).with_name("Save");
        forest.children = vec![node(Role::StaticText).with_name("Save")];

        let rows: Column<'_, ()> = Column::new();
        let _ = walk(&forest, 0, rows, &mut counts);

        assert_eq!(counts.nodes, 2);
    }

    #[test]
    fn focusable_and_actionable_are_counted_separately() {
        // They are not the same thing: a disabled control is focusable in
        // some designs and actionable in none, and conflating them would
        // hide the rule that a disabled control advertises no actions.
        let mut counts = Counts::default();

        counts.record(
            &node(Role::Button)
                .with_name("Save")
                .with_state(State::FOCUSABLE)
                .with_actions(Actions::ACTIVATE),
        );
        counts.record(
            &node(Role::Button)
                .with_name("Disabled")
                .with_state(State::DISABLED),
        );

        assert_eq!(counts.focusable, 1);
        assert_eq!(counts.actionable, 1);
        assert_eq!(counts.nodes, 2);
    }
}

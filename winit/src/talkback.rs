//! The semantic tree, as AccessKit sees it: what TalkBack reads on Android.
//!
//! The conversion is pure and tested on every platform; the Android adapter
//! that publishes it lives in [`crate::android`].
//!
//! Node identities come from each node's position in the tree (the path of
//! child indices from the window), not from widget ids: some views describe
//! themselves with a fresh id every frame, and a screen reader whose focused
//! node changes id loses its place.
use crate::core::semantics::{Actions, Node, Role, State, Value};
use crate::core::{Point, Rectangle};

use accesskit::{Action, Affine, NodeId, Rect, Toggled, TreeInfo, TreeUpdate};
use rustc_hash::{FxHashMap, FxHasher};
use std::hash::{Hash, Hasher};

/// Where each published node is, in logical window coordinates, so an
/// action on it can be turned back into input.
pub(crate) type Targets = FxHashMap<NodeId, Target>;

/// What an action on a node needs to know.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Target {
    pub bounds: Rectangle,
    pub role: Role,
}

impl Target {
    pub fn center(&self) -> Point {
        self.bounds.center()
    }
}

/// The root's identity: the window node.
const ROOT: NodeId = NodeId(1);

/// The full tree for one window, with its nodes' bounds scaled from logical
/// to physical pixels, and where each node is.
pub(crate) fn tree_update(window: &Node, scale: f32) -> (TreeUpdate, Targets) {
    let mut nodes = Vec::new();
    let mut targets = Targets::default();
    let mut focus = ROOT;

    let _ = convert(
        window,
        &mut Vec::new(),
        &mut nodes,
        &mut targets,
        &mut focus,
    );

    // TAIM lays out in logical pixels; Android wants physical ones.
    if let Some((_, root)) = nodes.iter_mut().find(|(id, _)| *id == ROOT) {
        root.set_transform(Affine::scale(f64::from(scale)));
    }

    let update = TreeUpdate {
        nodes,
        tree: Some({
            let mut tree = TreeInfo::new(ROOT);
            tree.toolkit_name = Some("TAIM".into());
            tree.toolkit_version = Some(env!("CARGO_PKG_VERSION").into());
            tree
        }),
        tree_id: accesskit::TreeId::ROOT,
        focus,
    };

    (update, targets)
}

fn identity(path: &[u32]) -> NodeId {
    if path.is_empty() {
        return ROOT;
    }

    let mut hasher = FxHasher::default();
    path.hash(&mut hasher);
    // Never collide with the root's identity.
    NodeId(hasher.finish().max(2))
}

fn convert(
    node: &Node,
    path: &mut Vec<u32>,
    nodes: &mut Vec<(NodeId, accesskit::Node)>,
    targets: &mut Targets,
    focus: &mut NodeId,
) -> NodeId {
    let id = identity(path);
    let mut out = accesskit::Node::new(role(node.role));

    let bounds = node.bounds;
    out.set_bounds(Rect {
        x0: f64::from(bounds.x),
        y0: f64::from(bounds.y),
        x1: f64::from(bounds.x + bounds.width),
        y1: f64::from(bounds.y + bounds.height),
    });

    if let Some(name) = &node.name {
        out.set_label(name.as_str());
    }
    if let Some(description) = &node.description {
        out.set_description(description.as_str());
    }

    match node.value() {
        Some(Value::Text(text)) => out.set_value(text.as_str()),
        Some(Value::Number { current, min, max }) => {
            out.set_numeric_value(*current);
            out.set_min_numeric_value(*min);
            out.set_max_numeric_value(*max);
        }
        _ => {}
    }

    let state = node.state;
    if state.contains(State::DISABLED) {
        out.set_disabled();
    }
    if state.contains(State::SELECTED) {
        out.set_selected(true);
    }
    if matches!(node.role, Role::Checkbox | Role::RadioButton)
        || state.intersects(State::CHECKED | State::INDETERMINATE)
    {
        out.set_toggled(if state.contains(State::INDETERMINATE) {
            Toggled::Mixed
        } else {
            Toggled::from(state.contains(State::CHECKED))
        });
    }
    if node.actions.contains(Actions::TOGGLE_EXPANDED) {
        out.set_expanded(state.contains(State::EXPANDED));
    }
    if state.contains(State::HIDDEN) {
        out.set_hidden();
    }
    if state.contains(State::BUSY) {
        out.set_busy();
    }
    if state.contains(State::FOCUSED) {
        *focus = id;
    }

    for action in actions(node) {
        out.add_action(action);
    }

    let _ = targets.insert(
        id,
        Target {
            bounds,
            role: node.role,
        },
    );

    let mut children = Vec::with_capacity(node.children.len());
    for (index, child) in node.children.iter().enumerate() {
        path.push(u32::try_from(index).unwrap_or(u32::MAX));
        children.push(convert(child, path, nodes, targets, focus));
        let _ = path.pop();
    }
    out.set_children(children);

    nodes.push((id, out));
    id
}

/// The actions TAIM can carry out for a screen reader. Only those that
/// can be turned into input the views already understand are offered.
fn actions(node: &Node) -> Vec<Action> {
    let mut actions = Vec::new();

    if node.state.contains(State::DISABLED) {
        return actions;
    }
    if node.actions.contains(Actions::ACTIVATE) {
        actions.push(Action::Click);
    }
    if node.actions.contains(Actions::FOCUS) || node.state.contains(State::FOCUSABLE) {
        actions.push(Action::Focus);
    }
    if node.actions.contains(Actions::TOGGLE_EXPANDED) {
        actions.push(if node.state.contains(State::EXPANDED) {
            Action::Collapse
        } else {
            Action::Expand
        });
    }
    if node.role == Role::ScrollView {
        actions.push(Action::ScrollDown);
        actions.push(Action::ScrollUp);
    }

    actions
}

fn role(role: Role) -> accesskit::Role {
    use accesskit::Role as A;

    match role {
        Role::StaticText => A::Label,
        Role::Image => A::Image,
        Role::Button => A::Button,
        Role::Checkbox => A::CheckBox,
        Role::RadioButton => A::RadioButton,
        Role::Slider => A::Slider,
        Role::TextField => A::TextInput,
        Role::SecureField => A::PasswordInput,
        Role::ScrollView => A::ScrollView,
        Role::Group | Role::NamedGroup => A::Group,
        Role::Link => A::Link,
        Role::Tab => A::Tab,
        Role::MenuItem => A::MenuItem,
        Role::ListItem => A::ListItem,
        Role::Window => A::Window,
        _ => A::GenericContainer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Size, widget};

    fn node(role: Role, x: f32, y: f32) -> Node {
        Node::new(
            widget::Id::unique(),
            Rectangle::new(Point::new(x, y), Size::new(100.0, 40.0)),
            role,
        )
    }

    fn window() -> Node {
        node(Role::Window, 0.0, 0.0)
            .with_child(
                node(Role::Button, 10.0, 10.0)
                    .with_name("Save")
                    .with_actions(Actions::ACTIVATE)
                    .with_state(State::FOCUSABLE | State::FOCUSED),
            )
            .with_child(
                node(Role::Checkbox, 10.0, 60.0)
                    .with_name("Remember me")
                    .with_state(State::CHECKED)
                    .with_actions(Actions::ACTIVATE),
            )
            .with_child(
                node(Role::ScrollView, 0.0, 120.0)
                    .with_child(node(Role::StaticText, 0.0, 120.0).with_name("Row 1")),
            )
    }

    fn find<'a>(update: &'a TreeUpdate, label: &str) -> (NodeId, &'a accesskit::Node) {
        update
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some(label))
            .map(|(id, node)| (*id, node))
            .unwrap_or_else(|| panic!("a node labelled {label:?}"))
    }

    #[test]
    fn every_node_is_published_under_the_window() {
        let (update, targets) = tree_update(&window(), 2.0);

        assert_eq!(update.nodes.len(), 5);
        assert_eq!(targets.len(), 5);
        assert_eq!(update.tree.as_ref().map(|tree| tree.root), Some(ROOT));

        let root = &update
            .nodes
            .iter()
            .find(|(id, _)| *id == ROOT)
            .expect("root")
            .1;
        assert_eq!(root.role(), accesskit::Role::Window);
        assert_eq!(root.children().len(), 3);
        assert_eq!(
            root.transform(),
            Some(&Affine::scale(2.0)),
            "logical pixels are scaled to physical ones at the root"
        );
    }

    #[test]
    fn roles_names_state_and_actions_carry_over() {
        let (update, _) = tree_update(&window(), 1.0);

        let (save, button) = find(&update, "Save");
        assert_eq!(button.role(), accesskit::Role::Button);
        assert!(button.supports_action(Action::Click));
        assert!(button.supports_action(Action::Focus));
        assert_eq!(update.focus, save, "the focused view is the focus");

        let (_, checkbox) = find(&update, "Remember me");
        assert_eq!(checkbox.role(), accesskit::Role::CheckBox);
        assert_eq!(checkbox.toggled(), Some(Toggled::True));

        let scroll = update
            .nodes
            .iter()
            .find(|(_, node)| node.role() == accesskit::Role::ScrollView)
            .expect("the scroll view");
        assert!(scroll.1.supports_action(Action::ScrollDown));
    }

    #[test]
    fn identities_are_stable_across_rebuilds() {
        // `window()` makes fresh widget ids each time, as some views do
        // every frame; the published identities must not change with them.
        let (first, _) = tree_update(&window(), 1.0);
        let (second, _) = tree_update(&window(), 1.0);

        let ids = |update: &TreeUpdate| {
            let mut ids: Vec<_> = update.nodes.iter().map(|(id, _)| *id).collect();
            ids.sort();
            ids
        };
        assert_eq!(ids(&first), ids(&second));
    }

    #[test]
    fn targets_keep_logical_bounds_for_input() {
        let (update, targets) = tree_update(&window(), 3.0);
        let (save, _) = find(&update, "Save");

        assert_eq!(targets[&save].center(), Point::new(60.0, 30.0));
        assert_eq!(targets[&save].role, Role::Button);
    }

    #[test]
    fn a_disabled_control_offers_no_actions() {
        let window = node(Role::Window, 0.0, 0.0).with_child(
            node(Role::Button, 0.0, 0.0)
                .with_name("Send")
                .with_actions(Actions::ACTIVATE)
                .with_state(State::DISABLED),
        );
        let (update, _) = tree_update(&window, 1.0);
        let (_, button) = find(&update, "Send");

        assert!(!button.supports_action(Action::Click));
        assert!(button.is_disabled());
    }
}

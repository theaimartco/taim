//! Headless semantic execution engine for Taim (Pillar X).
//!
//! Enables autonomous AI agents, screen readers, and automated testing harnesses
//! to query, navigate, and drive an interactive UI without requiring a GPU, window,
//! display server, or visual rendering pipeline.

use crate::scene::{NodeId, NodeKind, SceneArena};
use crate::semantics::{Node, Role};
use crate::widget::Id;

/// A headless semantic session that drives an application directly through its semantic graph.
pub struct SemanticsDriver {
    root_node: Option<Node>,
}

impl SemanticsDriver {
    /// Builds a semantic snapshot from a [`SceneArena`].
    pub fn from_arena(arena: &SceneArena) -> Self {
        let root = arena.root();
        let root_node = if root.is_some() {
            Self::build_node(arena, root)
        } else {
            None
        };

        Self { root_node }
    }

    fn build_node(arena: &SceneArena, id: NodeId) -> Option<Node> {
        let scene_node = arena.get(id)?;
        let world_bounds = scene_node.local_bounds * scene_node.world_transform;

        let (role, name) = match &scene_node.kind {
            NodeKind::Text { content, .. } => (Role::StaticText, Some(content.as_str())),
            NodeKind::Quad { .. } => (Role::Button, None),
            NodeKind::Container { .. } => (Role::Group, None),
            NodeKind::Custom { .. } => (Role::Group, None),
        };

        let mut node = Node::new(Id::unique(), world_bounds, role);
        if let Some(label) = name {
            node = node.with_name(label);
        }

        let mut child = scene_node.first_child;
        while child.is_some() {
            if let Some(child_node) = Self::build_node(arena, child) {
                node = node.with_child(child_node);
            }
            if let Some(curr) = arena.get(child) {
                child = curr.next_sibling;
            } else {
                break;
            }
        }

        Some(node)
    }

    /// Accesses the root semantic node.
    pub fn root(&self) -> Option<&Node> {
        self.root_node.as_ref()
    }

    /// Searches the semantic tree for the first node matching `label` (case-insensitive).
    pub fn find_by_name(&self, label: &str) -> Option<&Node> {
        self.root_node.as_ref().and_then(|root| Self::find_in_subtree(root, |n| {
            n.name.as_ref().is_some_and(|name| name.eq_ignore_ascii_case(label))
        }))
    }

    /// Finds all nodes matching a specific semantic [`Role`].
    pub fn find_by_role(&self, role: Role) -> Vec<&Node> {
        let mut results = Vec::new();
        if let Some(root) = &self.root_node {
            Self::collect_by_role(root, role, &mut results);
        }
        results
    }

    fn find_in_subtree<F>(node: &Node, predicate: F) -> Option<&Node>
    where
        F: Fn(&Node) -> bool + Copy,
    {
        if predicate(node) {
            return Some(node);
        }
        for child in &node.children {
            if let Some(hit) = Self::find_in_subtree(child, predicate) {
                return Some(hit);
            }
        }
        None
    }

    fn collect_by_role<'a>(node: &'a Node, role: Role, results: &mut Vec<&'a Node>) {
        if node.role == role {
            results.push(node);
        }
        for child in &node.children {
            Self::collect_by_role(child, role, results);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{FlexDirection, NodeKind, SceneArena};
    use crate::Color;

    #[test]
    fn semantics_driver_inspects_and_queries_tree() {
        let mut arena = SceneArena::new();
        let root = arena.alloc(NodeKind::Container {
            direction: FlexDirection::Column,
            align_items: crate::scene::CrossAxisAlignment::Start,
            gap: 8.0,
        });
        let label = arena.alloc(NodeKind::Text {
            content: "Login Button".into(),
            size: 16.0,
            color: Color::WHITE,
        });
        arena.append_child(root, label);

        let driver = SemanticsDriver::from_arena(&arena);
        assert!(driver.root().is_some());

        let found = driver.find_by_name("login button");
        assert!(found.is_some());
        assert_eq!(found.unwrap().role, Role::StaticText);

        let text_nodes = driver.find_by_role(Role::StaticText);
        assert_eq!(text_nodes.len(), 1);
    }
}

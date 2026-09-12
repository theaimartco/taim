//! Flat, data-oriented retained scene graph for Taim.
//!
//! Unlike legacy widget trees that rebuild virtual structures every tick and box dynamic
//! handlers (`Box<dyn Widget>`), [`SceneArena`] stores UI nodes in a contiguous, cache-local
//! slab indexed by [`NodeId`]. Reactive signals (`taim-signal`) bind directly to node attributes,
//! performing fine-grained dirty invalidation in O(1) time without tree walking.

use crate::{Color, Point, Rectangle, Transformation};
use taim_signal::{DirtyFlags, Revision};

/// A stable 32-bit handle to a scene node in [`SceneArena`].
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub u32);

impl Default for NodeId {
    fn default() -> Self {
        Self::NONE
    }
}

impl NodeId {
    /// Sentinel indicating no node or the root parent.
    pub const NONE: Self = Self(u32::MAX);

    /// Checks if this handle is valid (not sentinel).
    pub const fn is_some(self) -> bool {
        self.0 != u32::MAX
    }

    /// Checks if this handle is the sentinel.
    pub const fn is_none(self) -> bool {
        self.0 == u32::MAX
    }

    /// Raw index inside the scene arena.
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// The layout direction for container nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexDirection {
    /// Items laid out horizontally from left to right.
    #[default]
    Row,
    /// Items laid out vertically from top to bottom.
    Column,
}

/// Alignment along the cross axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrossAxisAlignment {
    /// Stretch to fit parent cross-axis size.
    #[default]
    Stretch,
    /// Align to the start of the cross-axis.
    Start,
    /// Align to the center of the cross-axis.
    Center,
    /// Align to the end of the cross-axis.
    End,
}

/// Sizing constraint mode for a layout node.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum NodeDimension {
    /// Fixed size in logical pixels.
    Exact(f32),
    /// Shrink to fit content children.
    #[default]
    Shrink,
    /// Fill available space with a flex weight.
    Fill(f32),
}

/// The specific payload and behavior of a scene node.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    /// Flex container (Row, Column, or Stack).
    Container {
        /// Layout direction.
        direction: FlexDirection,
        /// Alignment along the cross axis.
        align_items: CrossAxisAlignment,
        /// Spacing between children in pixels.
        gap: f32,
    },
    /// Solid or rounded rectangle shape.
    Quad {
        /// Fill background color.
        background: Color,
        /// Corner radius for all 4 corners.
        border_radius: [f32; 4],
        /// Border width in pixels.
        border_width: f32,
        /// Border color.
        border_color: Color,
    },
    /// Text content node.
    Text {
        /// The raw string content.
        content: String,
        /// Font size in logical pixels.
        size: f32,
        /// Text fill color.
        color: Color,
    },
    /// Custom leaf node (image, canvas, custom shader).
    Custom {
        /// Application or subsystem material identifier.
        material_id: u32,
    },
}

/// One node stored inside the contiguous [`SceneArena`].
#[derive(Debug, Clone, PartialEq)]
pub struct SceneNode {
    /// Parent node handle, or [`NodeId::NONE`].
    pub parent: NodeId,
    /// First child node handle, or [`NodeId::NONE`].
    pub first_child: NodeId,
    /// Next sibling handle, or [`NodeId::NONE`].
    pub next_sibling: NodeId,
    /// Previous sibling handle, or [`NodeId::NONE`].
    pub prev_sibling: NodeId,

    /// Fine-grained dirty flags.
    pub dirty: DirtyFlags,
    /// Monotonically increasing revision for caching.
    pub revision: Revision,

    /// Width sizing policy.
    pub width: NodeDimension,
    /// Height sizing policy.
    pub height: NodeDimension,
    /// Padding [top, right, bottom, left].
    pub padding: [f32; 4],

    /// Computed local bounds after layout resolution.
    pub local_bounds: Rectangle,
    /// Local transform matrix.
    pub local_transform: Transformation,
    /// Resolved world transform (parent_world * local).
    pub world_transform: Transformation,
    /// Optional clipping boundary in scene coordinates.
    pub clip_bounds: Option<Rectangle>,

    /// Node kind and visual properties.
    pub kind: NodeKind,
}

impl SceneNode {
    fn new(kind: NodeKind) -> Self {
        Self {
            parent: NodeId::NONE,
            first_child: NodeId::NONE,
            next_sibling: NodeId::NONE,
            prev_sibling: NodeId::NONE,
            dirty: DirtyFlags::ALL,
            revision: Revision::INITIAL,
            width: NodeDimension::Shrink,
            height: NodeDimension::Shrink,
            padding: [0.0; 4],
            local_bounds: Rectangle::default(),
            local_transform: Transformation::IDENTITY,
            world_transform: Transformation::IDENTITY,
            clip_bounds: None,
            kind,
        }
    }
}

/// Contiguous, zero-copy retained scene graph.
#[derive(Debug, Default)]
pub struct SceneArena {
    nodes: Vec<SceneNode>,
    free_list: Vec<NodeId>,
    root: NodeId,
    dirty_nodes: Vec<NodeId>,
    cached_raster: Option<taim_raster::Scene>,
}

impl SceneArena {
    /// Creates a new, empty scene arena.
    pub fn new() -> Self {
        Self {
            nodes: Vec::with_capacity(64),
            free_list: Vec::new(),
            root: NodeId::NONE,
            dirty_nodes: Vec::new(),
            cached_raster: None,
        }
    }

    /// Allocates a new node in the arena with the given kind.
    pub fn alloc(&mut self, kind: NodeKind) -> NodeId {
        let node = SceneNode::new(kind);
        let id = if let Some(free_id) = self.free_list.pop() {
            self.nodes[free_id.index()] = node;
            free_id
        } else {
            let id = NodeId(self.nodes.len() as u32);
            self.nodes.push(node);
            id
        };

        if self.root.is_none() {
            self.root = id;
        }

        self.dirty_nodes.push(id);
        id
    }

    /// Sets the root node of the scene graph.
    pub fn set_root(&mut self, id: NodeId) {
        self.root = id;
    }

    /// Returns the root node of the scene graph.
    pub const fn root(&self) -> NodeId {
        self.root
    }

    /// Appends a child to `parent` as its last child.
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        assert!(parent.is_some() && parent.index() < self.nodes.len());
        assert!(child.is_some() && child.index() < self.nodes.len());
        assert_ne!(parent, child);

        // Detach child from existing parent if any
        self.detach(child);

        let parent_first = self.nodes[parent.index()].first_child;
        if parent_first.is_none() {
            self.nodes[parent.index()].first_child = child;
            self.nodes[child.index()].parent = parent;
            self.nodes[child.index()].prev_sibling = NodeId::NONE;
            self.nodes[child.index()].next_sibling = NodeId::NONE;
        } else {
            // Find last child
            let mut curr = parent_first;
            while self.nodes[curr.index()].next_sibling.is_some() {
                curr = self.nodes[curr.index()].next_sibling;
            }
            self.nodes[curr.index()].next_sibling = child;
            self.nodes[child.index()].prev_sibling = curr;
            self.nodes[child.index()].next_sibling = NodeId::NONE;
            self.nodes[child.index()].parent = parent;
        }

        self.mark_dirty(parent, DirtyFlags::LAYOUT);
    }

    /// Detaches a node from its parent and sibling chain.
    pub fn detach(&mut self, node: NodeId) {
        if node.is_none() || node.index() >= self.nodes.len() {
            return;
        }

        let parent = self.nodes[node.index()].parent;
        let prev = self.nodes[node.index()].prev_sibling;
        let next = self.nodes[node.index()].next_sibling;

        if prev.is_some() {
            self.nodes[prev.index()].next_sibling = next;
        } else if parent.is_some() && self.nodes[parent.index()].first_child == node {
            self.nodes[parent.index()].first_child = next;
        }

        if next.is_some() {
            self.nodes[next.index()].prev_sibling = prev;
        }

        self.nodes[node.index()].parent = NodeId::NONE;
        self.nodes[node.index()].prev_sibling = NodeId::NONE;
        self.nodes[node.index()].next_sibling = NodeId::NONE;

        if parent.is_some() {
            self.mark_dirty(parent, DirtyFlags::LAYOUT);
        }
    }

    /// Borrows a scene node immutably.
    pub fn get(&self, id: NodeId) -> Option<&SceneNode> {
        if id.is_some() && id.index() < self.nodes.len() {
            Some(&self.nodes[id.index()])
        } else {
            None
        }
    }

    /// Borrows a scene node mutably.
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut SceneNode> {
        if id.is_some() && id.index() < self.nodes.len() {
            Some(&mut self.nodes[id.index()])
        } else {
            None
        }
    }

    /// Marks a node with specific dirty flags and schedules it for invalidation.
    pub fn mark_dirty(&mut self, id: NodeId, flags: DirtyFlags) {
        if id.is_none() || id.index() >= self.nodes.len() {
            return;
        }

        self.cached_raster = None;

        let node = &mut self.nodes[id.index()];
        let was_clean = node.dirty.is_empty();
        node.dirty.insert(flags);

        if was_clean {
            self.dirty_nodes.push(id);
        }

        // If layout or transform changed, propagate upwards to parent
        if flags.intersects(DirtyFlags::LAYOUT) {
            let parent = node.parent;
            if parent.is_some() {
                self.mark_dirty(parent, DirtyFlags::LAYOUT);
            }
        }
    }

    /// Clears the dirty flags for all dirty nodes and drains the dirty queue.
    pub fn clear_dirty(&mut self) {
        for &id in &self.dirty_nodes {
            if id.index() < self.nodes.len() {
                self.nodes[id.index()].dirty = DirtyFlags::NONE;
            }
        }
        self.dirty_nodes.clear();
    }

    /// Checks if there are any pending dirty nodes in the scene.
    pub fn is_dirty(&self) -> bool {
        !self.dirty_nodes.is_empty()
    }

    /// Returns the slice of nodes currently marked dirty.
    pub fn dirty_nodes(&self) -> &[NodeId] {
        &self.dirty_nodes
    }

    /// Computes the minimal bounding rectangle enclosing all currently dirty nodes.
    pub fn damage_bounds(&self) -> Option<Rectangle> {
        if self.dirty_nodes.is_empty() {
            return None;
        }

        let mut damage: Option<Rectangle> = None;
        for &id in &self.dirty_nodes {
            if let Some(node) = self.get(id) {
                let world_rect = node.local_bounds * node.world_transform;
                if world_rect.width > 0.0 && world_rect.height > 0.0 {
                    damage = match damage {
                        Some(d) => Some(d.union(&world_rect)),
                        None => Some(world_rect),
                    };
                }
            }
        }
        damage
    }

    /// Returns the number of allocated nodes.
    pub fn len(&self) -> usize {
        self.nodes.len() - self.free_list.len()
    }

    /// Checks if the arena has no active nodes.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Propagates world transforms from the root down to all descendants.
    pub fn update_world_transforms(&mut self) {
        if self.root.is_none() {
            return;
        }
        self.propagate_transform(self.root, Transformation::IDENTITY);
    }

    fn propagate_transform(&mut self, current: NodeId, parent_world: Transformation) {
        let node = &mut self.nodes[current.index()];
        node.world_transform = parent_world * node.local_transform;
        let world = node.world_transform;

        let mut child = node.first_child;
        while child.is_some() {
            self.propagate_transform(child, world);
            child = self.nodes[child.index()].next_sibling;
        }
    }

    /// Encodes the visible, transformed scene nodes into a [`taim_raster::Scene`].
    pub fn encode_to_raster(&self) -> Result<taim_raster::Scene, taim_raster::EncodeError> {
        let mut encoder = taim_raster::Encoder::new();
        if self.root.is_some() {
            self.encode_node(self.root, &mut encoder, None)?;
        }
        Ok(encoder.finish())
    }

    /// Encodes the scene nodes into a [`taim_raster::Scene`], returning a cached copy
    /// when clean to achieve zero steady-state encoding overhead.
    pub fn encode_cached(&mut self) -> Result<&taim_raster::Scene, taim_raster::EncodeError> {
        if self.cached_raster.is_none() || !self.dirty_nodes.is_empty() {
            let scene = self.encode_to_raster()?;
            self.cached_raster = Some(scene);
        }
        Ok(self.cached_raster.as_ref().unwrap())
    }

    fn encode_node(
        &self,
        current: NodeId,
        encoder: &mut taim_raster::Encoder,
        active_clip: Option<taim_raster::ClipId>,
    ) -> Result<(), taim_raster::EncodeError> {
        let node = &self.nodes[current.index()];
        let world_rect = node.local_bounds * node.world_transform;

        let mut next_clip = active_clip;
        if let Some(clip) = node.clip_bounds {
            let world_clip = clip * node.world_transform;
            let points = [
                [world_clip.x, world_clip.y],
                [world_clip.x + world_clip.width, world_clip.y],
                [
                    world_clip.x + world_clip.width,
                    world_clip.y + world_clip.height,
                ],
                [world_clip.x, world_clip.y + world_clip.height],
            ];
            let clip_path = encoder.push_path(&points, true)?;
            next_clip = Some(encoder.push_clip(clip_path, active_clip)?);
        }

        if world_rect.width > 0.0 && world_rect.height > 0.0 {
            let points = [
                [world_rect.x, world_rect.y],
                [world_rect.x + world_rect.width, world_rect.y],
                [
                    world_rect.x + world_rect.width,
                    world_rect.y + world_rect.height,
                ],
                [world_rect.x, world_rect.y + world_rect.height],
            ];
            let mat_id = match &node.kind {
                NodeKind::Quad { .. } | NodeKind::Container { .. } => 0,
                NodeKind::Text { .. } => 1,
                NodeKind::Custom { material_id } => *material_id,
            };
            let path = encoder.push_path(&points, true)?;
            encoder.push_draw(path, next_clip, taim_raster::MaterialId(mat_id), 1.0)?;
        }

        let mut child = node.first_child;
        while child.is_some() {
            self.encode_node(child, encoder, next_clip)?;
            child = self.nodes[child.index()].next_sibling;
        }

        Ok(())
    }

    /// Performs spatial hit testing against the scene graph, returning the topmost node under `point`.
    pub fn hit_test(&self, point: Point) -> Option<NodeId> {
        if self.root.is_none() {
            return None;
        }
        self.hit_test_node(self.root, point)
    }

    fn hit_test_node(&self, current: NodeId, point: Point) -> Option<NodeId> {
        let node = &self.nodes[current.index()];

        // Check clip bounds if present
        if let Some(clip) = node.clip_bounds {
            let world_clip = clip * node.world_transform;
            if !world_clip.contains(point) {
                return None;
            }
        }

        // Find last child to test topmost/frontmost children first (reverse paint order)
        let mut last_child = NodeId::NONE;
        let mut curr = node.first_child;
        while curr.is_some() {
            last_child = curr;
            curr = self.nodes[curr.index()].next_sibling;
        }

        // Walk backwards through siblings
        let mut child = last_child;
        while child.is_some() {
            if let Some(hit) = self.hit_test_node(child, point) {
                return Some(hit);
            }
            child = self.nodes[child.index()].prev_sibling;
        }

        // Test this node's bounds
        let world_rect = node.local_bounds * node.world_transform;
        if world_rect.contains(point) {
            Some(current)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Point, Size};

    #[test]
    fn arena_alloc_and_hierarchy() {
        let mut arena = SceneArena::new();
        let root = arena.alloc(NodeKind::Container {
            direction: FlexDirection::Column,
            align_items: CrossAxisAlignment::Stretch,
            gap: 8.0,
        });

        let child1 = arena.alloc(NodeKind::Quad {
            background: Color::BLACK,
            border_radius: [0.0; 4],
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        });

        let child2 = arena.alloc(NodeKind::Text {
            content: "Hello Taim".into(),
            size: 16.0,
            color: Color::WHITE,
        });

        arena.append_child(root, child1);
        arena.append_child(root, child2);

        assert_eq!(arena.root(), root);
        assert_eq!(arena.get(root).unwrap().first_child, child1);
        assert_eq!(arena.get(child1).unwrap().next_sibling, child2);
        assert_eq!(arena.get(child2).unwrap().prev_sibling, child1);
        assert_eq!(arena.get(child2).unwrap().parent, root);

        arena.detach(child1);
        assert_eq!(arena.get(root).unwrap().first_child, child2);
        assert_eq!(arena.get(child2).unwrap().prev_sibling, NodeId::NONE);
    }

    #[test]
    fn dirty_flags_propagation() {
        let mut arena = SceneArena::new();
        let root = arena.alloc(NodeKind::Container {
            direction: FlexDirection::Row,
            align_items: CrossAxisAlignment::Center,
            gap: 4.0,
        });
        let child = arena.alloc(NodeKind::Quad {
            background: Color::WHITE,
            border_radius: [4.0; 4],
            border_width: 1.0,
            border_color: Color::BLACK,
        });
        arena.append_child(root, child);
        arena.clear_dirty();

        assert_eq!(arena.dirty_nodes.len(), 0);

        arena.mark_dirty(child, DirtyFlags::LAYOUT);
        assert!(
            arena
                .get(child)
                .unwrap()
                .dirty
                .intersects(DirtyFlags::LAYOUT)
        );
        assert!(
            arena
                .get(root)
                .unwrap()
                .dirty
                .intersects(DirtyFlags::LAYOUT)
        );
    }

    #[test]
    fn encode_to_raster_scene() {
        let mut arena = SceneArena::new();
        let root = arena.alloc(NodeKind::Container {
            direction: FlexDirection::Column,
            align_items: CrossAxisAlignment::Stretch,
            gap: 0.0,
        });
        let child = arena.alloc(NodeKind::Quad {
            background: Color::WHITE,
            border_radius: [0.0; 4],
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        });
        arena.append_child(root, child);

        if let Some(c) = arena.get_mut(child) {
            c.local_bounds = Rectangle::new(Point::new(10.0, 10.0), Size::new(100.0, 50.0));
        }
        arena.update_world_transforms();

        let raster = arena
            .encode_to_raster()
            .expect("raster encoding must succeed");

        assert_eq!(raster.draws().len(), 1);
        assert_eq!(raster.paths().len(), 1);
    }

    #[test]
    fn spatial_hit_testing() {
        let mut arena = SceneArena::new();
        let root = arena.alloc(NodeKind::Container {
            direction: FlexDirection::Column,
            align_items: CrossAxisAlignment::Stretch,
            gap: 0.0,
        });
        let child1 = arena.alloc(NodeKind::Quad {
            background: Color::WHITE,
            border_radius: [0.0; 4],
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        });
        let child2 = arena.alloc(NodeKind::Quad {
            background: Color::BLACK,
            border_radius: [0.0; 4],
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        });
        arena.append_child(root, child1);
        arena.append_child(root, child2);

        if let Some(c) = arena.get_mut(child1) {
            c.local_bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        }
        if let Some(c) = arena.get_mut(child2) {
            // child2 overlaps child1 at (50, 50)
            c.local_bounds = Rectangle::new(Point::new(50.0, 50.0), Size::new(100.0, 100.0));
        }
        arena.update_world_transforms();

        // Point only in child1
        assert_eq!(arena.hit_test(Point::new(10.0, 10.0)), Some(child1));
        // Point in overlapping region (50..100, 50..100) should hit child2 because it's frontmost
        assert_eq!(arena.hit_test(Point::new(60.0, 60.0)), Some(child2));
        // Point outside both
        assert_eq!(arena.hit_test(Point::new(200.0, 200.0)), None);
    }

    #[test]
    fn encode_cached_reuses_clean_raster_scene() {
        let mut arena = SceneArena::new();
        let root = arena.alloc(NodeKind::Quad {
            background: Color::BLACK,
            border_radius: [0.0; 4],
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        });
        if let Some(c) = arena.get_mut(root) {
            c.local_bounds = Rectangle::new(Point::ORIGIN, Size::new(50.0, 50.0));
        }
        arena.clear_dirty();

        let raster1_ptr = {
            let raster = arena.encode_cached().expect("must encode");
            raster as *const _
        };
        let raster2_ptr = {
            let raster = arena.encode_cached().expect("must reuse cached");
            raster as *const _
        };
        assert_eq!(raster1_ptr, raster2_ptr);

        // Mutating dirty flags invalidates the cache
        arena.mark_dirty(root, DirtyFlags::RENDER);
        assert!(arena.cached_raster.is_none());
    }
}

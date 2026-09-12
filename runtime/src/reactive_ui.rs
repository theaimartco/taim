//! Retained Reactive User Interface for Taim.
//!
//! Unlike legacy virtual-tree architectures that rebuild dynamic elements and dynamic closures
//! every tick, [`ReactiveUi`] maintains a permanent, data-oriented [`SceneArena`].
//! Fine-grained state mutations directly invalidate dirty flags on affected nodes,
//! running the sub-microsecond layout solver only when needed and streaming POD scene
//! primitives directly to the GPU rasterizer (`taim-raster`).

use crate::core::Size;
use crate::core::event::Event;
use crate::core::layout_solver::solve_layout;
use crate::core::mouse;
use crate::core::scene::{NodeId, SceneArena};
use crate::core::touch;
use rustc_hash::FxHashMap;

/// Callback invoked on user interaction with a scene node.
pub type EventHandler = Box<dyn Fn(&mut SceneArena, &Event) + Send + Sync>;

/// A retained, zero-rebuild reactive UI session.
pub struct ReactiveUi {
    arena: SceneArena,
    event_handlers: FxHashMap<NodeId, EventHandler>,
    bounds: Size,
    hovered_node: Option<NodeId>,
}

impl Default for ReactiveUi {
    fn default() -> Self {
        Self::new(SceneArena::new())
    }
}

impl ReactiveUi {
    /// Creates a new reactive UI session hosting the given [`SceneArena`].
    pub fn new(arena: SceneArena) -> Self {
        Self {
            arena,
            event_handlers: FxHashMap::default(),
            bounds: Size::ZERO,
            hovered_node: None,
        }
    }

    /// Mounts an application scene using a construction closure.
    pub fn mount<F>(f: F) -> Self
    where
        F: FnOnce(&mut SceneArena, &mut FxHashMap<NodeId, EventHandler>) -> NodeId,
    {
        let mut arena = SceneArena::new();
        let mut handlers = FxHashMap::default();
        let root = f(&mut arena, &mut handlers);
        arena.set_root(root);

        Self {
            arena,
            event_handlers: handlers,
            bounds: Size::ZERO,
            hovered_node: None,
        }
    }

    /// Accesses the underlying retained [`SceneArena`].
    pub fn arena(&self) -> &SceneArena {
        &self.arena
    }

    /// Mutably accesses the underlying retained [`SceneArena`].
    pub fn arena_mut(&mut self) -> &mut SceneArena {
        &mut self.arena
    }

    /// Registers an event handler for a specific node.
    pub fn on_event(
        &mut self,
        node: NodeId,
        handler: impl Fn(&mut SceneArena, &Event) + Send + Sync + 'static,
    ) {
        let _ = self.event_handlers.insert(node, Box::new(handler));
    }

    /// Checks if any node in the scene graph has dirty flags set.
    pub fn is_dirty(&self) -> bool {
        self.arena.is_dirty()
    }

    /// Solves layout for the scene graph within `bounds`.
    ///
    /// Skips clean subtrees in O(1) time. Updates world transforms automatically.
    pub fn layout(&mut self, bounds: Size) {
        self.bounds = bounds;
        solve_layout(&mut self.arena, bounds);
    }

    /// Encodes the retained scene directly into a [`taim_raster::Scene`].
    pub fn render(&self) -> Result<taim_raster::Scene, taim_raster::EncodeError> {
        self.arena.encode_to_raster()
    }

    /// Encodes the scene returning a cached copy when clean, yielding zero-cost steady frames.
    pub fn render_cached(&mut self) -> Result<&taim_raster::Scene, taim_raster::EncodeError> {
        self.arena.encode_cached()
    }

    /// Computes the minimal bounding rectangle enclosing all dirty nodes for direct scanout.
    pub fn damage_region(&self) -> Option<crate::core::Rectangle> {
        self.arena.damage_bounds()
    }

    /// Clears all dirty flags across the scene graph after layout and rendering.
    pub fn clear_dirty(&mut self) {
        self.arena.clear_dirty();
    }

    /// Dispatches an event and computes resulting damage bounds for sub-4ms direct scanout.
    pub fn dispatch_event_with_damage(
        &mut self,
        event: &Event,
    ) -> (bool, Option<crate::core::Rectangle>) {
        let handled = self.dispatch_event(event);
        let damage = if handled {
            self.arena.damage_bounds()
        } else {
            None
        };
        (handled, damage)
    }

    /// Dispatches a system [`Event`] to the hit node, executing any attached handlers.
    ///
    /// Returns `true` if an event was handled and consumed.
    pub fn dispatch_event(&mut self, event: &Event) -> bool {
        let cursor_point = match event {
            Event::Mouse(mouse::Event::CursorMoved { position }) => Some(*position),
            Event::Touch(touch::Event::FingerPressed { position, .. }) => Some(*position),
            Event::Touch(touch::Event::FingerMoved { position, .. }) => Some(*position),
            _ => None,
        };

        if let Some(point) = cursor_point {
            let hit = self.arena.hit_test(point);

            if let Event::Mouse(mouse::Event::CursorMoved { .. }) = event {
                self.hovered_node = hit;
            }

            if let Some(node_id) = hit
                && let Some(handler) = self.event_handlers.get(&node_id)
            {
                handler(&mut self.arena, event);
                return true;
            }
        }

        false
    }

    /// Returns the currently hovered node handle, if any.
    pub fn hovered_node(&self) -> Option<NodeId> {
        self.hovered_node
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::scene::{CrossAxisAlignment, FlexDirection, NodeDimension, NodeKind};
    use crate::core::{Color, Point, Rectangle};
    use taim_signal::Signal;

    #[test]
    fn reactive_ui_mount_and_layout() {
        let mut ui = ReactiveUi::mount(|arena, _handlers| {
            let root = arena.alloc(NodeKind::Container {
                direction: FlexDirection::Column,
                align_items: CrossAxisAlignment::Start,
                gap: 10.0,
            });
            let text = arena.alloc(NodeKind::Text {
                content: "Reactive Taim".into(),
                size: 18.0,
                color: Color::WHITE,
            });
            arena.append_child(root, text);
            root
        });

        assert!(ui.is_dirty());
        ui.layout(Size::new(800.0, 600.0));
        assert!(ui.arena().get(ui.arena().root()).is_some());

        let raster = ui.render().expect("raster rendering must succeed");
        assert_eq!(raster.draws().len(), 2);

        ui.clear_dirty();
        assert!(!ui.is_dirty());
    }

    #[test]
    fn reactive_ui_signal_mutation() {
        let count = Signal::new(0);
        let count_clone = count.clone();

        let mut ui = ReactiveUi::mount(move |arena, handlers| {
            let button = arena.alloc(NodeKind::Quad {
                background: Color::BLACK,
                border_radius: [4.0; 4],
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
            });
            if let Some(n) = arena.get_mut(button) {
                n.local_bounds = Rectangle::new(Point::ORIGIN, Size::new(100.0, 50.0));
                n.width = NodeDimension::Exact(100.0);
                n.height = NodeDimension::Exact(50.0);
            }

            let _ = handlers.insert(
                button,
                Box::new(move |_arena, _event| {
                    count_clone.update(|c| *c += 1);
                }),
            );

            button
        });

        ui.layout(Size::new(500.0, 500.0));
        assert_eq!(count.get(), 0);

        // Click on the button at (25, 25)
        let click_event = Event::Touch(touch::Event::FingerPressed {
            id: touch::Finger(0),
            position: Point::new(25.0, 25.0),
            metadata: crate::core::pointer::Metadata::NONE,
        });
        let handled = ui.dispatch_event(&click_event);
        assert!(handled);
        assert_eq!(count.get(), 1);
    }
}

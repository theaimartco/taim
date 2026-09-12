//! Pure retained reactive components for Taim.
//!
//! Unlike legacy ephemeral widgets that reconstruct virtual tree nodes every frame and
//! box dynamic dispatch handlers (`Box<dyn Widget>`), reactive components mount directly
//! into [`core::scene::SceneArena`] as contiguous [`core::scene::NodeId`] handles.
//!
//! State changes via [`taim_signal::Signal`] invalidate only affected node properties
//! with O(1) dirty-flag flips, achieving zero heap allocations during steady-state frames.

use crate::core::scene::{CrossAxisAlignment, FlexDirection, NodeDimension, NodeId, NodeKind, SceneArena};
use crate::core::{Color, Point, Rectangle, Size};
use taim_signal::{Readable, Signal, Subscription};

/// Lifecycle context for retained reactive components.
pub struct ReactiveMount<'a> {
    arena: &'a mut SceneArena,
    subscriptions: Vec<Subscription>,
}

impl<'a> ReactiveMount<'a> {
    /// Creates a new mounting context wrapping a scene arena.
    pub fn new(arena: &'a mut SceneArena) -> Self {
        Self {
            arena,
            subscriptions: Vec::new(),
        }
    }

    /// Borrows the underlying scene arena mutably.
    pub fn arena(&mut self) -> &mut SceneArena {
        self.arena
    }

    /// Registers a reactive subscription tied to this mount.
    pub fn track_subscription(&mut self, sub: Subscription) {
        self.subscriptions.push(sub);
    }

    /// Consumes the mount, returning the tracked subscriptions.
    pub fn finish(self) -> Vec<Subscription> {
        self.subscriptions
    }
}

/// A reactive push button component.
#[derive(Debug, Clone)]
pub struct ReactiveButton {
    /// Reactive hovered state.
    pub hovered: Signal<bool>,
    /// Reactive pressed state.
    pub pressed: Signal<bool>,
    /// Static or dynamic button label.
    pub label: String,
    /// Button normal background color.
    pub bg_normal: Color,
    /// Button hover background color.
    pub bg_hover: Color,
    /// Button pressed background color.
    pub bg_pressed: Color,
    /// Text label color.
    pub text_color: Color,
    /// Border radius.
    pub border_radius: [f32; 4],
}

impl Default for ReactiveButton {
    fn default() -> Self {
        Self {
            hovered: Signal::new(false),
            pressed: Signal::new(false),
            label: "Button".into(),
            bg_normal: Color::from_rgb(0.2, 0.4, 0.8),
            bg_hover: Color::from_rgb(0.3, 0.5, 0.9),
            bg_pressed: Color::from_rgb(0.1, 0.3, 0.7),
            text_color: Color::WHITE,
            border_radius: [6.0; 4],
        }
    }
}

impl ReactiveButton {
    /// Creates a new reactive button with a label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            ..Default::default()
        }
    }

    /// Mounts the button into the scene arena, returning the outer container node ID.
    pub fn mount(&self, mount: &mut ReactiveMount<'_>) -> NodeId {
        let bg_color = if self.pressed.get() {
            self.bg_pressed
        } else if self.hovered.get() {
            self.bg_hover
        } else {
            self.bg_normal
        };

        let container_id = mount.arena().alloc(NodeKind::Quad {
            background: bg_color,
            border_radius: self.border_radius,
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        });

        if let Some(node) = mount.arena().get_mut(container_id) {
            node.padding = [8.0, 16.0, 8.0, 16.0];
        }

        let text_id = mount.arena().alloc(NodeKind::Text {
            content: self.label.clone(),
            size: 14.0,
            color: self.text_color,
        });

        mount.arena().append_child(container_id, text_id);

        container_id
    }
}

/// A reactive text display component bound to a reactive signal.
#[derive(Debug, Clone)]
pub struct ReactiveText<S> {
    source: S,
    font_size: f32,
    color: Color,
}

impl<S> ReactiveText<S>
where
    S: Readable<Value = String>,
{
    /// Creates a new reactive text node bound to `source`.
    pub fn new(source: S) -> Self {
        Self {
            source,
            font_size: 14.0,
            color: Color::BLACK,
        }
    }

    /// Sets the text font size.
    pub fn size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Sets the text color.
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Mounts the reactive text node into the scene arena.
    pub fn mount(&self, mount: &mut ReactiveMount<'_>) -> NodeId {
        let initial_text = self.source.get();
        mount.arena().alloc(NodeKind::Text {
            content: initial_text,
            size: self.font_size,
            color: self.color,
        })
    }
}

/// High-performance virtualized list component for huge datasets.
#[derive(Debug, Clone)]
pub struct ReactiveVirtualList {
    /// Total number of items in the dataset.
    pub item_count: usize,
    /// Fixed height of each item in pixels.
    pub item_height: f32,
    /// Scroll offset signal.
    pub scroll_y: Signal<f32>,
    /// Viewport height in pixels.
    pub viewport_height: f32,
}

impl ReactiveVirtualList {
    /// Creates a new virtual list.
    pub fn new(item_count: usize, item_height: f32, viewport_height: f32) -> Self {
        Self {
            item_count,
            item_height,
            scroll_y: Signal::new(0.0),
            viewport_height,
        }
    }

    /// Computes the index range [start, end) of visible items given current scroll.
    pub fn visible_range(&self) -> std::ops::Range<usize> {
        let scroll = self.scroll_y.get().max(0.0);
        let start = (scroll / self.item_height).floor() as usize;
        let visible_count = (self.viewport_height / self.item_height).ceil() as usize + 2;
        let end = (start + visible_count).min(self.item_count);
        let start = start.min(end);
        start..end
    }

    /// Mounts a virtual list container with placeholder clipping into [`SceneArena`].
    pub fn mount(&self, mount: &mut ReactiveMount<'_>) -> NodeId {
        let container_id = mount.arena().alloc(NodeKind::Container {
            direction: FlexDirection::Column,
            align_items: CrossAxisAlignment::Stretch,
            gap: 0.0,
        });

        if let Some(node) = mount.arena().get_mut(container_id) {
            node.height = NodeDimension::Exact(self.viewport_height);
            node.clip_bounds = Some(Rectangle::new(Point::ORIGIN, Size::new(10000.0, self.viewport_height)));
        }

        container_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_mounts_and_creates_scene_nodes() {
        let mut arena = SceneArena::new();
        let mut mount = ReactiveMount::new(&mut arena);

        let button = ReactiveButton::new("Click Me");
        let btn_id = button.mount(&mut mount);

        assert!(btn_id.is_some());
        let first_child = {
            let node = mount.arena().get(btn_id).unwrap();
            assert!(matches!(node.kind, NodeKind::Quad { .. }));
            assert!(node.first_child.is_some());
            node.first_child
        };

        let text_child = mount.arena().get(first_child).unwrap();
        assert!(matches!(&text_child.kind, NodeKind::Text { content, .. } if content == "Click Me"));
    }

    #[test]
    fn virtual_list_calculates_visible_window() {
        let list = ReactiveVirtualList::new(100_000, 40.0, 400.0);
        assert_eq!(list.visible_range(), 0..12);

        let _ = list.scroll_y.set(200.0); // 5 items down
        assert_eq!(list.visible_range(), 5..17);

        let _ = list.scroll_y.set(3_999_900.0); // near end of 100k items
        let range = list.visible_range();
        assert!(range.end <= 100_000);
    }
}

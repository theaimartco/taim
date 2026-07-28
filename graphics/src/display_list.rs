//! Immutable, compact display list representation.
//!
//! A [`DisplayList`] is a frozen snapshot of the rendering commands a
//! widget subtree produced during its most recent paint pass. When a
//! retained layer's [`Revision`] has not changed, the runtime can skip
//! calling `Widget::draw` entirely and replay the existing
//! [`DisplayList`] instead — no allocations, no widget traversal, no
//! primitive rebuilds.
//!
//! # Resource safety
//!
//! Every [`Command`] that references a GPU resource (a texture, a buffer,
//! a shaped text run) carries a [`Generation`] stamp. The renderer must
//! compare this stamp against the resource's current generation before
//! using it; if they disagree the display list is stale and must be
//! rebuilt rather than replayed. This prevents use-after-free of GPU
//! handles across device loss, atlas eviction, or cache purge events.
//!
//! [`Revision`]: crate::core::widget::tree::Revision

use crate::core::{Color, Point, Rectangle, Size, Transformation};
use std::sync::Arc;
use std::sync::atomic::{self, AtomicU64};

/// A monotonically increasing GPU resource generation counter.
///
/// When a GPU resource (texture, buffer, atlas slot) is created or
/// recreated (e.g. after device loss), its [`Generation`] is bumped.
/// A [`Command`] in a [`DisplayList`] records the [`Generation`] it was
/// built against; if the resource's current generation differs, the
/// command's reference is stale and the display list must be rebuilt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Generation(u64);

impl Generation {
    /// The initial generation (no resource has been created yet).
    pub const INITIAL: Self = Self(0);

    /// Allocates a new, globally unique generation.
    pub fn next() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, atomic::Ordering::Relaxed))
    }

    /// Returns the raw generation value.
    pub fn value(self) -> u64 {
        self.0
    }
}

/// A unique identifier for a [`DisplayList`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DisplayListId(u64);

impl DisplayListId {
    fn next() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, atomic::Ordering::Relaxed))
    }
}

/// A single rendering command in a [`DisplayList`].
///
/// Commands are compact value types designed for efficient storage and
/// replay. Each variant carries only the data needed to reproduce the
/// visual output; actual GPU resources are referenced by generation-
/// stamped handles rather than owned.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Draw a filled or bordered rectangle (quad).
    Quad {
        /// The bounds of the quad.
        bounds: Rectangle,
        /// The background color.
        background: Color,
        /// Border radius (uniform for now).
        border_radius: f32,
        /// Border width.
        border_width: f32,
        /// Border color.
        border_color: Color,
    },

    /// Draw a text run.
    Text {
        /// The position of the text.
        position: Point,
        /// The color of the text.
        color: Color,
        /// The bounds of the text area.
        bounds: Rectangle,
        /// The generation of the shaped text cache entry.
        cache_generation: Generation,
    },

    /// Draw a raster image.
    Image {
        /// The bounds to draw the image in.
        bounds: Rectangle,
        /// The generation of the image cache entry.
        cache_generation: Generation,
    },

    /// Push a clip rectangle onto the clip stack.
    PushClip {
        /// The clip bounds.
        bounds: Rectangle,
    },

    /// Pop the current clip rectangle from the clip stack.
    PopClip,

    /// Push a transformation onto the transform stack.
    PushTransform {
        /// The transformation to apply.
        transformation: Transformation,
    },

    /// Pop the current transformation from the transform stack.
    PopTransform,

    /// A no-op command, used as a placeholder or for alignment.
    Nop,
}

impl Command {
    /// Returns the bounds of this command, if it has spatial extent.
    pub fn bounds(&self) -> Option<Rectangle> {
        match self {
            Command::Quad { bounds, .. }
            | Command::Text { bounds, .. }
            | Command::Image { bounds, .. }
            | Command::PushClip { bounds } => Some(*bounds),
            Command::PopClip
            | Command::PushTransform { .. }
            | Command::PopTransform
            | Command::Nop => None,
        }
    }

    /// Returns the resource generation this command was built against,
    /// if it references a GPU resource.
    pub fn resource_generation(&self) -> Option<Generation> {
        match self {
            Command::Text {
                cache_generation, ..
            }
            | Command::Image {
                cache_generation, ..
            } => Some(*cache_generation),
            _ => None,
        }
    }
}

/// An immutable, compact list of rendering [`Command`]s.
///
/// Display lists are reference-counted (`Arc`) so they can be shared
/// between the retained layer cache and the active rendering pipeline
/// without copying. Two display lists can be compared cheaply by
/// [`Arc`] pointer equality when the question is "is this exactly the
/// same snapshot?" rather than "do these produce identical pixels?".
#[derive(Debug, Clone)]
pub struct DisplayList {
    id: DisplayListId,
    commands: Arc<[Command]>,
    /// The aggregate bounds of all spatial commands in the list.
    bounds: Rectangle,
    /// The generation of GPU resources this list was built against.
    /// If any resource has since been recreated with a newer generation,
    /// this list is stale and must not be replayed.
    resource_generation: Generation,
}

impl DisplayList {
    /// Returns the unique identifier of this display list.
    pub fn id(&self) -> DisplayListId {
        self.id
    }

    /// Returns the commands in this display list.
    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    /// Returns the aggregate bounds of all spatial commands.
    pub fn bounds(&self) -> Rectangle {
        self.bounds
    }

    /// Returns the resource generation this list was built against.
    pub fn resource_generation(&self) -> Generation {
        self.resource_generation
    }

    /// Returns `true` if this display list has no commands.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Returns the number of commands in this display list.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns `true` if this is the exact same display list instance
    /// (by `Arc` pointer equality). This is the cheapest possible
    /// staleness check: if two display lists share the same `Arc`,
    /// they are guaranteed identical.
    pub fn is_same_instance(&self, other: &DisplayList) -> bool {
        Arc::ptr_eq(&self.commands, &other.commands)
    }

    /// Returns `true` if this display list references any resource
    /// whose generation is older than `current_generation`.
    pub fn is_stale(&self, current_generation: Generation) -> bool {
        self.resource_generation != current_generation
    }
}

impl PartialEq for DisplayList {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for DisplayList {}

/// A builder for constructing [`DisplayList`]s.
///
/// Accumulates [`Command`]s and tracks aggregate bounds and resource
/// generations, then freezes the result into an immutable
/// [`DisplayList`].
#[derive(Debug)]
pub struct DisplayListBuilder {
    commands: Vec<Command>,
    bounds: Option<Rectangle>,
    max_resource_generation: Generation,
}

impl DisplayListBuilder {
    /// Creates a new, empty builder.
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            bounds: None,
            max_resource_generation: Generation::INITIAL,
        }
    }

    /// Creates a new builder with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            commands: Vec::with_capacity(capacity),
            bounds: None,
            max_resource_generation: Generation::INITIAL,
        }
    }

    /// Pushes a command into the display list.
    pub fn push(&mut self, command: Command) {
        // Track aggregate bounds
        if let Some(cmd_bounds) = command.bounds() {
            self.bounds = Some(match self.bounds {
                Some(existing) => existing.union(&cmd_bounds),
                None => cmd_bounds,
            });
        }

        // Track max resource generation
        if let Some(generation_id) = command.resource_generation()
            && generation_id > self.max_resource_generation
        {
            self.max_resource_generation = generation_id;
        }

        self.commands.push(command);
    }

    /// Pushes a quad command.
    pub fn push_quad(
        &mut self,
        bounds: Rectangle,
        background: Color,
        border_radius: f32,
        border_width: f32,
        border_color: Color,
    ) {
        self.push(Command::Quad {
            bounds,
            background,
            border_radius,
            border_width,
            border_color,
        });
    }

    /// Pushes a clip region.
    pub fn push_clip(&mut self, bounds: Rectangle) {
        self.push(Command::PushClip { bounds });
    }

    /// Pops the current clip region.
    pub fn pop_clip(&mut self) {
        self.push(Command::PopClip);
    }

    /// Pushes a transformation.
    pub fn push_transform(&mut self, transformation: Transformation) {
        self.push(Command::PushTransform { transformation });
    }

    /// Pops the current transformation.
    pub fn pop_transform(&mut self) {
        self.push(Command::PopTransform);
    }

    /// Returns the number of commands accumulated so far.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns `true` if no commands have been accumulated.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Freezes the builder into an immutable [`DisplayList`].
    pub fn build(self) -> DisplayList {
        let bounds = self
            .bounds
            .unwrap_or_else(|| Rectangle::new(Point::ORIGIN, Size::ZERO));

        DisplayList {
            id: DisplayListId::next(),
            commands: self.commands.into(),
            bounds,
            resource_generation: self.max_resource_generation,
        }
    }
}

impl Default for DisplayListBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rectangle {
        Rectangle::new(Point::new(x, y), Size::new(w, h))
    }

    #[test]
    fn generation_initial_is_zero() {
        assert_eq!(Generation::INITIAL.value(), 0);
    }

    #[test]
    fn generation_next_produces_unique_values() {
        let a = Generation::next();
        let b = Generation::next();
        assert_ne!(a, b);
    }

    #[test]
    fn empty_display_list() {
        let list = DisplayListBuilder::new().build();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn display_list_with_commands() {
        let mut builder = DisplayListBuilder::new();
        builder.push_quad(
            rect(0.0, 0.0, 100.0, 50.0),
            Color::WHITE,
            0.0,
            0.0,
            Color::TRANSPARENT,
        );
        builder.push_clip(rect(10.0, 10.0, 80.0, 30.0));
        builder.pop_clip();

        let list = builder.build();
        assert_eq!(list.len(), 3);
        assert!(!list.is_empty());
    }

    #[test]
    fn display_list_tracks_aggregate_bounds() {
        let mut builder = DisplayListBuilder::new();
        builder.push_quad(
            rect(0.0, 0.0, 50.0, 50.0),
            Color::WHITE,
            0.0,
            0.0,
            Color::TRANSPARENT,
        );
        builder.push_quad(
            rect(100.0, 100.0, 50.0, 50.0),
            Color::WHITE,
            0.0,
            0.0,
            Color::TRANSPARENT,
        );

        let list = builder.build();
        let bounds = list.bounds();
        assert_eq!(bounds.x, 0.0);
        assert_eq!(bounds.y, 0.0);
        assert_eq!(bounds.width, 150.0);
        assert_eq!(bounds.height, 150.0);
    }

    #[test]
    fn display_list_tracks_resource_generation() {
        let generation_id = Generation::next();
        let mut builder = DisplayListBuilder::new();
        builder.push(Command::Text {
            position: Point::ORIGIN,
            color: Color::WHITE,
            bounds: rect(0.0, 0.0, 100.0, 20.0),
            cache_generation: generation_id,
        });

        let list = builder.build();
        assert_eq!(list.resource_generation(), generation_id);
        assert!(!list.is_stale(generation_id));
        assert!(list.is_stale(Generation::next()));
    }

    #[test]
    fn display_list_same_instance_check() {
        let list = DisplayListBuilder::new().build();
        let clone = list.clone();
        assert!(list.is_same_instance(&clone));

        let other = DisplayListBuilder::new().build();
        assert!(!list.is_same_instance(&other));
    }

    #[test]
    fn display_list_equality_is_by_id() {
        let list = DisplayListBuilder::new().build();
        let clone = list.clone();
        assert_eq!(list, clone);

        let other = DisplayListBuilder::new().build();
        assert_ne!(list, other);
    }

    #[test]
    fn command_bounds() {
        let quad = Command::Quad {
            bounds: rect(10.0, 20.0, 30.0, 40.0),
            background: Color::WHITE,
            border_radius: 0.0,
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        };
        assert_eq!(quad.bounds(), Some(rect(10.0, 20.0, 30.0, 40.0)));

        assert_eq!(Command::PopClip.bounds(), None);
        assert_eq!(Command::Nop.bounds(), None);
    }

    #[test]
    fn command_resource_generation() {
        let generation_id = Generation::next();
        let text = Command::Text {
            position: Point::ORIGIN,
            color: Color::WHITE,
            bounds: rect(0.0, 0.0, 100.0, 20.0),
            cache_generation: generation_id,
        };
        assert_eq!(text.resource_generation(), Some(generation_id));

        let quad = Command::Quad {
            bounds: rect(0.0, 0.0, 10.0, 10.0),
            background: Color::WHITE,
            border_radius: 0.0,
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        };
        assert_eq!(quad.resource_generation(), None);
    }

    #[test]
    fn builder_with_capacity() {
        let builder = DisplayListBuilder::with_capacity(100);
        assert!(builder.is_empty());
    }
}

//! Leverage advanced concepts like custom widgets.
pub mod subscription {
    //! Write your own subscriptions.
    pub use crate::runtime::futures::subscription::{
        Event, EventStream, Hasher, MacOS, PlatformSpecific, Recipe, from_recipe, into_recipes,
    };
}

pub mod widget {
    //! Create custom widgets and operate on them.
    pub use crate::core::widget::*;
    pub use crate::runtime::task::widget as operate;
}

pub use crate::core::Shell;
pub use crate::core::clipboard;
pub use crate::core::image;
pub use crate::core::input_method::{self, InputMethod};
pub use crate::core::layout::{self, Layout};
pub use crate::core::mouse;
pub use crate::core::overlay::{self, Overlay};
pub use crate::core::renderer::{self, Renderer};
pub use crate::core::svg;
pub use crate::core::text::{self, Text};
pub use crate::renderer::graphics;

pub use widget::Widget;

/// TAIM frame/input low-level observability for specialized widget authors.
pub mod native_contracts {
    /// Event-queue coalescing policy: lifecycle events must not be lost.
    pub use crate::core::event::Coalescing;
    /// Per-window redraw deadlines selected by the TAIM frame scheduler.
    pub use crate::core::window::RedrawRequest;
    /// Cross-device mouse/touch/pen gesture identity and lifecycle.
    pub use crate::core::pointer::{Id as PointerId, Phase as PointerPhase, Pointer};
}

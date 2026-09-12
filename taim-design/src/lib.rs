//! Taim's renderer-neutral design system.
pub mod compiler;
pub mod components;
pub mod icon;
pub mod platform;
pub mod state;
pub mod surface;
pub mod theme;
pub mod tokens;

/// Common Taim design-system imports.
pub mod prelude {
    pub use crate::components::{
        Appearance, ComponentSpec, Density, FocusRing, InputMethods, LayoutSpec, MIN_TOUCH_TARGET,
        MIN_TOUCH_TARGET_COMPATIBILITY, StateSupport, TouchTargetPolicy, ViewportClass,
    };
    pub use crate::state::{Interaction, ResolvedState};
    pub use crate::theme::Theme;
    pub use crate::tokens::{ColorTokens, Spacing, Typography};
}

pub use theme::Theme;

//! Aura's renderer-neutral design system.
pub mod components;
pub mod icon;
pub mod platform;
pub mod state;
pub mod surface;
pub mod theme;
pub mod tokens;

/// Common Aura design-system imports.
pub mod prelude {
    pub use crate::components::{
        Appearance, ComponentSpec, Density, FocusRing, InputMethods, LayoutSpec, MIN_TOUCH_TARGET,
        StateSupport, ViewportClass,
    };
    pub use crate::state::{Interaction, ResolvedState};
    pub use crate::theme::Theme;
    pub use crate::tokens::{ColorTokens, Spacing, Typography};
}

pub use theme::Theme;

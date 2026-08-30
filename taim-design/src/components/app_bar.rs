#![allow(missing_docs)]

use super::{ComponentSpec, Density, InputMethods, StateSupport, ViewportClass};
use crate::tokens::Radius;

use taim_core::Size;
use taim_core::semantics::{Actions, Role};

/// How much vertical room the bar's title occupies, and whether it
/// collapses as content scrolls beneath it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppBarKind {
    #[default]
    Standard,
    Large,
    Collapsing,
}

impl AppBarKind {
    /// The bar's height at rest, in logical pixels.
    pub const fn expanded_height(self) -> f32 {
        match self {
            Self::Standard => 64.0,
            // A large/collapsing bar reserves room for a display-size
            // title above the action row.
            Self::Large | Self::Collapsing => 112.0,
        }
    }

    /// The height a collapsing bar shrinks to once content scrolls under
    /// it. Non-collapsing kinds keep their full height.
    pub const fn collapsed_height(self) -> f32 {
        match self {
            Self::Standard => 64.0,
            Self::Large => 112.0,
            Self::Collapsing => 64.0,
        }
    }
}

/// The application's top bar.
///
/// The bar is a **container**, not a control: it has no actions of its
/// own. Its `navigation_action` slot hosts an
/// [`icon_button::IconButton`](super::icon_button::IconButton), which is
/// what carries the role, focus, and touch target for that affordance.
/// Modelling the bar itself as activatable would claim an action with no
/// focusable, touch-sized target behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AppBar {
    pub kind: AppBarKind,
    /// Whether the bar hosts a leading navigation control (a back or menu
    /// button). The control itself is an `IconButton`.
    pub navigation_action: bool,
}

impl AppBar {
    /// Returns the bar's height for a given collapse progress, where
    /// `0.0` is fully expanded and `1.0` fully collapsed. Values outside
    /// that range clamp.
    pub fn height(&self, collapse_progress: f32) -> f32 {
        let expanded = self.kind.expanded_height();
        let collapsed = self.kind.collapsed_height();
        let progress = collapse_progress.clamp(0.0, 1.0);

        expanded + (collapsed - expanded) * progress
    }
}

impl ComponentSpec for AppBar {
    fn role(&self) -> Role {
        Role::Group
    }

    fn actions(&self) -> Actions {
        // A container has no actions; see this type's own documentation.
        Actions::empty()
    }

    fn min_size(&self, _density: Density) -> Size {
        // Width is imposed by the window, not the bar.
        Size::new(0.0, self.kind.collapsed_height())
    }

    fn supported_states(&self) -> StateSupport {
        StateSupport::empty()
    }

    fn input_methods(&self) -> InputMethods {
        InputMethods::empty()
    }

    fn layout(&self, viewport: ViewportClass, density: Density) -> super::LayoutSpec {
        use crate::tokens::Spacing;

        super::LayoutSpec {
            min_size: self.min_size(density),
            // A bar's padding tracks the window's own gutter, which grows
            // with the viewport.
            padding: match viewport {
                ViewportClass::Compact => Spacing::SM,
                ViewportClass::Medium => Spacing::LG,
                ViewportClass::Expanded => Spacing::XL,
            },
            gap: Spacing::SM,
        }
    }

    fn radius(&self) -> Radius {
        // The bar meets the window edge; rounding it would leave gaps.
        Radius::None
    }
}

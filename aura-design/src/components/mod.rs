//! Renderer-neutral production component contracts.
//!
//! Every Aura component implements [`ComponentSpec`], which encodes
//! `phase.md` Phase 11's "component completion contract" as types rather
//! than prose:
//!
//! - **State matrix.** [`ComponentSpec::supported_states`] declares which
//!   of the interaction states a component meaningfully has, and
//!   [`ComponentSpec::appearance`] resolves any [`Interaction`] against a
//!   [`Theme`] into a token-derived [`Appearance`]. A component never
//!   invents a colour: every field of `Appearance` comes from the theme's
//!   tokens, which is what "theme tokens only -- no unexplained
//!   hard-coded color/spacing/motion values" means mechanically.
//! - **Navigation.** [`ComponentSpec::navigation`] declares which input
//!   methods reach the component (keyboard, pointer, touch, gamepad).
//! - **Touch target and focus ring.** [`ComponentSpec::min_size`] must
//!   meet [`MIN_TOUCH_TARGET`] for anything interactive, and a focusable
//!   component must produce a [`FocusRing`] when focus is visible.
//! - **Adaptive layout.** [`ComponentSpec::layout`] returns the
//!   [`LayoutSpec`] for a [`ViewportClass`]/[`Density`] pair, all of whose
//!   spacing comes from [`Spacing`] tokens.
//! - **Readability without effects.**
//!   [`ComponentSpec::readable_without_effects`] asserts a component never
//!   requires blur/shadow/noise to be understood -- effects are decoration,
//!   never the thing that distinguishes content or controls.
//!
//! `tests/design/components.rs` drives *every* component in this module
//! through *every* state, theme, viewport, and density and enforces these
//! rules generically, so a new component cannot quietly skip part of the
//! contract.

#![allow(missing_docs)]

pub mod app_bar;
pub mod bottom_sheet;
pub mod button;
pub mod card;
pub mod dialog;
pub mod icon_button;
pub mod list_tile;
pub mod menu;
pub mod navigation;
pub mod notification;
pub mod progress;
pub mod selection;
pub mod slider;
pub mod text_field;
pub mod toast;
pub mod tooltip;
pub mod window_chrome;

use crate::state::Interaction;
use crate::theme::Theme;
use crate::tokens::{Radius, Spacing};

use aura_core::semantics::{Actions, Role};
use aura_core::{Color, Size};

/// The smallest edge, in logical pixels, any interactive component may
/// present as a touch target. Matches the 44px floor common to platform
/// accessibility guidance; Aura's own density scale starts above it.
pub const MIN_TOUCH_TARGET: f32 = 44.0;

/// Screen-size class used by adaptive components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewportClass {
    Compact,
    #[default]
    Medium,
    Expanded,
}

impl ViewportClass {
    /// Every viewport class, for exhaustive iteration in tests.
    pub const ALL: [Self; 3] = [Self::Compact, Self::Medium, Self::Expanded];
}

/// Layout density selected by user/system policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Density {
    Compact,
    #[default]
    Comfortable,
    Spacious,
}

impl Density {
    /// Every density, for exhaustive iteration in tests.
    pub const ALL: [Self; 3] = [Self::Compact, Self::Comfortable, Self::Spacious];
}

bitflags::bitflags! {
    /// Which interaction states a component meaningfully supports.
    ///
    /// A component only has to define behavior for the states it declares
    /// here; the state-matrix test uses this to decide what to demand of
    /// it. Declaring a state you do not honor is a contract violation the
    /// matrix test catches (e.g. claiming `DISABLED` while still exposing
    /// actions when disabled).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StateSupport: u16 {
        const HOVER = 1 << 0;
        const FOCUS_VISIBLE = 1 << 1;
        const PRESSED = 1 << 2;
        const SELECTED = 1 << 3;
        const DISABLED = 1 << 4;
        const LOADING = 1 << 5;
        const ERROR = 1 << 6;
    }
}

impl StateSupport {
    /// The states an ordinary focusable, pressable control has.
    pub const INTERACTIVE: Self = Self::HOVER
        .union(Self::FOCUS_VISIBLE)
        .union(Self::PRESSED)
        .union(Self::DISABLED);
}

bitflags::bitflags! {
    /// Which input methods can reach a component.
    ///
    /// `phase.md` requires every component to declare "keyboard, pointer,
    /// touch and gamepad/focus navigation". Anything interactive must be
    /// reachable by keyboard *and* pointer *and* touch at minimum --
    /// pointer-only controls are an accessibility failure the matrix test
    /// rejects.
    ///
    /// (Named `InputMethods` rather than `Navigation` because
    /// [`navigation::Navigation`] is itself a component in this module.)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct InputMethods: u8 {
        const KEYBOARD = 1 << 0;
        const POINTER = 1 << 1;
        const TOUCH = 1 << 2;
        const GAMEPAD = 1 << 3;
    }
}

impl InputMethods {
    /// Every input method an interactive Aura component must accept.
    pub const ALL: Self = Self::KEYBOARD
        .union(Self::POINTER)
        .union(Self::TOUCH)
        .union(Self::GAMEPAD);
}

/// The focus indicator drawn around a focused component.
///
/// A focus ring is never optional for a focusable control: it is the only
/// thing that tells a keyboard user where they are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FocusRing {
    /// Ring thickness in logical pixels.
    pub width: f32,
    /// Ring colour, always the theme's `focus` token.
    pub color: Color,
    /// Gap between the component's edge and the ring.
    pub offset: f32,
}

impl FocusRing {
    /// The standard ring: 2px, offset by the smallest spacing token.
    pub fn standard(color: Color) -> Self {
        Self {
            width: 2.0,
            color,
            offset: Spacing::XS / 2.0,
        }
    }
}

/// The layout a component occupies at a given viewport/density.
///
/// Every measurement is derived from [`Spacing`] tokens or the component's
/// own minimum size -- never an unexplained literal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutSpec {
    /// The smallest box the component may occupy.
    pub min_size: Size,
    /// Interior padding, from the spacing scale.
    pub padding: f32,
    /// Gap between the component's own children, from the spacing scale.
    pub gap: f32,
}

/// A component's resolved visual state, entirely token-derived.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Appearance {
    /// Fill behind the component, if it draws one.
    pub background: Option<Color>,
    /// Content colour (text/icon).
    pub foreground: Color,
    /// Border colour, if the component draws one.
    pub border: Option<Color>,
    /// Focus indicator, present exactly when focus is visible.
    pub focus_ring: Option<FocusRing>,
    /// State-layer alpha (hover/press/disabled scrim).
    pub overlay_alpha: f32,
    /// Corner shape from the radius scale.
    pub radius: Radius,
    /// Whether the component currently accepts input.
    pub enabled: bool,
    /// Whether the component is showing indeterminate progress.
    pub busy: bool,
}

/// Common semantic, physical, and visual requirements for every Aura
/// component. See this module's own documentation for the full contract.
pub trait ComponentSpec {
    /// The assistive-technology role this component reports.
    fn role(&self) -> Role;

    /// The actions assistive technology may invoke, given current state.
    /// A disabled component must expose no actions.
    fn actions(&self) -> Actions;

    /// The smallest box this component may occupy at `density`.
    fn min_size(&self, density: Density) -> Size;

    /// Which interaction states this component meaningfully supports.
    fn supported_states(&self) -> StateSupport;

    /// Which input methods reach this component. Defaults to none, which
    /// is correct only for non-interactive presentation components.
    fn input_methods(&self) -> InputMethods {
        InputMethods::empty()
    }

    /// Whether this component can hold focus. Derived from its declared
    /// actions by default.
    fn is_focusable(&self) -> bool {
        self.actions().contains(Actions::FOCUS)
    }

    /// Whether this component accepts input at all in its current state.
    fn is_interactive(&self) -> bool {
        !self.actions().is_empty()
    }

    /// The layout this component occupies at `viewport`/`density`.
    ///
    /// The default derives padding/gap from the spacing scale, widening
    /// as the viewport grows -- the "compact/medium/expanded layout
    /// behavior" every component must declare. Components with their own
    /// adaptive rules override this.
    fn layout(&self, viewport: ViewportClass, density: Density) -> LayoutSpec {
        let padding = match viewport {
            ViewportClass::Compact => Spacing::SM,
            ViewportClass::Medium => Spacing::MD,
            ViewportClass::Expanded => Spacing::LG,
        };

        LayoutSpec {
            min_size: self.min_size(density),
            padding,
            gap: match density {
                Density::Compact => Spacing::XS,
                Density::Comfortable => Spacing::SM,
                Density::Spacious => Spacing::MD,
            },
        }
    }

    /// The corner shape this component uses, from the radius scale.
    fn radius(&self) -> Radius {
        Radius::Medium
    }

    /// Resolves this component's appearance for `interaction` against
    /// `theme`.
    ///
    /// The default composes [`Interaction::resolve`] (the shared state
    /// resolver) with this component's own radius and focus ring, which
    /// is correct for most components; a component with its own fill
    /// (e.g. a filled button) overrides this while still drawing every
    /// value from `theme`.
    fn appearance(&self, interaction: Interaction, theme: &Theme) -> Appearance {
        let colors = theme.colors();
        let resolved = interaction.resolve(colors);

        Appearance {
            background: None,
            foreground: resolved.foreground,
            border: None,
            focus_ring: resolved.focus_ring.map(FocusRing::standard),
            overlay_alpha: resolved.overlay_alpha,
            radius: self.radius(),
            enabled: resolved.enabled,
            busy: resolved.busy,
        }
    }

    /// Whether this component reads correctly with every expensive effect
    /// disabled (the Recovery quality tier).
    ///
    /// Always `true` for Aura components by policy: blur, shadow, and
    /// noise are decoration, never the thing that distinguishes content or
    /// controls. Overriding this to `false` is a design error the matrix
    /// test rejects; the method exists so that rule is stated in code.
    fn readable_without_effects(&self) -> bool {
        true
    }

    /// Whether this component lays out correctly mirrored.
    fn supports_rtl(&self) -> bool {
        true
    }

    /// Whether this component reflows rather than clipping when the user
    /// enlarges text.
    fn supports_large_text(&self) -> bool {
        true
    }
}

/// The touch-target size for an interactive control at `density`, always
/// at or above [`MIN_TOUCH_TARGET`].
pub(crate) fn touch_size(density: Density) -> Size {
    let side = match density {
        Density::Compact => 48.0,
        Density::Comfortable => 52.0,
        Density::Spacious => 56.0,
    };
    Size::new(side, side)
}

//! Component interaction state resolver.
#![allow(missing_docs)]
use crate::tokens::ColorTokens;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Interaction {
    pub hovered: bool,
    pub focus_visible: bool,
    pub pressed: bool,
    pub selected: bool,
    pub disabled: bool,
    pub loading: bool,
    pub error: bool,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedState {
    pub foreground: taim_core::Color,
    pub focus_ring: Option<taim_core::Color>,
    pub overlay_alpha: f32,
    pub enabled: bool,
    pub busy: bool,
}
impl Interaction {
    pub fn resolve(self, c: ColorTokens) -> ResolvedState {
        let enabled = !self.disabled;
        let foreground = if self.error {
            c.error
        } else if self.selected {
            c.primary
        } else {
            c.on_surface
        };
        let overlay_alpha = if !enabled {
            0.38
        } else if self.pressed {
            0.16
        } else if self.hovered {
            0.08
        } else {
            0.0
        };
        ResolvedState {
            foreground,
            focus_ring: self.focus_visible.then_some(c.focus),
            overlay_alpha,
            enabled,
            busy: self.loading,
        }
    }
}

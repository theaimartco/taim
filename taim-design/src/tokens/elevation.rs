//! Surface hierarchy independent of expensive effects.
#![allow(missing_docs)]
use taim_core::{Color, Vector};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Elevation {
    #[default]
    Flat,
    Raised,
    Floating,
    Modal,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElevationRecipe {
    pub overlay_alpha: f32,
    pub shadow: Color,
    pub offset: Vector,
    pub blur_radius: f32,
}
impl Elevation {
    pub const fn recipe(self, dark: bool) -> ElevationRecipe {
        let shadow = Color::from_rgba(0.0, 0.0, 0.0, if dark { 0.45 } else { 0.18 });
        match self {
            Self::Flat => ElevationRecipe {
                overlay_alpha: 0.0,
                shadow,
                offset: Vector::new(0.0, 0.0),
                blur_radius: 0.0,
            },
            Self::Raised => ElevationRecipe {
                overlay_alpha: if dark { 0.08 } else { 0.0 },
                shadow,
                offset: Vector::new(0.0, 2.0),
                blur_radius: 6.0,
            },
            Self::Floating => ElevationRecipe {
                overlay_alpha: if dark { 0.12 } else { 0.0 },
                shadow,
                offset: Vector::new(0.0, 6.0),
                blur_radius: 12.0,
            },
            Self::Modal => ElevationRecipe {
                overlay_alpha: if dark { 0.16 } else { 0.0 },
                shadow,
                offset: Vector::new(0.0, 12.0),
                blur_radius: 20.0,
            },
        }
    }

    /// Maps elevation to the current GPU quality tier without making blur a readability requirement.
    pub const fn recipe_for_tier(
        self,
        dark: bool,
        tier: taim_graphics::quality::Tier,
    ) -> ElevationRecipe {
        let mut recipe = self.recipe(dark);
        let policy = tier.policy();
        recipe.blur_radius = recipe.blur_radius.min(policy.max_blur_radius);
        if matches!(tier, taim_graphics::quality::Tier::Recovery) {
            recipe.overlay_alpha = if dark { 0.12 } else { 0.0 };
        }
        recipe
    }
}

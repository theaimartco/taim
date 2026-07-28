//! Renderer-neutral adaptive quality policy.
#![allow(missing_docs)]

/// The active visual-quality tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Tier {
    /// Full effects where budgets allow them.
    Ultra,
    /// Default bounded effects.
    #[default]
    Balanced,
    /// Readable visuals with reduced effect cost.
    Efficient,
    /// GPU-loss/recovery-safe visuals with expensive effects disabled.
    Recovery,
}

/// Per-tier effect limits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Policy {
    /// Largest blur radius in logical pixels.
    pub max_blur_radius: f32,
    /// Largest effect sampling area in logical pixels squared.
    pub max_effect_area: f32,
    /// Maximum intermediate texture bytes.
    pub texture_budget_bytes: u64,
    /// Whether backdrop capture is allowed.
    pub backdrop_blur: bool,
    /// Whether optional material noise is allowed.
    pub noise: bool,
}

impl Tier {
    /// Returns bounded effect policy for this tier.
    pub const fn policy(self) -> Policy {
        match self {
            Self::Ultra => Policy {
                max_blur_radius: 32.0,
                max_effect_area: 2_073_600.0,
                texture_budget_bytes: 96 * 1024 * 1024,
                backdrop_blur: true,
                noise: true,
            },
            Self::Balanced => Policy {
                max_blur_radius: 20.0,
                max_effect_area: 1_048_576.0,
                texture_budget_bytes: 48 * 1024 * 1024,
                backdrop_blur: true,
                noise: true,
            },
            Self::Efficient => Policy {
                max_blur_radius: 8.0,
                max_effect_area: 262_144.0,
                texture_budget_bytes: 16 * 1024 * 1024,
                backdrop_blur: false,
                noise: false,
            },
            Self::Recovery => Policy {
                max_blur_radius: 0.0,
                max_effect_area: 0.0,
                texture_budget_bytes: 4 * 1024 * 1024,
                backdrop_blur: false,
                noise: false,
            },
        }
    }
}

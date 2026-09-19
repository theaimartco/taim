use crate::damage::DamageRegion;

/// Immutable hints describing one compositor presentation.
#[derive(Debug, Clone)]
pub struct FrameContext {
    /// Regions that changed, or `None` when the whole surface is unknown.
    pub damage: Option<DamageRegion>,
    /// Validated logical-to-physical scale factor.
    pub scale_factor: f32,
    /// Whether presentation uses the sRGB color space.
    pub srgb: bool,
}

impl FrameContext {
    /// Creates a full-surface presentation context.
    pub fn full_present(scale_factor: f32) -> Self {
        Self {
            damage: None,
            scale_factor: valid_scale_factor(scale_factor),
            srgb: true,
        }
    }

    /// Creates a damage-aware presentation context.
    pub fn with_damage(damage: DamageRegion, scale_factor: f32) -> Self {
        Self {
            damage: Some(damage),
            scale_factor: valid_scale_factor(scale_factor),
            srgb: true,
        }
    }

    /// Returns whether a non-empty, partial region can be presented.
    #[inline]
    pub fn supports_partial_present(&self) -> bool {
        self.damage
            .as_ref()
            .is_some_and(|damage| !damage.is_full() && !damage.is_empty())
    }
}

#[inline]
fn valid_scale_factor(scale_factor: f32) -> f32 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

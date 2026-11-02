//! TAIM shadow geometry used by paint and retained-cache decisions.

use crate::{Color, Vector};

/// Shadows are paint effects, not layout extents.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Shadow {
    /// Color of the shadow.
    pub color: Color,
    /// Displacement of the shadow from the casting shape.
    pub offset: Vector,
    /// Blur radius in logical pixels; zero gives a hard edge.
    pub blur_radius: f32,
}

impl Shadow {
    /// Drop non-finite or negative blur radii before queuing paint work.
    ///
    /// A zero radius remains a valid hard-edged shadow. Offset and color are
    /// preserved so renderers can validate their own coordinate-space limits.
    #[must_use]
    pub fn normalized(mut self) -> Self {
        if !self.blur_radius.is_finite() || self.blur_radius < 0.0 {
            self.blur_radius = 0.0;
        }
        self
    }

    /// True only when no alpha contribution is possible. NaN alpha is
    /// deliberately treated as potentially visible (conservative culling).
    #[must_use]
    pub fn is_fully_transparent(self) -> bool {
        self.color.a == 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_blur_cannot_leak_into_shadow_geometry() {
        let mut shadow = Shadow {
            blur_radius: f32::NAN,
            ..Shadow::default()
        };
        assert_eq!(shadow.normalized().blur_radius, 0.0);
        shadow.blur_radius = -10.0;
        assert_eq!(shadow.normalized().blur_radius, 0.0);
        shadow.blur_radius = 2.0;
        assert_eq!(shadow.normalized().blur_radius, 2.0);
    }

    #[test]
    fn transparent_shadow_culling_is_conservative() {
        let mut shadow = Shadow {
            color: Color::TRANSPARENT,
            ..Shadow::default()
        };
        assert!(shadow.is_fully_transparent());
        shadow.color = Color::BLACK;
        assert!(!shadow.is_fully_transparent());
    }
}

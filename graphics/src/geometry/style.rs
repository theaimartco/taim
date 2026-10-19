use crate::core::Color;
use crate::geometry::Gradient;

/// The coloring style of some drawing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Style {
    /// A solid [`Color`].
    Solid(Color),

    /// A [`Gradient`] color.
    Gradient(Gradient),
}

impl Style {
    /// Conservatively skip only geometry that cannot contribute visible alpha.
    ///
    /// An empty or partially defined gradient is allowed through because a
    /// backend may choose fallback stops. NaN alpha is never culled.
    #[inline]
    pub fn is_fully_transparent(self) -> bool {
        match self {
            Self::Solid(color) => color.a == 0.0,
            Self::Gradient(Gradient::Linear(gradient)) => {
                let mut has_stops = false;
                for stop in gradient.stops.into_iter().flatten() {
                    has_stops = true;
                    if stop.color.a != 0.0 {
                        return false;
                    }
                }
                has_stops
            }
        }
    }
}

impl From<Color> for Style {
    fn from(color: Color) -> Self {
        Self::Solid(color)
    }
}

impl From<Gradient> for Style {
    fn from(gradient: Gradient) -> Self {
        Self::Gradient(gradient)
    }
}

#[cfg(test)]
mod taim_transparency_tests {
    use super::*;
    use crate::core::Point;
    use crate::gradient::Linear;

    #[test]
    fn culls_only_proven_transparent_solid_paint() {
        assert!(Style::Solid(Color::TRANSPARENT).is_fully_transparent());
        assert!(!Style::Solid(Color::BLACK).is_fully_transparent());
        let mut unknown = Color::TRANSPARENT;
        unknown.a = f32::NAN;
        assert!(!Style::Solid(unknown).is_fully_transparent());
    }

    #[test]
    fn gradient_culling_preserves_empty_and_visible_fallbacks() {
        let empty = Linear::new(Point::ORIGIN, Point::new(1.0, 0.0));
        assert!(!Style::Gradient(Gradient::Linear(empty)).is_fully_transparent());
        let clear = empty
            .add_stop(0.0, Color::TRANSPARENT)
            .add_stop(1.0, Color::TRANSPARENT);
        assert!(Style::Gradient(Gradient::Linear(clear)).is_fully_transparent());
        let visible = clear.add_stop(0.5, Color::WHITE);
        assert!(!Style::Gradient(Gradient::Linear(visible)).is_fully_transparent());
    }
}

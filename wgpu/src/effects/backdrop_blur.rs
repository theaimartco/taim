//! Bounded backdrop capture model.
use crate::core::Rectangle;
use crate::graphics::quality::Policy;
/// Backdrop blur request, restricted to source bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackdropBlur {
    pub radius: f32,
    pub area: f32,
}
impl BackdropBlur {
    pub fn bounds(self, source: Rectangle, policy: Policy) -> Rectangle {
        if !policy.backdrop_blur || self.area > policy.max_effect_area {
            return source;
        }
        let pad = self.radius.clamp(0.0, policy.max_blur_radius).ceil();
        Rectangle {
            x: source.x - pad,
            y: source.y - pad,
            width: source.width + pad * 2.0,
            height: source.height + pad * 2.0,
        }
    }
}

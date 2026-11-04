//! Insets are maxima, not additive: the IME often overlaps the navigation bar.
use taim_core::{Point, Rectangle, Size};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Insets {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
}

impl Insets {
    pub fn area(self, physical: Size<u32>, scale: f32) -> Rectangle {
        let left = self.left.min(physical.width);
        let top = self.top.min(physical.height);
        let width = physical
            .width
            .saturating_sub(left)
            .saturating_sub(self.right);
        let height = physical
            .height
            .saturating_sub(top)
            .saturating_sub(self.bottom);
        Rectangle::new(
            Point::new(left as f32 / scale, top as f32 / scale),
            Size::new(width as f32 / scale, height as f32 / scale),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_insets_never_underflow_and_density_is_applied_once() {
        let insets = Insets {
            left: 0,
            top: 60,
            right: 0,
            bottom: 500,
        };
        assert_eq!(
            insets.area(Size::new(1000, 2000), 2.0),
            Rectangle::new(Point::new(0.0, 30.0), Size::new(500.0, 720.0))
        );
        assert_eq!(insets.area(Size::new(1, 1), 1.0).height, 0.0);
    }
}

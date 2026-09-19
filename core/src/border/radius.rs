use crate::Pixels;

/// Corner radii in top-left, top-right, bottom-right, bottom-left order.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Radius {
    /// Top-left radius.
    pub top_left: f32,
    /// Top-right radius.
    pub top_right: f32,
    /// Bottom-right radius.
    pub bottom_right: f32,
    /// Bottom-left radius.
    pub bottom_left: f32,
}

impl Radius {
    /// Creates equal radii for all corners.
    pub fn new(value: impl Into<Pixels>) -> Self {
        Self::from(normalize(value.into().0))
    }

    /// Sets top-left radius.
    pub fn top_left(self, value: impl Into<Pixels>) -> Self {
        Self {
            top_left: normalize(value.into().0),
            ..self
        }
    }

    /// Sets top-right radius.
    pub fn top_right(self, value: impl Into<Pixels>) -> Self {
        Self {
            top_right: normalize(value.into().0),
            ..self
        }
    }

    /// Sets bottom-right radius.
    pub fn bottom_right(self, value: impl Into<Pixels>) -> Self {
        Self {
            bottom_right: normalize(value.into().0),
            ..self
        }
    }

    /// Sets bottom-left radius.
    pub fn bottom_left(self, value: impl Into<Pixels>) -> Self {
        Self {
            bottom_left: normalize(value.into().0),
            ..self
        }
    }

    /// Sets both top radii.
    pub fn top(self, value: impl Into<Pixels>) -> Self {
        let value = normalize(value.into().0);
        Self {
            top_left: value,
            top_right: value,
            ..self
        }
    }

    /// Sets both bottom radii.
    pub fn bottom(self, value: impl Into<Pixels>) -> Self {
        let value = normalize(value.into().0);
        Self {
            bottom_left: value,
            bottom_right: value,
            ..self
        }
    }

    /// Sets both left radii.
    pub fn left(self, value: impl Into<Pixels>) -> Self {
        let value = normalize(value.into().0);
        Self {
            top_left: value,
            bottom_left: value,
            ..self
        }
    }

    /// Sets both right radii.
    pub fn right(self, value: impl Into<Pixels>) -> Self {
        let value = normalize(value.into().0);
        Self {
            top_right: value,
            bottom_right: value,
            ..self
        }
    }
}

impl From<f32> for Radius {
    fn from(radius: f32) -> Self {
        let radius = normalize(radius);
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }
}

impl From<u8> for Radius {
    fn from(value: u8) -> Self {
        Self::from(f32::from(value))
    }
}

impl From<u32> for Radius {
    fn from(value: u32) -> Self {
        Self::from(value as f32)
    }
}

impl From<i32> for Radius {
    fn from(value: i32) -> Self {
        Self::from(value as f32)
    }
}

impl From<Radius> for [f32; 4] {
    fn from(radius: Radius) -> Self {
        [
            radius.top_left,
            radius.top_right,
            radius.bottom_right,
            radius.bottom_left,
        ]
    }
}

impl std::ops::Mul<f32> for Radius {
    type Output = Self;

    fn mul(self, scale: f32) -> Self::Output {
        let scale = if scale.is_finite() { scale.abs() } else { 0.0 };
        Self {
            top_left: self.top_left * scale,
            top_right: self.top_right * scale,
            bottom_right: self.bottom_right * scale,
            bottom_left: self.bottom_left * scale,
        }
    }
}

#[inline]
fn normalize(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

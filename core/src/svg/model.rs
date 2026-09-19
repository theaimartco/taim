use super::Handle;
use crate::{Color, Radians};

/// A raster image that can be drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct Svg<H = Handle> {
    /// The handle of the [`Svg`].
    pub handle: H,

    /// The [`Color`] filter to be applied to the [`Svg`].
    ///
    /// If some [`Color`] is set, the whole [`Svg`] will be
    /// painted with it—ignoring any intrinsic colors.
    ///
    /// This can be useful for coloring icons programmatically
    /// (e.g. with a theme).
    pub color: Option<Color>,

    /// The rotation to be applied to the image; on its center.
    pub rotation: Radians,

    /// The opacity of the [`Svg`].
    ///
    /// 0 means transparent. 1 means opaque.
    pub opacity: f32,
}

impl Svg<Handle> {
    /// Creates a new [`Svg`] with the given handle.
    pub fn new(handle: impl Into<Handle>) -> Self {
        Self {
            handle: handle.into(),
            color: None,
            rotation: Radians(0.0),
            opacity: 1.0,
        }
    }

    /// Sets the [`Color`] filter of the [`Svg`].
    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// Sets the rotation of the [`Svg`].
    pub fn rotation(mut self, rotation: impl Into<Radians>) -> Self {
        self.rotation = rotation.into();
        self
    }

    /// Sets the opacity of the [`Svg`].
    pub fn opacity(mut self, opacity: impl Into<f32>) -> Self {
        let opacity = opacity.into();
        self.opacity = if opacity.is_finite() {
            opacity.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self
    }
}

impl From<&Handle> for Svg {
    fn from(handle: &Handle) -> Self {
        Svg::new(handle.clone())
    }
}

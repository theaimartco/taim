use crate::core::Rectangle;
#[cfg(feature = "geometry")]
use crate::core::{Point, Size};

/// A prepared software path with its conservative visible bounds.
#[derive(Debug, Clone, PartialEq)]
pub enum Primitive {
    Fill {
        path: tiny_skia::Path,
        paint: tiny_skia::Paint<'static>,
        rule: tiny_skia::FillRule,
        bounds: Rectangle,
    },
    Stroke {
        path: tiny_skia::Path,
        paint: tiny_skia::Paint<'static>,
        stroke: tiny_skia::Stroke,
        bounds: Rectangle,
    },
}

impl Primitive {
    #[cfg(feature = "geometry")]
    pub(crate) fn fill(
        path: tiny_skia::Path,
        paint: tiny_skia::Paint<'static>,
        rule: tiny_skia::FillRule,
    ) -> Self {
        let bounds = path_bounds(&path, 0.0);
        Self::Fill {
            path,
            paint,
            rule,
            bounds,
        }
    }

    #[cfg(feature = "geometry")]
    pub(crate) fn stroke(
        path: tiny_skia::Path,
        paint: tiny_skia::Paint<'static>,
        stroke: tiny_skia::Stroke,
    ) -> Self {
        let bounds = path_bounds(&path, stroke.width.max(0.0) * 0.5);
        Self::Stroke {
            path,
            paint,
            stroke,
            bounds,
        }
    }

    pub fn scale_alpha(mut self, factor: f32) -> Self {
        let paint = match &mut self {
            Self::Fill { paint, .. } | Self::Stroke { paint, .. } => paint,
        };
        paint.shader.apply_opacity(factor.clamp(0.0, 1.0));
        self
    }

    #[inline]
    pub fn visible_bounds(&self) -> Rectangle {
        match self {
            Self::Fill { bounds, .. } | Self::Stroke { bounds, .. } => *bounds,
        }
    }
}

#[cfg(feature = "geometry")]
fn path_bounds(path: &tiny_skia::Path, expansion: f32) -> Rectangle {
    let bounds = path.bounds();
    Rectangle::new(
        Point::new(bounds.x() - expansion, bounds.y() - expansion),
        Size::new(
            bounds.width() + expansion * 2.0,
            bounds.height() + expansion * 2.0,
        ),
    )
}

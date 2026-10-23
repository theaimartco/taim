//! Raster and vector pictures.
use crate::cx::{DescribeCx, MeasureCx, PaintCx};
use crate::layout::{Axis, Constraints, Sizing};
use crate::{Color, Point, Rectangle, Size, View, semantics};

use taim_core::{ContentFit, border};

#[cfg(feature = "image")]
use taim_core::image::{self, Renderer as _};
#[cfg(feature = "svg")]
use taim_core::svg::{self, Renderer as _};

/// How a picture asks for room: its natural size, or fixed sides, or a
/// fixed side with the other following the aspect ratio.
#[derive(Clone, Copy, Default)]
struct Wanted {
    width: Option<f32>,
    height: Option<f32>,
}

impl Wanted {
    /// The box for a picture of `natural` size inside `constraints`.
    fn resolve(self, natural: Size, constraints: Constraints) -> Size {
        let ratio = if natural.height > 0.0 {
            natural.width / natural.height
        } else {
            1.0
        };

        let (width, height) = match (self.width, self.height) {
            (Some(width), Some(height)) => (width, height),
            (Some(width), None) => (width, width / ratio.max(f32::EPSILON)),
            (None, Some(height)) => (height * ratio, height),
            (None, None) => (natural.width, natural.height),
        };

        let fitted = Size::new(width, height);
        let limit = constraints.max;

        // A picture that is too large for its parent shrinks to fit it.
        if self.width.is_none()
            && self.height.is_none()
            && (fitted.width > limit.width || fitted.height > limit.height)
        {
            return ContentFit::Contain.fit(fitted, limit);
        }

        fitted
    }
}

/// Where a picture of `natural` size lands in `bounds` under `fit`.
fn placed(fit: ContentFit, natural: Size, bounds: Rectangle) -> Rectangle {
    let size = fit.fit(natural, bounds.size());

    Rectangle::new(
        Point::new(
            bounds.x + (bounds.width - size.width) / 2.0,
            bounds.y + (bounds.height - size.height) / 2.0,
        ),
        size,
    )
}

fn sizing(wanted: Wanted) -> Sizing {
    let axis = |side: Option<f32>| side.map_or(Axis::Hug, Axis::Fixed);

    Sizing {
        width: axis(wanted.width),
        height: axis(wanted.height),
    }
}

/// A raster image.
#[cfg(feature = "image")]
pub fn image(handle: impl Into<image::Handle>) -> Image {
    Image {
        handle: handle.into(),
        wanted: Wanted::default(),
        fit: ContentFit::Contain,
        filter: image::FilterMethod::default(),
        radius: border::Radius::default(),
        opacity: 1.0,
        natural: Size::ZERO,
        label: String::new(),
        id: taim_core::widget::Id::unique(),
    }
}

/// See [`image`].
#[cfg(feature = "image")]
pub struct Image {
    handle: image::Handle,
    wanted: Wanted,
    fit: ContentFit,
    filter: image::FilterMethod,
    radius: border::Radius,
    opacity: f32,
    natural: Size,
    label: String,
    id: taim_core::widget::Id,
}

#[cfg(feature = "image")]
impl Image {
    /// Exactly this wide; the height follows the aspect ratio unless set.
    pub fn width(mut self, width: f32) -> Self {
        self.wanted.width = Some(width.max(0.0));
        self
    }

    /// Exactly this tall; the width follows the aspect ratio unless set.
    pub fn height(mut self, height: f32) -> Self {
        self.wanted.height = Some(height.max(0.0));
        self
    }

    /// How the picture fills its box when the aspect ratios differ.
    pub fn fit(mut self, fit: ContentFit) -> Self {
        self.fit = fit;
        self
    }

    /// Rounds the corners.
    pub fn radius(mut self, radius: impl Into<border::Radius>) -> Self {
        self.radius = radius.into();
        self
    }

    /// 0 is invisible, 1 is opaque.
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Samples with `filter` when scaling.
    pub fn filter(mut self, filter: image::FilterMethod) -> Self {
        self.filter = filter;
        self
    }

    /// What assistive technology announces for the picture.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }
}

#[cfg(feature = "image")]
impl View for Image {
    fn sizing(&self) -> Sizing {
        sizing(self.wanted)
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let renderer = cx.renderer();
        let natural = renderer
            .measure_image(&self.handle)
            .or_else(|| {
                renderer
                    .load_image(&self.handle)
                    .ok()
                    .map(|loaded| loaded.size())
            })
            .unwrap_or_default();

        self.natural = Size::new(natural.width as f32, natural.height as f32);
        self.wanted.resolve(self.natural, constraints)
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let bounds = cx.bounds();
        let target = placed(self.fit, self.natural, bounds);

        let mut picture = image::Image::new(self.handle.clone());
        picture.filter_method = self.filter;
        picture.border_radius = self.radius;
        picture.opacity = self.opacity;

        cx.renderer().draw_image(picture, target, bounds);
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        Some(
            semantics::Node::new(self.id.clone(), cx.bounds(), semantics::Role::Image)
                .with_name(self.label.clone()),
        )
    }

    fn clips(&self) -> bool {
        true
    }
}

/// A vector image.
#[cfg(feature = "svg")]
pub fn svg(handle: impl Into<svg::Handle>) -> Svg {
    Svg {
        handle: handle.into(),
        wanted: Wanted::default(),
        fit: ContentFit::Contain,
        tint: None,
        opacity: 1.0,
        natural: Size::ZERO,
        label: String::new(),
        id: taim_core::widget::Id::unique(),
    }
}

/// See [`svg`].
#[cfg(feature = "svg")]
pub struct Svg {
    handle: svg::Handle,
    wanted: Wanted,
    fit: ContentFit,
    tint: Option<Color>,
    opacity: f32,
    natural: Size,
    label: String,
    id: taim_core::widget::Id,
}

#[cfg(feature = "svg")]
impl Svg {
    /// Exactly this wide; the height follows the aspect ratio unless set.
    pub fn width(mut self, width: f32) -> Self {
        self.wanted.width = Some(width.max(0.0));
        self
    }

    /// Exactly this tall; the width follows the aspect ratio unless set.
    pub fn height(mut self, height: f32) -> Self {
        self.wanted.height = Some(height.max(0.0));
        self
    }

    /// Both sides at once, for icons.
    pub fn size(self, side: f32) -> Self {
        self.width(side).height(side)
    }

    /// How the picture fills its box when the aspect ratios differ.
    pub fn fit(mut self, fit: ContentFit) -> Self {
        self.fit = fit;
        self
    }

    /// Paints the whole picture in `color`, ignoring its own colors.
    pub fn tint(mut self, color: impl Into<Color>) -> Self {
        self.tint = Some(color.into());
        self
    }

    /// 0 is invisible, 1 is opaque.
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// What assistive technology announces for the picture.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }
}

#[cfg(feature = "svg")]
impl View for Svg {
    fn sizing(&self) -> Sizing {
        sizing(self.wanted)
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let natural = cx.renderer().measure_svg(&self.handle);

        self.natural = Size::new(natural.width as f32, natural.height as f32);
        self.wanted.resolve(self.natural, constraints)
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let bounds = cx.bounds();
        let target = placed(self.fit, self.natural, bounds);

        let mut picture = svg::Svg::new(self.handle.clone());
        picture.color = self.tint;
        picture.opacity = self.opacity;

        cx.renderer().draw_svg(picture, target, bounds);
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        Some(
            semantics::Node::new(self.id.clone(), cx.bounds(), semantics::Role::Image)
                .with_name(self.label.clone()),
        )
    }

    fn clips(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_fixed_side_keeps_the_aspect_ratio() {
        let natural = Size::new(200.0, 100.0);
        let wanted = Wanted {
            width: Some(50.0),
            height: None,
        };

        let size = wanted.resolve(natural, Constraints::loose(Size::new(500.0, 500.0)));
        assert_eq!(size, Size::new(50.0, 25.0));
    }

    #[test]
    fn oversized_pictures_shrink_to_their_parent() {
        let size = Wanted::default().resolve(
            Size::new(400.0, 200.0),
            Constraints::loose(Size::new(100.0, 100.0)),
        );
        assert_eq!(size, Size::new(100.0, 50.0));
    }

    #[test]
    fn contain_centers_the_picture() {
        let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        let at = placed(ContentFit::Contain, Size::new(200.0, 100.0), bounds);
        assert_eq!(
            at,
            Rectangle::new(Point::new(0.0, 25.0), Size::new(100.0, 50.0))
        );
    }
}

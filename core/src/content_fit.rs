//! Control the fit of some content (like an image) within a space.
use crate::Size;

use std::fmt;

/// The strategy used to fit the contents of a widget to its bounding box.
///
/// Each variant of this enum is a strategy that can be applied for resolving
/// differences in aspect ratio and size between the image being displayed and
/// the space its being displayed in.
///
/// For an interactive demonstration of these properties as they are implemented
/// in CSS, see [Mozilla's docs][1], or run the `tour` example
///
/// [1]: https://developer.mozilla.org/en-US/docs/Web/CSS/object-fit
#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentFit {
    /// Scale as big as it can be without needing to crop or hide parts.
    ///
    /// The image will be scaled (preserving aspect ratio) so that it just fits
    /// within the window.  This won't distort the image or crop/hide any edges,
    /// but if the image doesn't fit perfectly, there may be whitespace on the
    /// top/bottom or left/right.
    ///
    /// This is a great fit for when you need to display an image without losing
    /// any part of it, particularly when the image itself is the focus of the
    /// screen.
    #[default]
    Contain,

    /// Scale the image to cover all of the bounding box, cropping if needed.
    ///
    /// This doesn't distort the image, and it ensures that the widget's area is
    /// completely covered, but it might crop off a bit of the edges of the
    /// widget, particularly when there is a big difference between the aspect
    /// ratio of the widget and the aspect ratio of the image.
    ///
    /// This is best for when you're using an image as a background, or to fill
    /// space, and any details of the image around the edge aren't too
    /// important.
    Cover,

    /// Distort the image so the widget is 100% covered without cropping.
    ///
    /// This stretches the image to fit the widget, without any whitespace or
    /// cropping. However, because of the stretch, the image may look distorted
    /// or elongated, particularly when there's a mismatch of aspect ratios.
    Fill,

    /// Don't resize or scale the image at all.
    ///
    /// This will not apply any transformations to the provided image, but also
    /// means that unless you do the math yourself, the widget's area will not
    /// be completely covered, or the image might be cropped.
    ///
    /// This is best for when you've sized the image yourself.
    None,

    /// Scale the image down if it's too big for the space, but never scale it
    /// up.
    ///
    /// This works much like [`Contain`](Self::Contain), except that if the
    /// image would have been scaled up, it keeps its original resolution to
    /// avoid the bluring that accompanies upscaling images.
    ScaleDown,
}

impl ContentFit {
    /// Attempt to apply the given fit for a content size within some bounds.
    ///
    /// The returned value is the recommended scaled size of the content.
    pub fn fit(&self, content: Size, bounds: Size) -> Size {
        match self {
            Self::Fill => bounds,
            Self::None => content,
            Self::Contain => scaled(content, fit_scale(content, bounds, Scale::Contain)),
            Self::Cover => scaled(content, fit_scale(content, bounds, Scale::Cover)),
            Self::ScaleDown => scaled(content, fit_scale(content, bounds, Scale::Contain).min(1.0)),
        }
    }
}

#[derive(Clone, Copy)]
enum Scale {
    Contain,
    Cover,
}

#[inline]
fn fit_scale(content: Size, bounds: Size, mode: Scale) -> f32 {
    if !content.width.is_finite()
        || !content.height.is_finite()
        || content.width <= 0.0
        || content.height <= 0.0
    {
        return 0.0;
    }

    let horizontal = axis_scale(bounds.width, content.width);
    let vertical = axis_scale(bounds.height, content.height);

    match (horizontal, vertical, mode) {
        (Some(x), Some(y), Scale::Contain) => x.min(y),
        (Some(x), Some(y), Scale::Cover) => x.max(y),
        (Some(scale), None, _) | (None, Some(scale), _) => scale,
        (None, None, _) => 1.0,
    }
}

#[inline]
fn axis_scale(limit: f32, intrinsic: f32) -> Option<f32> {
    if limit.is_infinite() && limit.is_sign_positive() {
        None
    } else if limit.is_finite() {
        Some((limit / intrinsic).max(0.0))
    } else {
        Some(0.0)
    }
}

#[inline]
fn scaled(content: Size, factor: f32) -> Size {
    Size::new(content.width * factor, content.height * factor)
}

impl fmt::Display for ContentFit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ContentFit::Contain => "Contain",
            ContentFit::Cover => "Cover",
            ContentFit::Fill => "Fill",
            ContentFit::None => "None",
            ContentFit::ScaleDown => "Scale Down",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_modes_use_stable_axis_scales() {
        let content = Size::new(200.0, 100.0);
        let bounds = Size::new(100.0, 100.0);

        assert_eq!(
            ContentFit::Contain.fit(content, bounds),
            Size::new(100.0, 50.0)
        );
        assert_eq!(
            ContentFit::Cover.fit(content, bounds),
            Size::new(200.0, 100.0)
        );
        assert_eq!(
            ContentFit::ScaleDown.fit(content, bounds),
            Size::new(100.0, 50.0)
        );
    }

    #[test]
    fn malformed_or_unbounded_geometry_never_produces_nan() {
        assert_eq!(
            ContentFit::Contain.fit(Size::ZERO, Size::INFINITE),
            Size::ZERO
        );
        assert_eq!(
            ContentFit::Contain.fit(Size::UNIT, Size::INFINITE),
            Size::UNIT
        );
        assert_eq!(
            ContentFit::Cover.fit(Size::UNIT, Size::new(f32::NAN, 20.0)),
            Size::new(20.0, 20.0)
        );
    }
}

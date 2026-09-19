use crate::core::border;
use crate::core::image;
use crate::core::layout;
use crate::core::{ContentFit, Layout, Length, Point, Rectangle, Rotation, Size, Vector};

use super::FilterMethod;

/// Computes raster image layout using renderer-provided natural dimensions.
pub fn layout<Renderer, Handle>(
    renderer: &Renderer,
    limits: &layout::Limits,
    handle: &Handle,
    width: Length,
    height: Length,
    region: Option<Rectangle<u32>>,
    content_fit: ContentFit,
    rotation: Rotation,
    expand: bool,
) -> layout::Node
where
    Renderer: image::Renderer<Handle = Handle>,
{
    layout_with_size(
        limits,
        renderer.measure_image(handle).unwrap_or_default(),
        width,
        height,
        region,
        content_fit,
        rotation,
        expand,
    )
}

pub(super) fn layout_with_size(
    limits: &layout::Limits,
    original_size: Size<u32>,
    width: Length,
    height: Length,
    region: Option<Rectangle<u32>>,
    content_fit: ContentFit,
    rotation: Rotation,
    expand: bool,
) -> layout::Node {
    let image_size = cropped_size(original_size, region);
    let rotated_size = rotation.apply(image_size);
    let bounds = if expand {
        limits.width(width).height(height).max()
    } else {
        limits.resolve(width, height, rotated_size)
    };
    let fitted = content_fit.fit(rotated_size, bounds);

    layout::Node::new(Size {
        width: if width == Length::Shrink {
            bounds.width.min(fitted.width)
        } else {
            bounds.width
        },
        height: if height == Length::Shrink {
            bounds.height.min(fitted.height)
        } else {
            bounds.height
        },
    })
}

fn drawing_bounds(
    bounds: Rectangle,
    original_size: Size<u32>,
    region: Option<Rectangle<u32>>,
    content_fit: ContentFit,
    rotation: Rotation,
    scale: f32,
) -> Option<Rectangle> {
    let image_size = cropped_size(original_size, region);

    if original_size.width == 0
        || original_size.height == 0
        || image_size.width <= 0.0
        || image_size.height <= 0.0
        || bounds.width <= 0.0
        || bounds.height <= 0.0
    {
        return None;
    }

    let rotated_size = rotation.apply(image_size);
    if rotated_size.width <= 0.0 || rotated_size.height <= 0.0 {
        return None;
    }

    let fitted = content_fit.fit(rotated_size, bounds.size());
    let fit_scale = Vector::new(
        fitted.width / rotated_size.width,
        fitted.height / rotated_size.height,
    );
    let final_size = image_size * fit_scale * scale;

    let (crop_offset, final_size) = if let Some(region) = region {
        let x = region.x.min(original_size.width) as f32;
        let y = region.y.min(original_size.height) as f32;
        let ratio = Vector::new(
            original_size.width as f32 / image_size.width,
            original_size.height as f32 / image_size.height,
        );
        let final_size = final_size * ratio;
        let pixel_scale = Vector::new(
            final_size.width / original_size.width as f32,
            final_size.height / original_size.height as f32,
        );
        let offset = match content_fit {
            ContentFit::None => Vector::new(x * pixel_scale.x, y * pixel_scale.y),
            _ => Vector::new(
                ((original_size.width as f32 - image_size.width) / 2.0 - x) * pixel_scale.x,
                ((original_size.height as f32 - image_size.height) / 2.0 - y) * pixel_scale.y,
            ),
        };

        (offset, final_size)
    } else {
        (Vector::ZERO, final_size)
    };

    let position = match content_fit {
        ContentFit::None => Point::new(
            bounds.x + (rotated_size.width - fitted.width) / 2.0,
            bounds.y + (rotated_size.height - fitted.height) / 2.0,
        ),
        _ => Point::new(
            bounds.center_x() - final_size.width / 2.0,
            bounds.center_y() - final_size.height / 2.0,
        ),
    };

    Some(Rectangle::new(position + crop_offset, final_size))
}

fn cropped_size(size: Size<u32>, region: Option<Rectangle<u32>>) -> Size<f32> {
    let Some(region) = region else {
        return Size::new(size.width as f32, size.height as f32);
    };

    let x = region.x.min(size.width);
    let y = region.y.min(size.height);

    Size::new(
        region.width.min(size.width.saturating_sub(x)) as f32,
        region.height.min(size.height.saturating_sub(y)) as f32,
    )
}

/// Draws an image using dimensions queried from the renderer.
pub fn draw<Renderer, Handle>(
    renderer: &mut Renderer,
    layout: Layout<'_>,
    handle: &Handle,
    crop: Option<Rectangle<u32>>,
    border_radius: border::Radius,
    content_fit: ContentFit,
    filter_method: FilterMethod,
    rotation: Rotation,
    opacity: f32,
    scale: f32,
) where
    Renderer: image::Renderer<Handle = Handle>,
    Handle: Clone,
{
    draw_with_size(
        renderer,
        layout.bounds(),
        handle,
        renderer.measure_image(handle).unwrap_or_default(),
        crop,
        border_radius,
        content_fit,
        filter_method,
        rotation,
        opacity,
        scale,
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_with_size<Renderer, Handle>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    handle: &Handle,
    original_size: Size<u32>,
    crop: Option<Rectangle<u32>>,
    border_radius: border::Radius,
    content_fit: ContentFit,
    filter_method: FilterMethod,
    rotation: Rotation,
    opacity: f32,
    scale: f32,
) where
    Renderer: image::Renderer<Handle = Handle>,
    Handle: Clone,
{
    let Some(drawing_bounds) =
        drawing_bounds(bounds, original_size, crop, content_fit, rotation, scale)
    else {
        return;
    };

    renderer.draw_image(
        image::Image {
            handle: handle.clone(),
            border_radius,
            filter_method,
            rotation: rotation.radians(),
            opacity,
        },
        drawing_bounds,
        bounds,
    );
}

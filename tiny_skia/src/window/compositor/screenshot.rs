use crate::Renderer;
use crate::core::{Color, Rectangle};
use crate::graphics::Viewport;

pub(super) fn capture(renderer: &mut Renderer, viewport: &Viewport, background: Color) -> Vec<u8> {
    let size = viewport.physical_size();
    let Some(pixel_count) = crate::window::screenshot_pixel_budget(size.width, size.height) else {
        // The capture budget covers both target storage and output emission.
        return Vec::new();
    };

    let mut buffer = Vec::new();
    if buffer.try_reserve_exact(pixel_count).is_err() {
        return Vec::new();
    }
    buffer.resize(pixel_count, 0u32);
    let Some(mut clip_mask) = tiny_skia::Mask::new(size.width, size.height) else {
        return Vec::new();
    };
    let Some(mut pixels) = tiny_skia::PixmapMut::from_bytes(
        bytemuck::cast_slice_mut(&mut buffer),
        size.width,
        size.height,
    ) else {
        return Vec::new();
    };

    renderer.draw(
        &mut pixels,
        &mut clip_mask,
        viewport,
        &[Rectangle::with_size(viewport.logical_size())],
        background,
    );

    let Some(byte_count) = pixel_count.checked_mul(4) else {
        return Vec::new();
    };
    let mut output = Vec::new();
    if output.try_reserve_exact(byte_count).is_err() {
        return Vec::new();
    }
    for pixel in buffer {
        let alpha = (pixel >> 24) as u8;
        let mut red = ((pixel >> 16) & 0xFF) as u8;
        let mut green = ((pixel >> 8) & 0xFF) as u8;
        let mut blue = (pixel & 0xFF) as u8;
        if crate::graphics::color::GAMMA_CORRECTION {
            red = crate::engine::encode_channel(red);
            green = crate::engine::encode_channel(green);
            blue = crate::engine::encode_channel(blue);
        }
        output.extend_from_slice(&[red, green, blue, alpha]);
    }
    output
}

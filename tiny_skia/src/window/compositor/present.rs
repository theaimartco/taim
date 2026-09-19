use super::state::{Frame, MAX_FRAME_HISTORY, Surface};
use crate::core::{Color, Rectangle, Size};
use crate::graphics::Viewport;
use crate::graphics::compositor;
use crate::graphics::damage;
use crate::{Layer, Renderer};

use std::sync::Arc;

pub(super) fn frame(
    renderer: &mut Renderer,
    surface: &mut Surface,
    viewport: &Viewport,
    background: Color,
    on_pre_present: impl FnOnce(),
) -> Result<(), compositor::SurfaceError> {
    let size = viewport.physical_size();
    if size.width == 0 || size.height == 0 {
        on_pre_present();
        return Ok(());
    }

    let mut buffer = surface
        .window
        .buffer_mut()
        .map_err(|_| compositor::SurfaceError::Lost)?;
    let age = usize::from(buffer.age());
    surface.max_age = surface.max_age.max(age).min(MAX_FRAME_HISTORY);
    surface.frames.truncate(surface.max_age);
    let previous = age
        .checked_sub(1)
        .filter(|index| *index < MAX_FRAME_HISTORY)
        .and_then(|index| surface.frames.get(index));

    let layers = renderer.layers();
    let damage = previous
        .filter(|frame| frame.background == background)
        .map(|frame| {
            damage::diff(
                &frame.layers,
                layers,
                |layer| vec![layer.bounds],
                Layer::damage,
            )
        })
        .unwrap_or_else(|| vec![Rectangle::with_size(viewport.logical_size())]);

    if damage.is_empty() {
        if let Some(previous) = previous {
            surface.frames.push_front(previous.clone());
            surface.frames.truncate(MAX_FRAME_HISTORY);
        }
    } else {
        surface.frames.push_front(Frame {
            background,
            layers: Arc::from(layers),
        });
        surface.frames.truncate(MAX_FRAME_HISTORY);

        let damage = damage::group(damage, Rectangle::with_size(viewport.logical_size()));
        if crate::graphics::color::GAMMA_CORRECTION {
            let required = (size.width as usize)
                .checked_mul(size.height as usize)
                .ok_or(compositor::SurfaceError::OutOfMemory)?;
            if surface.working.len() != required {
                surface
                    .working
                    .try_reserve(required.saturating_sub(surface.working.len()))
                    .map_err(|_| compositor::SurfaceError::OutOfMemory)?;
                surface.working.resize(required, 0);
            }
            let mut pixels = tiny_skia::PixmapMut::from_bytes(
                bytemuck::cast_slice_mut(&mut surface.working),
                size.width,
                size.height,
            )
            .ok_or(compositor::SurfaceError::Lost)?;
            renderer.draw(
                &mut pixels,
                &mut surface.clip_mask,
                viewport,
                &damage,
                background,
            );
            encode_damage(
                &surface.working,
                &mut buffer,
                size,
                &damage,
                viewport.scale_factor(),
            );
        } else {
            let mut pixels = tiny_skia::PixmapMut::from_bytes(
                bytemuck::cast_slice_mut(&mut buffer),
                size.width,
                size.height,
            )
            .ok_or(compositor::SurfaceError::Lost)?;
            renderer.draw(
                &mut pixels,
                &mut surface.clip_mask,
                viewport,
                &damage,
                background,
            );
        }
    }

    on_pre_present();
    buffer.present().map_err(|_| compositor::SurfaceError::Lost)
}

fn encode_damage(
    working: &[u32],
    presented: &mut [u32],
    size: Size<u32>,
    damage: &[Rectangle],
    scale_factor: f32,
) {
    for region in damage {
        let left = ((region.x * scale_factor).floor().max(0.0) as u32).min(size.width);
        let top = ((region.y * scale_factor).floor().max(0.0) as u32).min(size.height);
        let right =
            (((region.x + region.width) * scale_factor).ceil().max(0.0) as u32).min(size.width);
        let bottom =
            (((region.y + region.height) * scale_factor).ceil().max(0.0) as u32).min(size.height);

        for y in top..bottom {
            let row = (y * size.width) as usize;
            for index in row + left as usize..row + right as usize {
                presented[index] = encode_pixel(working[index]);
            }
        }
    }
}

#[inline]
fn encode_pixel(source: u32) -> u32 {
    let alpha = source & 0xFF_00_00_00;
    let red = crate::engine::encode_channel(((source >> 16) & 0xFF) as u8);
    let green = crate::engine::encode_channel(((source >> 8) & 0xFF) as u8);
    let blue = crate::engine::encode_channel((source & 0xFF) as u8);
    alpha | (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue)
}

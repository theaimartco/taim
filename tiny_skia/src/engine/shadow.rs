use crate::core::{Rectangle, Shadow, Transformation};

use rustc_hash::FxHashMap;
use std::sync::Arc;

const BYTE_BUDGET: usize = 16 * 1024 * 1024;
const MAX_IDLE_FRAMES: u64 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    width: u32,
    height: u32,
    body_width: u32,
    body_height: u32,
    blur: u32,
    radii: [u32; 4],
    color: [u32; 4],
}

struct Entry {
    pixmap: Arc<tiny_skia::Pixmap>,
    bytes: usize,
    last_frame: u64,
}

pub(super) struct Cache {
    entries: FxHashMap<Key, Entry>,
    bytes: usize,
    frame: u64,
}

impl std::fmt::Debug for Cache {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ShadowCache")
            .field("entries", &self.entries.len())
            .field("bytes", &self.bytes)
            .field("frame", &self.frame)
            .finish()
    }
}

impl Cache {
    pub fn new() -> Self {
        Self {
            entries: FxHashMap::default(),
            bytes: 0,
            frame: 0,
        }
    }

    pub fn draw(
        &mut self,
        bounds: Rectangle,
        shadow: Shadow,
        radii: [f32; 4],
        transformation: Transformation,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: &tiny_skia::Mask,
    ) {
        if shadow.color.a <= 0.0 || bounds.width <= 0.0 || bounds.height <= 0.0 {
            return;
        }

        let axes = transformation.scale_factors();
        if !axes.x.is_finite() || !axes.y.is_finite() || axes.x <= 0.0 || axes.y <= 0.0 {
            return;
        }

        let scale = axes.x.min(axes.y);
        let blur = (shadow.blur_radius.max(0.0) * scale).ceil();
        let body_width = (bounds.width * axes.x).ceil().max(1.0);
        let body_height = (bounds.height * axes.y).ceil().max(1.0);
        let width = (body_width + 2.0 * blur).ceil();
        let height = (body_height + 2.0 * blur).ceil();
        if width > u32::MAX as f32 || height > u32::MAX as f32 {
            return;
        }

        let width = width as u32;
        let height = height as u32;
        let Some(bytes) = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
        else {
            return;
        };
        if bytes > BYTE_BUDGET {
            log::warn!(
                "TAIM software shadow skipped: {}x{} exceeds {} MiB cache ceiling",
                width,
                height,
                BYTE_BUDGET / (1024 * 1024)
            );
            return;
        }

        let color = crate::engine::working_components(shadow.color);
        let scaled_radii = radii.map(|radius| radius.max(0.0) * scale);
        let key = Key {
            width,
            height,
            body_width: body_width.to_bits(),
            body_height: body_height.to_bits(),
            blur: blur.to_bits(),
            radii: scaled_radii.map(f32::to_bits),
            color: color.map(f32::to_bits),
        };

        let pixmap = if let Some(entry) = self.entries.get_mut(&key) {
            entry.last_frame = self.frame;
            Arc::clone(&entry.pixmap)
        } else {
            let Some(pixmap) = rasterize(
                width,
                height,
                body_width,
                body_height,
                blur,
                scaled_radii,
                shadow.color,
            ) else {
                return;
            };

            self.evict_to_fit(bytes);
            let pixmap = Arc::new(pixmap);
            self.bytes += bytes;
            let _ = self.entries.insert(
                key,
                Entry {
                    pixmap: Arc::clone(&pixmap),
                    bytes,
                    last_frame: self.frame,
                },
            );
            pixmap
        };

        let logical_blur_x = blur / axes.x;
        let logical_blur_y = blur / axes.y;
        let origin_x = bounds.x + shadow.offset.x - logical_blur_x;
        let origin_y = bounds.y + shadow.offset.y - logical_blur_y;
        let matrix: [f32; 16] = transformation.into();
        let transform = tiny_skia::Transform {
            sx: matrix[0] / axes.x,
            kx: matrix[4] / axes.y,
            ky: matrix[1] / axes.x,
            sy: matrix[5] / axes.y,
            tx: matrix[0] * origin_x + matrix[4] * origin_y + matrix[12],
            ty: matrix[1] * origin_x + matrix[5] * origin_y + matrix[13],
        };

        pixels.draw_pixmap(
            0,
            0,
            pixmap.as_ref().as_ref(),
            &tiny_skia::PixmapPaint::default(),
            transform,
            Some(clip_mask),
        );
    }

    pub fn trim(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        let frame = self.frame;
        self.entries.retain(|_, entry| {
            let keep = frame.saturating_sub(entry.last_frame) <= MAX_IDLE_FRAMES;
            if !keep {
                self.bytes = self.bytes.saturating_sub(entry.bytes);
            }
            keep
        });
    }

    fn evict_to_fit(&mut self, incoming: usize) {
        while self.bytes.saturating_add(incoming) > BYTE_BUDGET {
            let victim = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_frame)
                .map(|(key, _)| *key);
            let Some(victim) = victim else {
                break;
            };
            if let Some(entry) = self.entries.remove(&victim) {
                self.bytes = self.bytes.saturating_sub(entry.bytes);
            }
        }
    }
}

fn rasterize(
    width: u32,
    height: u32,
    body_width: f32,
    body_height: f32,
    blur: f32,
    radii: [f32; 4],
    color: crate::core::Color,
) -> Option<tiny_skia::Pixmap> {
    let size = tiny_skia::IntSize::from_wh(width, height)?;
    let half_width = body_width * 0.5;
    let half_height = body_height * 0.5;
    let center_x = blur + half_width;
    let center_y = blur + half_height;
    let base = crate::engine::into_color(color);
    let mut colors = Vec::with_capacity(width as usize * height as usize);

    for y in 0..height {
        for x in 0..width {
            let to_center_x = x as f32 + 0.5 - center_x;
            let to_center_y = y as f32 + 0.5 - center_y;
            let distance =
                rounded_box_sdf(to_center_x, to_center_y, half_width, half_height, radii).max(0.0);
            let opacity = if blur <= f32::EPSILON {
                f32::from(distance <= 0.0)
            } else {
                1.0 - smoothstep(-blur, blur, distance)
            };
            let mut sample = base;
            sample.apply_opacity(opacity);
            colors.push(sample.to_color_u8().premultiply());
        }
    }

    tiny_skia::Pixmap::from_vec(bytemuck::cast_vec(colors), size)
}

fn rounded_box_sdf(x: f32, y: f32, half_width: f32, half_height: f32, radii: [f32; 4]) -> f32 {
    let radius = match (x > 0.0, y > 0.0) {
        (true, true) => radii[2],
        (true, false) => radii[1],
        (false, true) => radii[3],
        (false, false) => radii[0],
    };
    let outside_x = (x.abs() - half_width + radius).max(0.0);
    let outside_y = (y.abs() - half_height + radius).max(0.0);
    outside_x.hypot(outside_y) - radius
}

fn smoothstep(low: f32, high: f32, value: f32) -> f32 {
    let value = ((value - low) / (high - low)).clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

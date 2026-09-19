use crate::core::Color;

use rustc_hash::FxHashMap;

const BYTE_BUDGET: usize = 16 * 1024 * 1024;
const MAX_IDLE_FRAMES: u64 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Mask(cosmic_text::CacheKey, [u8; 3]),
    Color(cosmic_text::CacheKey),
}

#[derive(Debug, Clone)]
struct Entry {
    pixels: Vec<u32>,
    placement: cosmic_text::Placement,
    last_frame: u64,
    last_touch: u64,
}

#[derive(Debug, Default)]
pub(super) struct Cache {
    entries: FxHashMap<Key, Entry>,
    bytes: usize,
    frame: u64,
    clock: u64,
}

impl Cache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allocate(
        &mut self,
        cache_key: cosmic_text::CacheKey,
        color: Color,
        font_system: &mut cosmic_text::FontSystem,
        swash: &mut cosmic_text::SwashCache,
    ) -> Option<(&[u8], cosmic_text::Placement)> {
        let [red, green, blue, _] = color.into_rgba8();
        let mask_color = [
            crate::engine::decode_channel(red),
            crate::engine::decode_channel(green),
            crate::engine::decode_channel(blue),
        ];
        let color_key = Key::Color(cache_key);
        let mask_key = Key::Mask(cache_key, mask_color);
        let existing = self
            .entries
            .contains_key(&color_key)
            .then_some(color_key)
            .or_else(|| self.entries.contains_key(&mask_key).then_some(mask_key));

        self.clock = self.clock.wrapping_add(1);
        if let Some(key) = existing {
            let entry = self.entries.get_mut(&key)?;
            entry.last_frame = self.frame;
            entry.last_touch = self.clock;
            return Some((bytemuck::cast_slice(&entry.pixels), entry.placement));
        }

        let image = swash.get_image_uncached(font_system, cache_key)?;
        let pixel_count =
            (image.placement.width as usize).checked_mul(image.placement.height as usize)?;
        if pixel_count == 0 {
            return None;
        }
        let bytes = pixel_count.checked_mul(std::mem::size_of::<u32>())?;
        if bytes > BYTE_BUDGET || !self.evict_to_fit(bytes) {
            log::warn!(
                "TAIM software glyph skipped: {} bytes exceed active cache budget",
                bytes
            );
            return None;
        }

        let (key, pixels) = match image.content {
            cosmic_text::SwashContent::Mask => {
                if image.data.len() < pixel_count {
                    return None;
                }
                (
                    mask_key,
                    mask_pixels(&image.data[..pixel_count], mask_color),
                )
            }
            cosmic_text::SwashContent::SubpixelMask => {
                let required = pixel_count.checked_mul(4)?;
                if image.data.len() < required {
                    return None;
                }
                let coverage = image.data[..required]
                    .chunks_exact(4)
                    .map(|pixel| {
                        let total = u16::from(pixel[0]) + u16::from(pixel[1]) + u16::from(pixel[2]);
                        (total / 3) as u8
                    })
                    .collect::<Vec<_>>();
                (mask_key, mask_pixels(&coverage, mask_color))
            }
            cosmic_text::SwashContent::Color => {
                let required = pixel_count.checked_mul(4)?;
                if image.data.len() < required {
                    return None;
                }
                (color_key, color_pixels(&image.data[..required]))
            }
        };

        self.bytes += bytes;
        let _ = self.entries.insert(
            key,
            Entry {
                pixels,
                placement: image.placement,
                last_frame: self.frame,
                last_touch: self.clock,
            },
        );
        let entry = self.entries.get(&key)?;
        Some((bytemuck::cast_slice(&entry.pixels), entry.placement))
    }

    pub fn trim(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        let frame = self.frame;
        self.entries.retain(|_, entry| {
            let keep = frame.saturating_sub(entry.last_frame) <= MAX_IDLE_FRAMES;
            if !keep {
                self.bytes = self
                    .bytes
                    .saturating_sub(entry.pixels.len() * std::mem::size_of::<u32>());
            }
            keep
        });
    }

    fn evict_to_fit(&mut self, incoming: usize) -> bool {
        while self.bytes.saturating_add(incoming) > BYTE_BUDGET {
            let victim = self
                .entries
                .iter()
                .filter(|(_, entry)| entry.last_frame != self.frame)
                .min_by_key(|(_, entry)| (entry.last_frame, entry.last_touch))
                .map(|(key, _)| *key);
            let Some(victim) = victim else {
                return false;
            };
            if let Some(entry) = self.entries.remove(&victim) {
                self.bytes = self
                    .bytes
                    .saturating_sub(entry.pixels.len() * std::mem::size_of::<u32>());
            }
        }
        true
    }
}

fn mask_pixels(coverage: &[u8], [red, green, blue]: [u8; 3]) -> Vec<u32> {
    coverage
        .iter()
        .map(|alpha| {
            bytemuck::cast(tiny_skia::ColorU8::from_rgba(blue, green, red, *alpha).premultiply())
        })
        .collect()
}

fn color_pixels(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|pixel| {
            bytemuck::cast(
                tiny_skia::ColorU8::from_rgba(
                    crate::engine::decode_channel(pixel[2]),
                    crate::engine::decode_channel(pixel[1]),
                    crate::engine::decode_channel(pixel[0]),
                    pixel[3],
                )
                .premultiply(),
            )
        })
        .collect()
}

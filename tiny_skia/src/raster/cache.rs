use crate::core::image as raster;
use crate::graphics;

use rustc_hash::FxHashMap;

const BYTE_BUDGET: usize = 128 * 1024 * 1024;
const MAX_IDLE_FRAMES: u64 = 120;

#[derive(Debug)]
struct Entry {
    width: u32,
    height: u32,
    pixels: Vec<u32>,
    last_frame: u64,
    last_touch: u64,
}

impl Entry {
    fn bytes(&self) -> usize {
        self.pixels.len() * std::mem::size_of::<u32>()
    }

    fn pixmap(&self) -> Option<tiny_skia::PixmapRef<'_>> {
        tiny_skia::PixmapRef::from_bytes(
            bytemuck::cast_slice(&self.pixels),
            self.width,
            self.height,
        )
    }
}

#[derive(Debug)]
struct Failure {
    error: raster::Error,
    last_frame: u64,
}

#[derive(Debug, Default)]
pub(super) struct Cache {
    entries: FxHashMap<raster::Id, Entry>,
    failures: FxHashMap<raster::Id, Failure>,
    bytes: usize,
    frame: u64,
    clock: u64,
}

impl Cache {
    pub fn allocate(
        &mut self,
        handle: &raster::Handle,
    ) -> Result<tiny_skia::PixmapRef<'_>, raster::Error> {
        let id = handle.id();
        self.clock = self.clock.wrapping_add(1);

        if self.entries.contains_key(&id) {
            let entry = self.entries.get_mut(&id).ok_or(raster::Error::Empty)?;
            entry.last_frame = self.frame;
            entry.last_touch = self.clock;
            return entry.pixmap().ok_or(raster::Error::Empty);
        }
        if let Some(failure) = self.failures.get_mut(&id) {
            failure.last_frame = self.frame;
            return Err(failure.error.clone());
        }

        let decoded = match graphics::image::load(handle) {
            Ok(image) => image,
            Err(error) => return Err(self.remember_failure(id, error)),
        };
        let width = decoded.width();
        let height = decoded.height();
        let pixel_count = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| self.remember_failure(id, raster::Error::OutOfMemory))?;
        if pixel_count == 0 {
            return Err(self.remember_failure(id, raster::Error::Empty));
        }
        let required = pixel_count
            .checked_mul(std::mem::size_of::<u32>())
            .ok_or_else(|| self.remember_failure(id, raster::Error::OutOfMemory))?;
        if required > BYTE_BUDGET || !self.evict_to_fit(required) {
            return Err(self.remember_failure(id, raster::Error::OutOfMemory));
        }

        let mut pixels = Vec::new();
        if pixels.try_reserve_exact(pixel_count).is_err() {
            return Err(self.remember_failure(id, raster::Error::OutOfMemory));
        }
        pixels.extend(decoded.pixels().map(|pixel| {
            let [red, green, blue, alpha] = pixel.0;
            let encoded: u32 = bytemuck::cast(
                tiny_skia::ColorU8::from_rgba(
                    crate::engine::decode_channel(blue),
                    crate::engine::decode_channel(green),
                    crate::engine::decode_channel(red),
                    alpha,
                )
                .premultiply(),
            );
            encoded
        }));

        self.bytes += required;
        let _ = self.entries.insert(
            id,
            Entry {
                width,
                height,
                pixels,
                last_frame: self.frame,
                last_touch: self.clock,
            },
        );
        self.entries
            .get(&id)
            .and_then(Entry::pixmap)
            .ok_or(raster::Error::Empty)
    }

    pub fn trim(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        let frame = self.frame;
        let mut reclaimed = 0usize;
        self.entries.retain(|_, entry| {
            let keep = frame.saturating_sub(entry.last_frame) <= MAX_IDLE_FRAMES;
            if !keep {
                reclaimed = reclaimed.saturating_add(entry.bytes());
            }
            keep
        });
        self.bytes = self.bytes.saturating_sub(reclaimed);
        self.failures
            .retain(|_, failure| frame.saturating_sub(failure.last_frame) <= MAX_IDLE_FRAMES);
    }

    fn evict_to_fit(&mut self, incoming: usize) -> bool {
        while self.bytes.saturating_add(incoming) > BYTE_BUDGET {
            let victim = self
                .entries
                .iter()
                .filter(|(_, entry)| entry.last_frame != self.frame)
                .min_by_key(|(_, entry)| (entry.last_frame, entry.last_touch))
                .map(|(id, _)| *id);
            let Some(victim) = victim else {
                return false;
            };
            if let Some(entry) = self.entries.remove(&victim) {
                self.bytes = self.bytes.saturating_sub(entry.bytes());
            }
        }
        true
    }

    fn remember_failure(&mut self, id: raster::Id, error: raster::Error) -> raster::Error {
        let _ = self.failures.insert(
            id,
            Failure {
                error: error.clone(),
                last_frame: self.frame,
            },
        );
        error
    }
}

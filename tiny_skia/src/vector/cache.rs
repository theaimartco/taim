use crate::core::svg::{Data, Handle};
use crate::core::{Color, Size};

use resvg::usvg;
use rustc_hash::FxHashMap;

use std::fs;
use std::panic;
use std::sync::Arc;

const RASTER_BYTE_BUDGET: usize = 64 * 1024 * 1024;
const MAX_TREE_ENTRIES: usize = 256;
const MAX_RASTER_FAILURES: usize = 256;
const MAX_IDLE_FRAMES: u64 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RasterKey {
    id: u64,
    color: Option<[u8; 4]>,
    size: Size<u32>,
}

struct TreeEntry {
    tree: Option<Arc<usvg::Tree>>,
    last_frame: u64,
    last_touch: u64,
}

struct RasterEntry {
    image: tiny_skia::Pixmap,
    last_frame: u64,
    last_touch: u64,
}

impl RasterEntry {
    fn bytes(&self) -> usize {
        self.image.data().len()
    }
}

pub(super) struct Cache {
    trees: FxHashMap<u64, TreeEntry>,
    rasters: FxHashMap<RasterKey, RasterEntry>,
    raster_failures: FxHashMap<RasterKey, u64>,
    raster_bytes: usize,
    frame: u64,
    clock: u64,
    #[cfg(feature = "svg-text")]
    fontdb: Option<Arc<usvg::fontdb::Database>>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            trees: FxHashMap::default(),
            rasters: FxHashMap::default(),
            raster_failures: FxHashMap::default(),
            raster_bytes: 0,
            frame: 0,
            clock: 0,
            #[cfg(feature = "svg-text")]
            fontdb: None,
        }
    }

    pub fn viewport_dimensions(&mut self, handle: &Handle) -> Option<Size<u32>> {
        let tree = self.tree(handle)?;
        let size = tree.size();
        Some(Size::new(
            size.width().ceil().max(0.0) as u32,
            size.height().ceil().max(0.0) as u32,
        ))
    }

    pub fn raster(
        &mut self,
        handle: &Handle,
        color: Option<Color>,
        size: Size<u32>,
    ) -> Option<tiny_skia::PixmapRef<'_>> {
        if size.width == 0 || size.height == 0 {
            return None;
        }
        let key = RasterKey {
            id: handle.id(),
            color: color.map(Color::into_rgba8),
            size,
        };
        self.clock = self.clock.wrapping_add(1);

        if self.rasters.contains_key(&key) {
            let entry = self.rasters.get_mut(&key)?;
            entry.last_frame = self.frame;
            entry.last_touch = self.clock;
            return Some(entry.image.as_ref());
        }
        if self.raster_failures.contains_key(&key) {
            let _ = self.raster_failures.insert(key, self.frame);
            return None;
        }

        let required = (size.width as usize)
            .checked_mul(size.height as usize)?
            .checked_mul(4)?;
        if required > RASTER_BYTE_BUDGET || !self.evict_rasters(required) {
            self.fail_raster(key);
            return None;
        }
        let tree = self.tree(handle)?;
        let mut svg_image = resvg::tiny_skia::Pixmap::new(size.width, size.height)?;
        let tree_size = tree.size();
        if tree_size.width() <= 0.0 || tree_size.height() <= 0.0 {
            self.fail_raster(key);
            return None;
        }
        let raster_transform = resvg::tiny_skia::Transform::from_scale(
            size.width as f32 / tree_size.width(),
            size.height as f32 / tree_size.height(),
        );
        let rendered = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            resvg::render(&tree, raster_transform, &mut svg_image.as_mut());
        }));
        if let Err(error) = rendered {
            log::warn!("TAIM SVG rasterization recovered from panic: {error:?}");
            self.fail_raster(key);
            return None;
        }

        // Both versions store premultiplied RGBA8. Transfer the allocation
        // without copying; the software scene backend retains its own API.
        let mut image = tiny_skia::Pixmap::from_vec(
            svg_image.take(),
            tiny_skia::IntSize::from_wh(size.width, size.height)?,
        )?;
        convert_pixels(&mut image, key.color);
        self.raster_bytes += required;
        let _ = self.rasters.insert(
            key,
            RasterEntry {
                image,
                last_frame: self.frame,
                last_touch: self.clock,
            },
        );
        self.rasters.get(&key).map(|entry| entry.image.as_ref())
    }

    pub fn trim(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        let frame = self.frame;
        self.trees
            .retain(|_, entry| frame.saturating_sub(entry.last_frame) <= MAX_IDLE_FRAMES);

        let mut reclaimed = 0usize;
        self.rasters.retain(|_, entry| {
            let keep = frame.saturating_sub(entry.last_frame) <= MAX_IDLE_FRAMES;
            if !keep {
                reclaimed = reclaimed.saturating_add(entry.bytes());
            }
            keep
        });
        self.raster_bytes = self.raster_bytes.saturating_sub(reclaimed);
        self.raster_failures
            .retain(|_, last_frame| frame.saturating_sub(*last_frame) <= MAX_IDLE_FRAMES);
    }

    fn tree(&mut self, handle: &Handle) -> Option<Arc<usvg::Tree>> {
        let id = handle.id();
        self.clock = self.clock.wrapping_add(1);
        if let Some(entry) = self.trees.get_mut(&id) {
            entry.last_frame = self.frame;
            entry.last_touch = self.clock;
            return entry.tree.clone();
        }

        let cache_tree = self.evict_tree_slot();
        let options = self.options();
        let tree = match handle.data() {
            Data::Path(path) => fs::read(path)
                .ok()
                .and_then(|bytes| usvg::Tree::from_data(&bytes, &options).ok()),
            Data::Bytes(bytes) => usvg::Tree::from_data(bytes, &options).ok(),
        }
        .map(Arc::new);
        let result = tree.clone();
        if cache_tree {
            let _ = self.trees.insert(
                id,
                TreeEntry {
                    tree,
                    last_frame: self.frame,
                    last_touch: self.clock,
                },
            );
        }
        result
    }

    fn options(&mut self) -> usvg::Options<'static> {
        #[cfg(feature = "svg-text")]
        if self.fontdb.is_none() {
            let mut fontdb = usvg::fontdb::Database::new();
            fontdb.load_system_fonts();
            self.fontdb = Some(Arc::new(fontdb));
        }

        usvg::Options {
            #[cfg(feature = "svg-text")]
            fontdb: self
                .fontdb
                .as_ref()
                .cloned()
                .unwrap_or_else(|| Arc::new(usvg::fontdb::Database::new())),
            ..usvg::Options::default()
        }
    }

    fn evict_tree_slot(&mut self) -> bool {
        if self.trees.len() < MAX_TREE_ENTRIES {
            return true;
        }
        let victim = self
            .trees
            .iter()
            .filter(|(_, entry)| entry.last_frame != self.frame)
            .min_by_key(|(_, entry)| (entry.last_frame, entry.last_touch))
            .map(|(id, _)| *id);
        if let Some(victim) = victim {
            let _ = self.trees.remove(&victim);
            true
        } else {
            false
        }
    }

    fn evict_rasters(&mut self, incoming: usize) -> bool {
        while self.raster_bytes.saturating_add(incoming) > RASTER_BYTE_BUDGET {
            let victim = self
                .rasters
                .iter()
                .filter(|(_, entry)| entry.last_frame != self.frame)
                .min_by_key(|(_, entry)| (entry.last_frame, entry.last_touch))
                .map(|(key, _)| *key);
            let Some(victim) = victim else {
                return false;
            };
            if let Some(entry) = self.rasters.remove(&victim) {
                self.raster_bytes = self.raster_bytes.saturating_sub(entry.bytes());
            }
        }
        true
    }

    fn fail_raster(&mut self, key: RasterKey) {
        if self.raster_failures.len() < MAX_RASTER_FAILURES
            || self.raster_failures.contains_key(&key)
        {
            let _ = self.raster_failures.insert(key, self.frame);
        }
    }
}

fn convert_pixels(image: &mut tiny_skia::Pixmap, color: Option<[u8; 4]>) {
    if let Some([red, green, blue, color_alpha]) = color {
        let decoded = [
            crate::engine::decode_channel(red),
            crate::engine::decode_channel(green),
            crate::engine::decode_channel(blue),
        ];
        for pixel in bytemuck::cast_slice_mut::<u8, u32>(image.data_mut()) {
            let coverage = (*pixel >> 24) as u8;
            let alpha = ((u16::from(coverage) * u16::from(color_alpha) + 127) / 255) as u8;
            *pixel = bytemuck::cast(
                tiny_skia::ColorU8::from_rgba(decoded[2], decoded[1], decoded[0], alpha)
                    .premultiply(),
            );
        }
        return;
    }

    for pixel in bytemuck::cast_slice_mut::<u8, u32>(image.data_mut()) {
        let value = *pixel;
        let alpha = (value >> 24) as u8;
        let (red, green, blue) = crate::engine::decode_premultiplied(
            (value & 0xFF) as u8,
            ((value >> 8) & 0xFF) as u8,
            ((value >> 16) & 0xFF) as u8,
            alpha,
        );
        *pixel =
            u32::from(alpha) << 24 | u32::from(red) << 16 | u32::from(green) << 8 | u32::from(blue);
    }
}

impl std::fmt::Debug for Cache {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VectorCache")
            .field("trees", &self.trees.len())
            .field("rasters", &self.rasters.len())
            .field("raster_bytes", &self.raster_bytes)
            .finish()
    }
}

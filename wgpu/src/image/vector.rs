//! Parsed and rasterized SVG residency for TAIM.

use crate::core::svg;
use crate::core::{Color, Size};
use crate::image::atlas::{self, Atlas};

use resvg::{tiny_skia, usvg};
use rustc_hash::FxHashMap;
use std::fs;
use std::panic;
#[cfg(feature = "svg-text")]
use std::sync::Arc;

type ColorFilter = Option<[u8; 4]>;
type RasterKey = (u64, u32, u32, ColorFilter);

pub enum Svg {
    Loaded(usvg::Tree),
    NotFound,
}

impl Svg {
    pub fn viewport_dimensions(&self) -> Size<u32> {
        match self {
            Self::Loaded(tree) => {
                let size = tree.size();
                Size::new(size.width() as u32, size.height() as u32)
            }
            Self::NotFound => Size::new(1, 1),
        }
    }
}

#[derive(Debug)]
struct Seen<T> {
    value: T,
    frame: u64,
}

#[derive(Debug)]
enum Rasterized {
    Ready(atlas::Entry),
    Deferred { retry_after: u64 },
    Invalid,
}

/// Two-tier SVG cache: parsed trees and size/color-specific GPU rasters.
#[derive(Debug)]
pub struct Cache {
    parsed: FxHashMap<u64, Seen<Svg>>,
    rasters: FxHashMap<RasterKey, Seen<Rasterized>>,
    frame: u64,
    #[cfg(feature = "svg-text")]
    font_database: Option<Arc<usvg::fontdb::Database>>,
}

impl Default for Cache {
    fn default() -> Self {
        Self {
            parsed: FxHashMap::default(),
            rasters: FxHashMap::default(),
            frame: 1,
            #[cfg(feature = "svg-text")]
            font_database: None,
        }
    }
}

impl Cache {
    pub fn load(&mut self, handle: &svg::Handle) -> &Svg {
        let id = handle.id();
        if self.parsed.contains_key(&id) {
            let cached = self.parsed.get_mut(&id).expect("checked SVG entry");
            cached.frame = self.frame;
            return &cached.value;
        }

        let parsed = self.parse(handle);
        let _ = self.parsed.insert(
            id,
            Seen {
                value: parsed,
                frame: self.frame,
            },
        );
        &self.parsed.get(&id).expect("inserted SVG entry").value
    }

    #[allow(clippy::too_many_arguments)]
    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        handle: &svg::Handle,
        color: Option<Color>,
        requested_size: Size<u32>,
        atlas: &mut Atlas,
    ) -> Option<&atlas::Entry> {
        let raster_size = raster_bucket(requested_size)?;
        let id = handle.id();
        let key = (
            id,
            raster_size.width,
            raster_size.height,
            color.map(Color::into_rgba8),
        );

        let retry_deferred = self.rasters.get(&key).is_some_and(|cached| {
            matches!(
                cached.value,
                Rasterized::Deferred { retry_after } if self.frame >= retry_after
            )
        });
        if retry_deferred {
            let _ = self.rasters.remove(&key);
        }

        if self.rasters.contains_key(&key) {
            if let Some(parsed) = self.parsed.get_mut(&id) {
                parsed.frame = self.frame;
            }
            let cached = self.rasters.get_mut(&key).expect("checked SVG raster");
            cached.frame = self.frame;
            return match &cached.value {
                Rasterized::Ready(entry) => Some(entry),
                Rasterized::Deferred { .. } | Rasterized::Invalid => None,
            };
        }

        let value = match self.rasterize(handle, raster_size, key.3) {
            Some(pixels) => match atlas.upload(
                device,
                encoder,
                belt,
                raster_size.width,
                raster_size.height,
                &pixels,
            ) {
                Some(entry) => Rasterized::Ready(entry),
                None => Rasterized::Deferred {
                    retry_after: self.frame.wrapping_add(1).max(1),
                },
            },
            None => Rasterized::Invalid,
        };
        let _ = self.rasters.insert(
            key,
            Seen {
                value,
                frame: self.frame,
            },
        );

        match &self.rasters.get(&key).expect("inserted SVG raster").value {
            Rasterized::Ready(entry) => Some(entry),
            Rasterized::Deferred { .. } | Rasterized::Invalid => None,
        }
    }

    pub fn trim(&mut self, atlas: &mut Atlas) {
        let frame = self.frame;
        self.rasters.retain(|key, raster| {
            if raster.frame == frame {
                return true;
            }
            if let Rasterized::Ready(entry) = &raster.value {
                atlas.remove(entry);
            }
            log::debug!("TAIM evicting SVG raster {key:?}");
            false
        });
        self.parsed.retain(|_, svg| svg.frame == frame);
        self.frame = self.frame.wrapping_add(1).max(1);
    }

    fn parse(&mut self, handle: &svg::Handle) -> Svg {
        #[cfg(feature = "svg-text")]
        if self.font_database.is_none() {
            let mut database = usvg::fontdb::Database::new();
            database.load_system_fonts();
            self.font_database = Some(Arc::new(database));
        }

        let options = usvg::Options {
            #[cfg(feature = "svg-text")]
            fontdb: Arc::clone(
                self.font_database
                    .as_ref()
                    .expect("TAIM SVG font database initialized"),
            ),
            ..usvg::Options::default()
        };
        let tree = match handle.data() {
            svg::Data::Path(path) => fs::read(path)
                .ok()
                .and_then(|bytes| usvg::Tree::from_data(&bytes, &options).ok()),
            svg::Data::Bytes(bytes) => usvg::Tree::from_data(bytes, &options).ok(),
        };
        tree.map(Svg::Loaded).unwrap_or(Svg::NotFound)
    }

    fn rasterize(
        &mut self,
        handle: &svg::Handle,
        size: Size<u32>,
        color: ColorFilter,
    ) -> Option<Vec<u8>> {
        let tree = match self.load(handle) {
            Svg::Loaded(tree) => tree,
            Svg::NotFound => return None,
        };
        let mut pixmap = tiny_skia::Pixmap::new(size.width, size.height)?;
        let source = tree.size();
        let scale = (size.width as f32 / source.width())
            .min(size.height as f32 / source.height())
            .max(f32::EPSILON);
        let transform = tiny_skia::Transform::from_scale(scale, scale);
        let rendered = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            resvg::render(tree, transform, &mut pixmap.as_mut());
        }));
        if let Err(error) = rendered {
            log::warn!("TAIM SVG rasterization failed for {handle:?}: {error:?}");
            return None;
        }

        let mut pixels = pixmap.take();
        if let Some(color) = color {
            for pixel in pixels.chunks_exact_mut(4) {
                if pixel[3] != 0 {
                    pixel[0] = color[0];
                    pixel[1] = color[1];
                    pixel[2] = color[2];
                }
            }
        }
        Some(pixels)
    }
}

/// Quantizes only larger vectors, avoiding a full CPU reraster for every
/// single-pixel resize tick while leaving icons pixel-exact.
fn raster_bucket(size: Size<u32>) -> Option<Size<u32>> {
    if size.width == 0 || size.height == 0 {
        return None;
    }
    let longest = size.width.max(size.height);
    let step = if longest <= 128 {
        1
    } else if longest <= 1024 {
        4
    } else {
        8
    };
    let bucket = |value: u32| value.saturating_add(step - 1) / step * step;
    Some(Size::new(bucket(size.width), bucket(size.height)))
}

impl std::fmt::Debug for Svg {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Loaded(_) => "Svg::Loaded",
            Self::NotFound => "Svg::NotFound",
        })
    }
}

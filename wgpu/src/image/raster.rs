//! Frame-aware raster residency for TAIM's image renderer.

use crate::core::Size;
use crate::core::image;
use crate::graphics;
use crate::image::atlas::{self, Atlas};

use rustc_hash::FxHashMap;
use std::sync::{Arc, Weak};

pub type Image = graphics::image::Buffer;

#[derive(Debug)]
pub enum Memory {
    Host(Image),
    Device {
        entry: atlas::Entry,
        bind_group: Option<Arc<wgpu::BindGroup>>,
        allocation: Option<Weak<image::Memory>>,
    },
    Error(image::Error),
}

impl Memory {
    pub fn load(handle: &image::Handle) -> Self {
        graphics::image::load(handle)
            .map(Self::Host)
            .unwrap_or_else(Self::Error)
    }

    pub fn dimensions(&self) -> Size<u32> {
        match self {
            Self::Host(image) => Size::new(image.width(), image.height()),
            Self::Device { entry, .. } => entry.size(),
            Self::Error(_) => Size::new(1, 1),
        }
    }

    /// Returns a cheap shared view of host pixels.
    pub fn host(&self) -> Option<Image> {
        match self {
            Self::Host(image) => Some(image.clone()),
            Self::Device { .. } | Self::Error(_) => None,
        }
    }
}

#[derive(Debug)]
struct Resident {
    memory: Memory,
    seen_in_frame: u64,
}

/// Raster entries retained only while used by the current UI frame.
#[derive(Debug)]
pub struct Cache {
    entries: FxHashMap<image::Id, Resident>,
    frame: u64,
}

impl Default for Cache {
    fn default() -> Self {
        Self {
            entries: FxHashMap::default(),
            frame: 1,
        }
    }
}

impl Cache {
    /// Returns resident image data, loading it on a miss with one map probe.
    /// Synchronous image users avoid the separate contains/get_mut lookup
    /// sequence that otherwise hashes the handle ID twice.
    pub fn get_or_load(&mut self, handle: &image::Handle) -> &mut Memory {
        let frame = self.frame;
        let resident = self.entries.entry(handle.id()).or_insert_with(|| Resident {
            memory: Memory::load(handle),
            seen_in_frame: frame,
        });
        resident.seen_in_frame = frame;
        &mut resident.memory
    }

    pub fn get_mut(&mut self, handle: &image::Handle) -> Option<&mut Memory> {
        let resident = self.entries.get_mut(&handle.id())?;
        resident.seen_in_frame = self.frame;
        Some(&mut resident.memory)
    }

    pub fn insert(&mut self, handle: &image::Handle, memory: Memory) {
        let _ = self.entries.insert(
            handle.id(),
            Resident {
                memory,
                seen_in_frame: self.frame,
            },
        );
    }

    pub fn contains(&self, handle: &image::Handle) -> bool {
        self.entries.contains_key(&handle.id())
    }

    pub fn trim(&mut self, atlas: &mut Atlas, on_isolated_drop: impl Fn(Arc<wgpu::BindGroup>)) {
        let frame = self.frame;
        self.entries.retain(|id, resident| {
            let externally_allocated = matches!(
                &resident.memory,
                Memory::Device {
                    allocation: Some(allocation),
                    ..
                } if allocation.strong_count() > 0
            );
            let keep = resident.seen_in_frame == frame || externally_allocated;
            if keep {
                return true;
            }

            log::debug!("TAIM evicting raster image {id:?}");
            if let Memory::Device {
                entry, bind_group, ..
            } = &mut resident.memory
            {
                if let Some(binding) = bind_group.take() {
                    on_isolated_drop(binding);
                } else {
                    atlas.remove(entry);
                }
            }
            false
        });

        self.frame = self.frame.wrapping_add(1).max(1);
    }
}

use super::batch::{Batch, Item};
use super::cache::{Cache, Id};
use super::{COLOR_MODE, prepare};
use crate::core::{Rectangle, Transformation};
use crate::graphics::cache;
use crate::graphics::text::cache::Cache as BufferCache;

use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::hash_map;
use std::sync::{Arc, Weak};
use taim_text_gpu as gpu;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Signature {
    content_version: usize,
    atlas_version: usize,
    transformation: Transformation,
    bounds: Rectangle,
}

struct Upload {
    group: cache::Group,
    renderer: gpu::TextRenderer,
    buffer_cache: BufferCache,
    prepared: Option<Signature>,
    seen_content_version: usize,
    text: Weak<[super::Text]>,
    _atlas: Weak<()>,
}

#[derive(Default)]
pub struct Storage {
    groups: FxHashMap<cache::Group, Group>,
    uploads: FxHashMap<Id, Upload>,
}

struct Group {
    atlas: gpu::TextAtlas,
    version: usize,
    handle: Arc<()>,
}

impl Storage {
    /// Starts one retained-text batch against a coherent atlas allocation
    /// epoch. Membership changes invalidate the group before any upload is
    /// read, so every visible cache is rebuilt before recycled glyphs render.
    pub(super) fn begin_batch(&mut self, batch: &Batch) {
        let mut changed = FxHashSet::default();

        for item in batch {
            let Item::Cached { cache, .. } = item else {
                continue;
            };

            let content_changed = self
                .uploads
                .get(&cache.id)
                .is_none_or(|upload| upload.seen_content_version != cache.version);

            if content_changed && self.groups.contains_key(&cache.group) {
                let _ = changed.insert(cache.group);
            }
        }

        for id in changed {
            if let Some(group) = self.groups.get_mut(&id) {
                group.atlas.trim();
                for upload in self.uploads.values().filter(|upload| upload.group == id) {
                    group.atlas.retain(&upload.renderer);
                }
                group.version = group.version.wrapping_add(1);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        viewport: &gpu::Viewport,
        format: wgpu::TextureFormat,
        state: &gpu::Cache,
        cache: &Cache,
        swash: &mut crate::graphics::text::cosmic_text::SwashCache,
        transformation: Transformation,
        bounds: Rectangle,
    ) {
        let group_count = self.groups.len();
        let group = self.groups.entry(cache.group).or_insert_with(|| {
            log::debug!(
                "New TAIM text atlas: {:?} (total: {})",
                cache.group,
                group_count + 1
            );

            Group {
                atlas: gpu::TextAtlas::with_color_mode(device, queue, state, format, COLOR_MODE),
                version: 0,
                handle: Arc::new(()),
            }
        });
        let signature = Signature {
            content_version: cache.version,
            atlas_version: group.version,
            transformation,
            bounds,
        };

        match self.uploads.entry(cache.id) {
            hash_map::Entry::Occupied(entry) => {
                let upload = entry.into_mut();
                upload.seen_content_version = cache.version;

                if upload.prepared == Some(signature) {
                    upload.text = Arc::downgrade(&cache.text);
                    return;
                }

                match prepare::run(
                    device,
                    queue,
                    viewport,
                    &mut upload.renderer,
                    &mut group.atlas,
                    &mut upload.buffer_cache,
                    swash,
                    &cache.text,
                    bounds,
                    transformation,
                ) {
                    Ok(()) => {
                        upload.prepared = Some(signature);
                        upload.buffer_cache.trim();
                    }
                    Err(error) => {
                        upload.prepared = None;
                        log::warn!("TAIM retained text preparation deferred: {error}");
                    }
                }

                upload.text = Arc::downgrade(&cache.text);
            }
            hash_map::Entry::Vacant(entry) => {
                let mut renderer = gpu::TextRenderer::new(
                    &mut group.atlas,
                    device,
                    wgpu::MultisampleState::default(),
                    None,
                );
                let mut buffer_cache = BufferCache::new();
                let prepared = match prepare::run(
                    device,
                    queue,
                    viewport,
                    &mut renderer,
                    &mut group.atlas,
                    &mut buffer_cache,
                    swash,
                    &cache.text,
                    bounds,
                    transformation,
                ) {
                    Ok(()) => Some(signature),
                    Err(error) => {
                        log::warn!("TAIM retained text preparation deferred: {error}");
                        None
                    }
                };

                let _ = entry.insert(Upload {
                    group: cache.group,
                    renderer,
                    buffer_cache,
                    prepared,
                    seen_content_version: cache.version,
                    text: Arc::downgrade(&cache.text),
                    _atlas: Arc::downgrade(&group.handle),
                });

                log::debug!(
                    "New TAIM text upload: {} (total: {})",
                    cache.id.0,
                    self.uploads.len()
                );
            }
        }
    }

    pub(super) fn render<'a>(
        &'a self,
        cache: &Cache,
        viewport: &'a gpu::Viewport,
        render_pass: &mut wgpu::RenderPass<'a>,
    ) {
        if cache.text.is_empty() {
            return;
        }

        let Some(group) = self.groups.get(&cache.group) else {
            return;
        };
        let Some(upload) = self.uploads.get(&cache.id) else {
            return;
        };

        upload.renderer.render(&group.atlas, viewport, render_pass);
    }

    pub fn trim(&mut self) {
        self.uploads
            .retain(|_, upload| upload.text.strong_count() > 0);

        self.groups.retain(|id, group| {
            if Arc::weak_count(&group.handle) == 0 {
                log::debug!("Dropping TAIM text atlas: {id:?}");
                false
            } else {
                true
            }
        });
    }
}

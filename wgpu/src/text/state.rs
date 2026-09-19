use super::batch::{Batch, Item};
use super::pipeline::{Id as PipelineId, Pipeline, Viewport};
use super::{Storage, prepare};
use crate::core::{Rectangle, Transformation};
use crate::graphics::text::cache::{self as text_cache, Cache as BufferCache};

use rustc_hash::FxHashMap;
use std::sync::Weak;
use taim_text_gpu as gpu;

struct PreparedPipeline {
    lifetime: Weak<()>,
    renderers: Vec<gpu::TextRenderer>,
    prepare_layer: usize,
    cache: BufferCache,
    storage: Storage,
}

impl PreparedPipeline {
    fn new(pipeline: &Pipeline) -> Self {
        Self {
            lifetime: pipeline.lifetime(),
            renderers: Vec::new(),
            prepare_layer: 0,
            cache: BufferCache::new(),
            storage: Storage::default(),
        }
    }
}

pub struct State {
    pipelines: FxHashMap<PipelineId, PreparedPipeline>,
    active: Option<PipelineId>,
    swash: crate::graphics::text::cosmic_text::SwashCache,
}

impl Default for State {
    fn default() -> Self {
        Self {
            pipelines: FxHashMap::default(),
            active: None,
            swash: crate::graphics::text::cosmic_text::SwashCache::new(),
        }
    }
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        pipeline: &Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        viewport: &Viewport,
        batch: &Batch,
        layer_bounds: Rectangle,
        layer_transformation: Transformation,
    ) {
        self.active = Some(pipeline.id);
        let state = self
            .pipelines
            .entry(pipeline.id)
            .or_insert_with(|| PreparedPipeline::new(pipeline));
        state.storage.begin_batch(batch);

        let mut atlas = pipeline
            .atlas
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        for item in batch {
            match item {
                Item::Group {
                    transformation,
                    text,
                } => {
                    if state.renderers.len() <= state.prepare_layer {
                        state.renderers.push(gpu::TextRenderer::new(
                            &mut atlas,
                            device,
                            wgpu::MultisampleState::default(),
                            None,
                        ));
                    }

                    let renderer = &mut state.renderers[state.prepare_layer];
                    if let Err(error) = prepare::run(
                        device,
                        queue,
                        &viewport.0,
                        renderer,
                        &mut atlas,
                        &mut state.cache,
                        &mut self.swash,
                        text,
                        layer_bounds * layer_transformation,
                        layer_transformation * *transformation,
                    ) {
                        log::warn!("TAIM text layer preparation skipped: {error}");
                    }

                    // One renderer slot belongs to every group, including a
                    // failed/empty one. This keeps render indexing stable and
                    // prevents a later group from drawing in the wrong z-slot.
                    state.prepare_layer += 1;
                }
                Item::Cached {
                    transformation,
                    cache,
                } => state.storage.prepare(
                    device,
                    queue,
                    &viewport.0,
                    pipeline.format,
                    &pipeline.cache,
                    cache,
                    &mut self.swash,
                    layer_transformation * *transformation,
                    layer_bounds * layer_transformation,
                ),
            }
        }
    }

    pub fn render<'a>(
        &'a self,
        pipeline: &'a Pipeline,
        viewport: &'a Viewport,
        start: usize,
        batch: &'a Batch,
        bounds: Rectangle<u32>,
        render_pass: &mut wgpu::RenderPass<'a>,
    ) -> usize {
        let Some(state) = self.pipelines.get(&pipeline.id) else {
            return 0;
        };
        let atlas = pipeline
            .atlas
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut layer_count = 0;

        render_pass.set_scissor_rect(bounds.x, bounds.y, bounds.width, bounds.height);

        for item in batch {
            match item {
                Item::Group { .. } => {
                    if let Some(renderer) = state.renderers.get(start + layer_count) {
                        renderer.render(&atlas, &viewport.0, render_pass);
                    } else {
                        log::warn!("TAIM text renderer slot missing; empty layer emitted");
                    }
                    layer_count += 1;
                }
                Item::Cached { cache, .. } => {
                    state.storage.render(cache, &viewport.0, render_pass);
                }
            }
        }

        layer_count
    }

    pub fn trim(&mut self) {
        if let Some(active) = self.active.take() {
            if let Some(state) = self.pipelines.get_mut(&active) {
                state.cache.trim();
                state.storage.trim();
                state.prepare_layer = 0;
            }
        }

        self.pipelines
            .retain(|_, state| state.lifetime.strong_count() > 0);
    }

    pub fn cache_statistics(&self) -> text_cache::Statistics {
        self.pipelines
            .values()
            .fold(text_cache::Statistics::default(), |mut total, state| {
                let statistics = state.cache.statistics();
                total.hits = total.hits.saturating_add(statistics.hits);
                total.misses = total.misses.saturating_add(statistics.misses);
                total.evicted_bytes = total.evicted_bytes.saturating_add(statistics.evicted_bytes);
                total.total_miss_nanos = total
                    .total_miss_nanos
                    .saturating_add(statistics.total_miss_nanos);
                total
            })
    }
}

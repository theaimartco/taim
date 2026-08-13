#[path = "pipeline_cache.rs"]
pub mod pipeline_cache;

use crate::graphics::{Antialiasing, Shell};
use crate::primitive;
use crate::quad;
use crate::text;
use crate::triangle;

use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct Engine {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) format: wgpu::TextureFormat,

    pub(crate) pipeline_cache: Arc<RwLock<pipeline_cache::PipelineCache>>,

    pub(crate) quad_pipeline: quad::Pipeline,
    pub(crate) text_pipeline: text::Pipeline,
    pub(crate) triangle_pipeline: triangle::Pipeline,
    #[cfg(any(feature = "image", feature = "svg"))]
    pub(crate) image_pipeline: crate::image::Pipeline,
    pub(crate) effect_pipeline: crate::effects::pipeline::Pipeline,
    pub(crate) primitive_storage: Arc<RwLock<primitive::Storage>>,
    _shell: Shell,
}

impl Engine {
    pub fn new(
        _adapter: &wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        antialiasing: Option<Antialiasing>, // TODO: Initialize AA pipelines lazily
        shell: Shell,
    ) -> Self {
        let mut cache = pipeline_cache::PipelineCache::new(32);
        cache.prewarm(format, &device, &queue);

        let quad_pipeline = match cache.get_or_create(
            pipeline_cache::PipelineKey {
                kind: pipeline_cache::PipelineKind::Quad,
                format,
                antialiasing: None,
                backend: None,
            },
            &device,
            &queue,
        ) {
            pipeline_cache::CachedPipeline::Quad(p) => p,
            _ => unreachable!(),
        };

        let text_pipeline = match cache.get_or_create(
            pipeline_cache::PipelineKey {
                kind: pipeline_cache::PipelineKind::Text,
                format,
                antialiasing: None,
                backend: None,
            },
            &device,
            &queue,
        ) {
            pipeline_cache::CachedPipeline::Text(p) => p,
            _ => unreachable!(),
        };

        let triangle_pipeline = match cache.get_or_create(
            pipeline_cache::PipelineKey {
                kind: pipeline_cache::PipelineKind::Triangle,
                format,
                antialiasing,
                backend: None,
            },
            &device,
            &queue,
        ) {
            pipeline_cache::CachedPipeline::Triangle(p) => p,
            _ => unreachable!(),
        };

        #[cfg(any(feature = "image", feature = "svg"))]
        let image_pipeline = {
            let backend = _adapter.get_info().backend;
            match cache.get_or_create(
                pipeline_cache::PipelineKey {
                    kind: pipeline_cache::PipelineKind::Image,
                    format,
                    antialiasing: None,
                    backend: Some(backend),
                },
                &device,
                &queue,
            ) {
                pipeline_cache::CachedPipeline::Image(p) => p,
                _ => unreachable!(),
            }
        };

        Self {
            format,

            pipeline_cache: Arc::new(RwLock::new(cache)),

            quad_pipeline,
            text_pipeline,
            triangle_pipeline,
            effect_pipeline: crate::effects::pipeline::Pipeline::new(&device, format),

            #[cfg(any(feature = "image", feature = "svg"))]
            image_pipeline,
            


            primitive_storage: Arc::new(RwLock::new(primitive::Storage::default())),

            device,
            queue,
            _shell: shell,
        }
    }

    #[cfg(any(feature = "image", feature = "svg"))]
    pub fn create_image_cache(&self) -> crate::image::Cache {
        self.image_pipeline
            .create_cache(&self.device, &self.queue, &self._shell)
    }

    pub fn trim(&mut self) {
        self.text_pipeline.trim();
        self.effect_pipeline.trim();

        self.primitive_storage
            .write()
            .expect("primitive storage should be writable")
            .trim();
    }
}

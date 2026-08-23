use crate::graphics::Antialiasing;
use crate::quad;
use crate::text;
use crate::triangle;

use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PipelineKind {
    Quad,
    Text,
    Triangle,
    #[cfg(any(feature = "image", feature = "svg"))]
    Image,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PipelineKey {
    pub kind: PipelineKind,
    pub format: wgpu::TextureFormat,
    pub antialiasing: Option<Antialiasing>,
    pub backend: Option<wgpu::Backend>,
}

#[derive(Clone)]
pub enum CachedPipeline {
    Quad(quad::Pipeline),
    Text(text::Pipeline),
    Triangle(triangle::Pipeline),
    #[cfg(any(feature = "image", feature = "svg"))]
    Image(crate::image::Pipeline),
}

pub struct PipelineCache {
    pipelines: HashMap<PipelineKey, CachedPipeline>,
    access_times: HashMap<PipelineKey, Instant>,
    capacity: usize,
}

impl PipelineCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            pipelines: HashMap::new(),
            access_times: HashMap::new(),
            capacity,
        }
    }

    pub fn get_or_create(
        &mut self,
        key: PipelineKey,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> CachedPipeline {
        if let Some(pipeline) = self.pipelines.get(&key) {
            let _ = self.access_times.insert(key.clone(), Instant::now());
            return pipeline.clone();
        }

        let pipeline = match key.kind {
            PipelineKind::Quad => CachedPipeline::Quad(quad::Pipeline::new(device, key.format)),
            PipelineKind::Text => {
                CachedPipeline::Text(text::Pipeline::new(device, queue, key.format))
            }
            PipelineKind::Triangle => CachedPipeline::Triangle(triangle::Pipeline::new(
                device,
                key.format,
                key.antialiasing,
            )),
            #[cfg(any(feature = "image", feature = "svg"))]
            PipelineKind::Image => {
                let backend = key
                    .backend
                    .expect("Backend must be provided for image pipeline");
                CachedPipeline::Image(crate::image::Pipeline::new(device, key.format, backend))
            }
        };

        self.evict_if_needed();

        let _ = self.pipelines.insert(key.clone(), pipeline.clone());
        let _ = self.access_times.insert(key.clone(), Instant::now());

        pipeline
    }

    pub fn prewarm(
        &mut self,
        format: wgpu::TextureFormat,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        let quad_key = PipelineKey {
            kind: PipelineKind::Quad,
            format,
            antialiasing: None,
            backend: None,
        };
        let _ = self.get_or_create(quad_key, device, queue);

        let text_key = PipelineKey {
            kind: PipelineKind::Text,
            format,
            antialiasing: None,
            backend: None,
        };
        let _ = self.get_or_create(text_key, device, queue);
    }

    fn evict_if_needed(&mut self) {
        if self.pipelines.len() >= self.capacity
            && let Some((lru_key, _)) = self.access_times.iter().min_by_key(|&(_, &time)| time)
        {
            let key_to_remove = lru_key.clone();
            let _ = self.pipelines.remove(&key_to_remove);
            let _ = self.access_times.remove(&key_to_remove);
        }
    }
}

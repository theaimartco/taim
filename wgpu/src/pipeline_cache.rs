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

#[derive(Debug, Default, Clone)]
pub struct Telemetry {
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub creation_time_ms: f64,
}

pub struct PipelineCache {
    pipelines: HashMap<PipelineKey, CachedPipeline>,
    access_times: HashMap<PipelineKey, Instant>,
    telemetry: Telemetry,
    resource_generation: u64,
    capacity: usize,
}

impl PipelineCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            pipelines: HashMap::new(),
            access_times: HashMap::new(),
            telemetry: Telemetry::default(),
            resource_generation: 0,
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
            self.telemetry.cache_hits += 1;
            let _ = self.access_times.insert(key.clone(), Instant::now());
            return pipeline.clone();
        }

        self.telemetry.cache_misses += 1;
        let start = Instant::now();

        let pipeline = match key.kind {
            PipelineKind::Quad => {
                CachedPipeline::Quad(quad::Pipeline::new(device, key.format))
            }
            PipelineKind::Text => {
                CachedPipeline::Text(text::Pipeline::new(device, queue, key.format))
            }
            PipelineKind::Triangle => {
                CachedPipeline::Triangle(triangle::Pipeline::new(device, key.format, key.antialiasing))
            }
            #[cfg(any(feature = "image", feature = "svg"))]
            PipelineKind::Image => {
                let backend = key.backend.expect("Backend must be provided for image pipeline");
                CachedPipeline::Image(crate::image::Pipeline::new(device, key.format, backend))
            }
        };

        self.telemetry.creation_time_ms += start.elapsed().as_secs_f64() * 1000.0;

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

    pub fn invalidate_all(&mut self) {
        self.pipelines.clear();
        self.access_times.clear();
        self.resource_generation += 1;
    }

    fn evict_if_needed(&mut self) {
        if self.pipelines.len() >= self.capacity {
            if let Some((lru_key, _)) = self.access_times.iter().min_by_key(|&(_, &time)| time) {
                let key_to_remove = lru_key.clone();
                let _ = self.pipelines.remove(&key_to_remove);
                let _ = self.access_times.remove(&key_to_remove);
            }
        }
    }

    pub fn telemetry(&self) -> &Telemetry {
        &self.telemetry
    }

    pub fn resource_generation(&self) -> u64 {
        self.resource_generation
    }
}

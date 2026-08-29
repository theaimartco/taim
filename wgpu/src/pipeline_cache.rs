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
    /// A stand-in with no GPU resources, for tests that exercise the cache's
    /// bookkeeping rather than pipeline construction.
    #[cfg(test)]
    Marker,
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

impl PipelineCache {
    /// Releases per-frame resources held by every cached pipeline.
    ///
    /// Only the text pipeline holds any: a glyph atlas, which is per-format,
    /// so a renderer that has drawn into two formats has two atlases to trim.
    pub fn trim(&mut self) {
        for pipeline in self.pipelines.values_mut() {
            if let CachedPipeline::Text(text) = pipeline {
                text.trim();
            }
        }
    }

    /// Returns how many pipelines are currently cached.
    ///
    /// Test-only, and deliberately not public API: the format-flexibility
    /// tests need to assert that a second format *adds* pipelines rather than
    /// replacing the first's — the regression that would quietly reintroduce
    /// a single-format engine — and nothing in production has a reason to ask.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.pipelines.len()
    }

    /// Returns whether any pipeline is cached. Test-only; see [`Self::len`].
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.pipelines.is_empty()
    }

    /// Returns whether a pipeline for `key` is built. Test-only; see
    /// [`Self::len`].
    #[cfg(test)]
    pub fn contains(&self, key: &PipelineKey) -> bool {
        self.pipelines.contains_key(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(kind: PipelineKind, format: wgpu::TextureFormat) -> PipelineKey {
        PipelineKey {
            kind,
            format,
            antialiasing: None,
            backend: None,
        }
    }

    #[test]
    fn the_key_distinguishes_every_axis_that_affects_compatibility() {
        // A cache hit must never return a pipeline the GPU will reject. The
        // axes below are the ones that make two pipelines incompatible in
        // this renderer: the kind, the attachment format, the sample count
        // (carried by `antialiasing`, and only the triangle pipeline is
        // multisampled), and the backend (which the image pipeline needs).
        //
        // Blend state and depth format are deliberately absent: blend is
        // fixed per kind, and nothing here uses a depth attachment. Adding
        // key axes for state that cannot vary would be cost without cover.
        let base = key(PipelineKind::Quad, wgpu::TextureFormat::Rgba8Unorm);

        let different_format = key(PipelineKind::Quad, wgpu::TextureFormat::Rgba16Float);
        let different_kind = key(PipelineKind::Text, wgpu::TextureFormat::Rgba8Unorm);
        let different_aa = PipelineKey {
            antialiasing: Some(Antialiasing::MSAAx4),
            ..base.clone()
        };
        let different_backend = PipelineKey {
            backend: Some(wgpu::Backend::Vulkan),
            ..base.clone()
        };

        for other in [
            different_format,
            different_kind,
            different_aa,
            different_backend,
        ] {
            assert_ne!(base, other, "two incompatible pipelines share a cache key");
        }
    }

    #[test]
    fn an_srgb_format_is_not_interchangeable_with_its_linear_twin() {
        // The pair most likely to be treated as equivalent, and the one where
        // doing so silently changes whether the hardware applies a transfer.
        assert_ne!(
            key(PipelineKind::Quad, wgpu::TextureFormat::Bgra8Unorm),
            key(PipelineKind::Quad, wgpu::TextureFormat::Bgra8UnormSrgb),
        );
    }

    #[test]
    fn a_new_format_adds_pipelines_rather_than_replacing_them() {
        // The regression that would reintroduce a single-format engine: a
        // cache that evicted the previous format on every switch would make a
        // window dragged between two displays rebuild its pipelines twice per
        // move, and would hide the fact that both formats are live.
        //
        // Uses the cache's bookkeeping directly rather than a device, so it
        // runs everywhere; `tests/gpu/target_formats.rs` covers the same
        // property against a real GPU.
        let mut cache = PipelineCache::new(32);

        assert!(cache.is_empty());

        let first = key(PipelineKind::Quad, wgpu::TextureFormat::Bgra8UnormSrgb);
        let second = key(PipelineKind::Quad, wgpu::TextureFormat::Rgba16Float);

        let _ = cache
            .pipelines
            .insert(first.clone(), CachedPipeline::Marker);
        assert_eq!(cache.len(), 1);
        assert!(cache.contains(&first));
        assert!(!cache.contains(&second));

        let _ = cache
            .pipelines
            .insert(second.clone(), CachedPipeline::Marker);
        assert_eq!(cache.len(), 2, "the second format displaced the first");
        assert!(cache.contains(&first), "the first format was evicted");
        assert!(cache.contains(&second));
    }
}

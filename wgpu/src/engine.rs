#[path = "pipeline_cache.rs"]
pub mod pipeline_cache;

use crate::graphics::{Antialiasing, Shell};

use crate::primitive;
use crate::quad;
use crate::text;
use crate::triangle;
use rustc_hash::FxHashMap;

use std::sync::{Arc, RwLock};

/// Device-global rendering resources, shared by every window and every
/// render target.
///
/// # What belongs here, and what does not
///
/// An `Engine` owns things whose identity is the *device*: the device and
/// queue themselves, the shared primitive storage, and the pipeline cache.
///
/// It deliberately does **not** own a render pipeline. A pipeline's identity
/// includes the format of the attachment it writes to, which is a property of
/// the *target*, not of the device. Storing one set of pipelines here is what
/// made `Engine` a single-format object: rendering into any other format was
/// not a wrong colour but a render-pass validation error, and a window moving
/// between an SDR and an HDR display is exactly that.
///
/// Pipelines are resolved per target format through [`Engine::pipelines`],
/// which is backed by a retained [`pipeline_cache::PipelineCache`]. The cache
/// was always here and already keyed by format; it was previously constructed
/// inside `new`, used for one format, and dropped.
#[derive(Clone)]
pub struct Engine {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,

    /// The format the surface this engine was created for prefers.
    ///
    /// A default and a starting point, not a constraint: see
    /// [`Engine::pipelines`].
    pub(crate) format: wgpu::TextureFormat,

    /// Antialiasing, which is part of a pipeline's compatibility state.
    pub(crate) antialiasing: Option<Antialiasing>,

    /// The adapter's backend, which the image pipeline's key requires.
    pub(crate) backend: Option<wgpu::Backend>,

    /// Format-keyed pipelines, retained for the engine's lifetime.
    pipelines: Arc<RwLock<pipeline_cache::PipelineCache>>,

    /// Effect pipelines, one per target format.
    ///
    /// Separate from `pipelines` because an effect pipeline is not a single
    /// `wgpu::RenderPipeline` but a small collection of them plus a mask
    /// atlas and a uniform buffer, so it does not fit `CachedPipeline`.
    effects: Arc<RwLock<FxHashMap<wgpu::TextureFormat, crate::effects::pipeline::Pipeline>>>,

    pub(crate) primitive_storage: Arc<RwLock<primitive::Storage>>,
    _shell: Shell,
}

/// Every pipeline needed to draw a frame into one particular format.
///
/// Cloning is cheap: each field is a handle.
#[derive(Clone)]
pub struct Pipelines {
    /// The attachment format these pipelines are compatible with.
    pub format: wgpu::TextureFormat,
    /// The quad pipeline.
    pub quad: quad::Pipeline,
    /// The text pipeline, including its format-specific glyph atlas.
    pub text: text::Pipeline,
    /// The triangle pipeline, including its MSAA variant.
    pub triangle: triangle::Pipeline,
    /// The image pipeline.
    #[cfg(any(feature = "image", feature = "svg"))]
    pub image: crate::image::Pipeline,
    /// The post-process effect pipeline.
    pub effect: crate::effects::pipeline::Pipeline,
}

impl Engine {
    /// Creates an [`Engine`] for `device`, preferring `format`.
    pub fn new(
        adapter: &wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        antialiasing: Option<Antialiasing>,
        shell: Shell,
    ) -> Self {
        let cache = pipeline_cache::PipelineCache::new(32);
        let backend = Some(adapter.get_info().backend);

        let engine = Self {
            format,
            antialiasing,
            backend,
            pipelines: Arc::new(RwLock::new(cache)),
            effects: Arc::new(RwLock::new(FxHashMap::default())),
            primitive_storage: Arc::new(RwLock::new(primitive::Storage::default())),
            device,
            queue,
            _shell: shell,
        };

        // Build the preferred format's pipelines up front, so the first frame
        // does not pay for them and a misconfigured format fails here rather
        // than inside the first render pass. Any other format is built on
        // first use.
        let _ = engine.pipelines(format);

        engine
    }

    /// Returns the pipelines compatible with an attachment of `format`,
    /// building them on first use.
    ///
    /// This is the whole of the format-flexibility contract. A caller draws
    /// into a target, asks for that target's format, and gets pipelines that
    /// are compatible with it by construction — rather than inheriting one
    /// format chosen when the engine was built.
    pub fn pipelines(&self, format: wgpu::TextureFormat) -> Pipelines {
        let mut cache = self
            .pipelines
            .write()
            .expect("the pipeline cache lock is never held across a panic");

        let key = |kind| pipeline_cache::PipelineKey {
            kind,
            format,
            antialiasing: None,
            backend: self.backend,
        };

        let pipeline_cache::CachedPipeline::Quad(quad) = cache.get_or_create(
            key(pipeline_cache::PipelineKind::Quad),
            &self.device,
            &self.queue,
        ) else {
            unreachable!("the cache returns the kind it was asked for")
        };

        let pipeline_cache::CachedPipeline::Text(text) = cache.get_or_create(
            key(pipeline_cache::PipelineKind::Text),
            &self.device,
            &self.queue,
        ) else {
            unreachable!("the cache returns the kind it was asked for")
        };

        // Antialiasing is part of the triangle pipeline's compatibility state
        // and of no other kind's, because it is the only pipeline this crate
        // multisamples.
        let pipeline_cache::CachedPipeline::Triangle(triangle) = cache.get_or_create(
            pipeline_cache::PipelineKey {
                kind: pipeline_cache::PipelineKind::Triangle,
                format,
                antialiasing: self.antialiasing,
                backend: self.backend,
            },
            &self.device,
            &self.queue,
        ) else {
            unreachable!("the cache returns the kind it was asked for")
        };

        #[cfg(any(feature = "image", feature = "svg"))]
        let image = {
            let pipeline_cache::CachedPipeline::Image(image) = cache.get_or_create(
                key(pipeline_cache::PipelineKind::Image),
                &self.device,
                &self.queue,
            ) else {
                unreachable!("the cache returns the kind it was asked for")
            };

            image
        };

        drop(cache);

        Pipelines {
            format,
            quad,
            text,
            triangle,
            #[cfg(any(feature = "image", feature = "svg"))]
            image,
            effect: self.effect_pipeline(format),
        }
    }

    /// Returns an effect pipeline for `format`, building it on first use.
    ///
    /// The returned pipeline is a clone, and `Pipeline::clone` deliberately
    /// drops the per-frame work queue — a caller must keep the one it was
    /// given for the whole frame rather than re-resolving mid-frame.
    fn effect_pipeline(&self, format: wgpu::TextureFormat) -> crate::effects::pipeline::Pipeline {
        let mut effects = self
            .effects
            .write()
            .expect("the effect pipeline lock is never held across a panic");

        effects
            .entry(format)
            .or_insert_with(|| crate::effects::pipeline::Pipeline::new(&self.device, format))
            .clone()
    }

    #[cfg(any(feature = "image", feature = "svg"))]
    pub fn create_image_cache(&self) -> crate::image::Cache {
        self.pipelines(self.format)
            .image
            .create_cache(&self.device, &self.queue, &self._shell)
    }

    /// Releases what every format's pipelines are holding between frames.
    ///
    /// Trims every format, not just the preferred one: a window that has
    /// moved between an SDR and an HDR display leaves a populated glyph atlas
    /// behind on the format it left, and never trimming it is a leak that
    /// only shows up on machines with two different monitors.
    pub fn trim(&mut self) {
        {
            let mut cache = self
                .pipelines
                .write()
                .expect("the pipeline cache lock is never held across a panic");

            cache.trim();
        }

        {
            let mut effects = self
                .effects
                .write()
                .expect("the effect pipeline lock is never held across a panic");

            for pipeline in effects.values_mut() {
                pipeline.trim();
            }
        }

        self.primitive_storage
            .write()
            .expect("primitive storage should be writable")
            .trim();
    }
}

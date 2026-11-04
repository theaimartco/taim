//! TAIM's native GPU text raster cache and renderer.
//!
//! The engine keeps shaping in `cosmic-text`, rasterizes each distinct physical
//! glyph once, and submits one compact instance buffer per text layer. Its two
//! atlases grow geometrically up to a hard device-aware limit. When full, only
//! glyphs not referenced by the layer being prepared are eligible for eviction.

use bytemuck::{Pod, Zeroable};
use cosmic_text::{CacheKey, FontSystem, LayoutRunIter, SwashCache, SwashContent};
use rustc_hash::{FxHashMap, FxHashSet};
use std::borrow::Cow;
use std::fmt;
use std::sync::{Arc, Mutex};
use taim_atlas::{Allocator as AtlasAllocator, Region};

const INITIAL_ATLAS_SIDE: u32 = 512;
const MAX_ATLAS_SIDE: u32 = 2048;
const STALE_FRAME_WINDOW: u64 = 120;
const INITIAL_INSTANCE_CAPACITY: usize = 256;

/// The color-management policy used for bitmap and mask glyphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    /// Store bitmap glyphs in an sRGB texture and linearize mask colors.
    Accurate,
    /// Preserve browser-compatible gamma-encoded values.
    Web,
}

/// Physical output dimensions used to transform pixels into clip space.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub struct Resolution {
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct ViewportUniform {
    size: [u32; 2],
    padding: [u32; 2],
}

/// A small uniform buffer containing the current physical output size.
pub struct Viewport {
    resolution: Resolution,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl Viewport {
    /// Creates a viewport attached to a shared text pipeline cache.
    pub fn new(device: &wgpu::Device, cache: &Cache) -> Self {
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taim_text_gpu.viewport"),
            size: std::mem::size_of::<ViewportUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = cache.create_viewport_bind_group(device, &uniform);

        Self {
            resolution: Resolution {
                width: 0,
                height: 0,
            },
            uniform,
            bind_group,
        }
    }

    /// Updates the output size only when it actually changes.
    pub fn update(&mut self, queue: &wgpu::Queue, resolution: Resolution) {
        if self.resolution == resolution {
            return;
        }

        self.resolution = resolution;
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&ViewportUniform {
                size: [resolution.width, resolution.height],
                padding: [0; 2],
            }),
        );
    }

    fn resolution(&self) -> Resolution {
        self.resolution
    }
}

/// Shared bind-group layouts, shader, and render-pipeline cache.
#[derive(Clone)]
pub struct Cache(Arc<Shared>);

struct Shared {
    atlas_layout: wgpu::BindGroupLayout,
    viewport_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
    pipelines: Mutex<Vec<PipelineEntry>>,
}

struct PipelineEntry {
    format: wgpu::TextureFormat,
    multisample: wgpu::MultisampleState,
    depth_stencil: Option<wgpu::DepthStencilState>,
    pipeline: wgpu::RenderPipeline,
}

impl Cache {
    /// Creates the device-scoped immutable text resources.
    pub fn new(device: &wgpu::Device) -> Self {
        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("taim_text_gpu.atlas_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let viewport_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("taim_text_gpu.viewport_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("taim_text_gpu.pipeline_layout"),
            bind_group_layouts: &[Some(&atlas_layout), Some(&viewport_layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("taim_text_gpu.shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
        });

        // On the web wgpu's handles are neither `Send` nor `Sync`; a page
        // runs on one thread, so sharing them through an `Arc` is sound.
        #[cfg_attr(target_arch = "wasm32", allow(clippy::arc_with_non_send_sync))]
        let shared = Arc::new(Shared {
            atlas_layout,
            viewport_layout,
            pipeline_layout,
            shader,
            pipelines: Mutex::new(Vec::new()),
        });
        Self(shared)
    }

    fn create_viewport_bind_group(
        &self,
        device: &wgpu::Device,
        uniform: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("taim_text_gpu.viewport_bind_group"),
            layout: &self.0.viewport_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        })
    }

    fn create_atlas_bind_group(
        &self,
        device: &wgpu::Device,
        color: &wgpu::TextureView,
        mask: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("taim_text_gpu.atlas_bind_group"),
            layout: &self.0.atlas_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(color),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(mask),
                },
            ],
        })
    }

    fn render_pipeline(
        &self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        multisample: wgpu::MultisampleState,
        depth_stencil: Option<wgpu::DepthStencilState>,
    ) -> wgpu::RenderPipeline {
        let mut pipelines = self.0.pipelines.lock().expect("Lock TAIM text pipelines");

        if let Some(entry) = pipelines.iter().find(|entry| {
            entry.format == format
                && entry.multisample == multisample
                && entry.depth_stencil == depth_stencil
        }) {
            return entry.pipeline.clone();
        }

        let attributes = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Sint32x2,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint32x2,
                offset: 8,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint32x2,
                offset: 16,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 24,
                shader_location: 3,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint32,
                offset: 40,
                shader_location: 4,
            },
        ];
        let buffers = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GlyphInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &attributes,
        }];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("taim_text_gpu.pipeline"),
            layout: Some(&self.0.pipeline_layout),
            vertex: wgpu::VertexState {
                module: &self.0.shader,
                entry_point: Some("vertex"),
                buffers: &buffers,
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.0.shader,
                entry_point: Some("fragment"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: depth_stencil.clone(),
            multisample,
            multiview_mask: None,
            cache: None,
        });

        pipelines.push(PipelineEntry {
            format,
            multisample,
            depth_stencil,
            pipeline: pipeline.clone(),
        });
        pipeline
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlaneKind {
    Color,
    Mask,
}

struct Plane {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    allocator: AtlasAllocator,
    side: u32,
    max_side: u32,
    format: wgpu::TextureFormat,
}

impl Plane {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat, max_side: u32) -> Self {
        let side = INITIAL_ATLAS_SIDE.min(max_side);
        let (texture, view) = create_atlas_texture(device, format, side);

        Self {
            texture,
            view,
            allocator: AtlasAllocator::unpadded(side),
            side,
            max_side,
            format,
        }
    }

    fn allocate(&mut self, width: u32, height: u32) -> Option<Region> {
        self.allocator.allocate(width, height)
    }

    fn can_grow(&self) -> bool {
        self.side < self.max_side
    }

    fn grow(&mut self, device: &wgpu::Device) {
        let side = (self.side * 2).min(self.max_side);
        self.allocator.grow(side);
        (self.texture, self.view) = create_atlas_texture(device, self.format, side);
        self.side = side;
    }
}

fn create_atlas_texture(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    side: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("taim_text_gpu.atlas"),
        size: wgpu::Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

#[derive(Debug, Clone, Copy)]
struct GlyphRecord {
    plane: PlaneKind,
    allocation: Option<Region>,
    width: u32,
    height: u32,
    left: i32,
    top: i32,
    last_frame: u64,
}

/// A pair of bounded mask/color textures and their glyph allocation metadata.
pub struct TextAtlas {
    cache: Cache,
    bind_group: wgpu::BindGroup,
    color: Plane,
    mask: Plane,
    glyphs: FxHashMap<CacheKey, GlyphRecord>,
    used: FxHashSet<CacheKey>,
    frame: u64,
    mode: ColorMode,
    format: wgpu::TextureFormat,
    fallback: Option<fn(&mut FontSystem, CacheKey) -> Option<cosmic_text::SwashImage>>,
}

impl TextAtlas {
    /// Creates a bounded atlas using the requested color mode.
    pub fn with_color_mode(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        cache: &Cache,
        format: wgpu::TextureFormat,
        mode: ColorMode,
    ) -> Self {
        let max_side = device.limits().max_texture_dimension_2d.min(MAX_ATLAS_SIDE);
        let color_format = match mode {
            ColorMode::Accurate => wgpu::TextureFormat::Rgba8UnormSrgb,
            ColorMode::Web => wgpu::TextureFormat::Rgba8Unorm,
        };
        let color = Plane::new(device, color_format, max_side);
        let mask = Plane::new(device, wgpu::TextureFormat::R8Unorm, max_side);
        let bind_group = cache.create_atlas_bind_group(device, &color.view, &mask.view);

        Self {
            cache: cache.clone(),
            bind_group,
            color,
            mask,
            glyphs: FxHashMap::default(),
            used: FxHashSet::default(),
            frame: 0,
            mode,
            format,
            fallback: None,
        }
    }

    /// Supplies an optional rasterizer for glyph formats the default scaler
    /// cannot draw. Its results use the same bounded atlas and eviction policy.
    pub fn set_fallback_rasterizer(
        &mut self,
        rasterizer: fn(&mut FontSystem, CacheKey) -> Option<cosmic_text::SwashImage>,
    ) {
        self.fallback = Some(rasterizer);
    }

    /// Ends an atlas usage epoch while retaining warm glyphs for reuse.
    pub fn trim(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        self.used.clear();
    }

    /// Pins the glyph allocations referenced by an already prepared renderer
    /// in the current atlas epoch. Retained UI layers can therefore keep their
    /// instance buffers without letting a later cache mutation recycle the
    /// texels those buffers still address.
    pub fn retain(&mut self, renderer: &TextRenderer) {
        for key in &renderer.glyphs {
            if let Some(record) = self.glyphs.get_mut(key) {
                record.last_frame = self.frame;
                let _ = self.used.insert(*key);
            }
        }
    }

    fn resolve(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        font_system: &mut FontSystem,
        swash: &mut SwashCache,
        key: CacheKey,
    ) -> Result<Option<GlyphRecord>, PrepareError> {
        if let Some(record) = self.glyphs.get_mut(&key) {
            record.last_frame = self.frame;
            let record = *record;
            let _ = self.used.insert(key);
            return Ok(record.allocation.map(|_| record));
        }

        let image = swash.get_image_uncached(font_system, key);
        let image = if image
            .as_ref()
            .is_none_or(|image| image.placement.width == 0 || image.placement.height == 0)
        {
            self.fallback
                .and_then(|rasterize| rasterize(font_system, key))
                .or(image)
        } else {
            image
        };
        let Some(image) = image else {
            return Ok(None);
        };
        let width = image.placement.width;
        let height = image.placement.height;

        if width == 0 || height == 0 {
            let _ = self.glyphs.insert(
                key,
                GlyphRecord {
                    plane: PlaneKind::Mask,
                    allocation: None,
                    width,
                    height,
                    left: image.placement.left,
                    top: image.placement.top,
                    last_frame: self.frame,
                },
            );
            let _ = self.used.insert(key);
            return Ok(None);
        }

        let plane = match image.content {
            SwashContent::Color => PlaneKind::Color,
            SwashContent::Mask | SwashContent::SubpixelMask => PlaneKind::Mask,
        };
        let allocation = self.allocate(device, queue, font_system, swash, plane, width, height)?;
        let pixels = pixels_for_upload(image.content, &image.data);
        self.write(queue, plane, allocation, width, height, &pixels);

        let record = GlyphRecord {
            plane,
            allocation: Some(allocation),
            width,
            height,
            left: image.placement.left,
            top: image.placement.top,
            last_frame: self.frame,
        };
        let _ = self.glyphs.insert(key, record);
        let _ = self.used.insert(key);
        Ok(Some(record))
    }

    fn allocate(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        font_system: &mut FontSystem,
        swash: &mut SwashCache,
        plane: PlaneKind,
        width: u32,
        height: u32,
    ) -> Result<Region, PrepareError> {
        if width > self.plane(plane).max_side || height > self.plane(plane).max_side {
            return Err(PrepareError::AtlasFull {
                width,
                height,
                limit: self.plane(plane).max_side,
            });
        }

        if let Some(allocation) = self.plane_mut(plane).allocate(width, height) {
            return Ok(allocation);
        }

        while self.evict_oldest(plane, false) {
            if let Some(allocation) = self.plane_mut(plane).allocate(width, height) {
                return Ok(allocation);
            }
        }

        while self.plane(plane).can_grow() {
            self.plane_mut(plane).grow(device);
            self.reupload_plane(queue, font_system, swash, plane);
            self.rebind(device);

            if let Some(allocation) = self.plane_mut(plane).allocate(width, height) {
                return Ok(allocation);
            }
        }

        while self.evict_oldest(plane, true) {
            if let Some(allocation) = self.plane_mut(plane).allocate(width, height) {
                return Ok(allocation);
            }
        }

        Err(PrepareError::AtlasFull {
            width,
            height,
            limit: self.plane(plane).max_side,
        })
    }

    fn evict_oldest(&mut self, plane: PlaneKind, aggressive: bool) -> bool {
        let oldest = self
            .glyphs
            .iter()
            .filter(|(key, record)| {
                record.plane == plane
                    && record.allocation.is_some()
                    && !self.used.contains(key)
                    && (aggressive
                        || self.frame.saturating_sub(record.last_frame) > STALE_FRAME_WINDOW)
            })
            .min_by_key(|(_, record)| record.last_frame)
            .map(|(key, _)| *key);

        let Some(oldest) = oldest else {
            return false;
        };
        let record = self.glyphs.remove(&oldest).expect("Selected glyph exists");
        self.plane_mut(plane)
            .allocator
            .deallocate(&record.allocation.expect("Selected glyph is allocated"));
        true
    }

    fn reupload_plane(
        &self,
        queue: &wgpu::Queue,
        font_system: &mut FontSystem,
        swash: &mut SwashCache,
        plane: PlaneKind,
    ) {
        for (key, record) in &self.glyphs {
            if record.plane != plane {
                continue;
            }
            let Some(allocation) = record.allocation else {
                continue;
            };
            let Some(image) = swash.get_image_uncached(font_system, *key) else {
                continue;
            };
            let pixels = pixels_for_upload(image.content, &image.data);
            self.write(
                queue,
                plane,
                allocation,
                record.width,
                record.height,
                &pixels,
            );
        }
    }

    fn write(
        &self,
        queue: &wgpu::Queue,
        plane: PlaneKind,
        allocation: Region,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) {
        let target = self.plane(plane);
        let (x, y) = allocation.position();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * bytes_per_pixel(plane)),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    fn plane(&self, plane: PlaneKind) -> &Plane {
        match plane {
            PlaneKind::Color => &self.color,
            PlaneKind::Mask => &self.mask,
        }
    }

    fn plane_mut(&mut self, plane: PlaneKind) -> &mut Plane {
        match plane {
            PlaneKind::Color => &mut self.color,
            PlaneKind::Mask => &mut self.mask,
        }
    }

    fn rebind(&mut self, device: &wgpu::Device) {
        self.bind_group =
            self.cache
                .create_atlas_bind_group(device, &self.color.view, &self.mask.view);
    }
}

fn pixels_for_upload(content: SwashContent, pixels: &[u8]) -> Cow<'_, [u8]> {
    match content {
        SwashContent::Color | SwashContent::Mask => Cow::Borrowed(pixels),
        SwashContent::SubpixelMask => Cow::Owned(
            pixels
                .chunks_exact(4)
                .map(|pixel| {
                    let sum = u16::from(pixel[0]) + u16::from(pixel[1]) + u16::from(pixel[2]);
                    (sum / 3) as u8
                })
                .collect(),
        ),
    }
}

fn bytes_per_pixel(plane: PlaneKind) -> u32 {
    match plane {
        PlaneKind::Color => 4,
        PlaneKind::Mask => 1,
    }
}

/// Integer clipping bounds for one text area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextBounds {
    /// Left edge, inclusive.
    pub left: i32,
    /// Top edge, inclusive.
    pub top: i32,
    /// Right edge, exclusive.
    pub right: i32,
    /// Bottom edge, exclusive.
    pub bottom: i32,
}

/// One shaped text buffer together with its placement and clipping policy.
pub struct TextArea<'a> {
    /// Pre-shaped line runs.
    pub text: LayoutRunIter<'a>,
    /// Physical left position.
    pub left: f32,
    /// Physical top position.
    pub top: f32,
    /// Physical scale factor.
    pub scale: f32,
    /// Clipping rectangle.
    pub bounds: TextBounds,
    /// Color used by glyphs without an explicit rich-text color.
    pub default_color: cosmic_text::Color,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct GlyphInstance {
    position: [i32; 2],
    size: [u32; 2],
    atlas_origin: [u32; 2],
    color: [f32; 4],
    plane: u32,
    padding: [u32; 3],
}

/// A reusable per-layer glyph instance stream.
pub struct TextRenderer {
    pipeline: wgpu::RenderPipeline,
    instances: Vec<GlyphInstance>,
    glyphs: FxHashSet<CacheKey>,
    buffer: wgpu::Buffer,
    buffer_capacity: usize,
}

impl TextRenderer {
    /// Creates a renderer sharing the atlas pipeline but owning its instance buffer.
    pub fn new(
        atlas: &mut TextAtlas,
        device: &wgpu::Device,
        multisample: wgpu::MultisampleState,
        depth_stencil: Option<wgpu::DepthStencilState>,
    ) -> Self {
        let buffer_capacity = INITIAL_INSTANCE_CAPACITY * std::mem::size_of::<GlyphInstance>();
        let buffer = create_instance_buffer(device, buffer_capacity);
        let pipeline =
            atlas
                .cache
                .render_pipeline(device, atlas.format, multisample, depth_stencil);

        Self {
            pipeline,
            instances: Vec::with_capacity(INITIAL_INSTANCE_CAPACITY),
            glyphs: FxHashSet::default(),
            buffer,
            buffer_capacity,
        }
    }

    /// Starts a streaming text batch without releasing its retained capacity.
    pub fn begin(&mut self) {
        self.instances.clear();
        self.glyphs.clear();
    }

    /// Appends one shaped text area to the active batch.
    ///
    /// The caller can keep only the current buffer alive, which avoids building
    /// a temporary ownership list for every paragraph in a layer.
    pub fn push(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        font_system: &mut FontSystem,
        atlas: &mut TextAtlas,
        viewport: &Viewport,
        area: TextArea<'_>,
        swash: &mut SwashCache,
    ) -> Result<(), PrepareError> {
        let resolution = viewport.resolution();
        let viewport_right = resolution.width.min(i32::MAX as u32) as i32;
        let viewport_bottom = resolution.height.min(i32::MAX as u32) as i32;
        let clip_left = area.bounds.left.max(0);
        let clip_top = area.bounds.top.max(0);
        let clip_right = area.bounds.right.min(viewport_right);
        let clip_bottom = area.bounds.bottom.min(viewport_bottom);

        if clip_left >= clip_right || clip_top >= clip_bottom || area.scale <= 0.0 {
            return Ok(());
        }

        for run in area.text {
            let run_top = (area.top + run.line_top * area.scale).floor() as i32;
            let run_bottom =
                (area.top + (run.line_top + run.line_height) * area.scale).ceil() as i32;

            if run_bottom <= clip_top || run_top >= clip_bottom {
                continue;
            }

            for glyph in run.glyphs {
                let physical = glyph.physical((area.left, area.top), area.scale);
                let Some(record) =
                    atlas.resolve(device, queue, font_system, swash, physical.cache_key)?
                else {
                    continue;
                };
                let _ = self.glyphs.insert(physical.cache_key);
                let allocation = record.allocation.expect("Resolved glyph is allocated");
                let mut x = physical.x.saturating_add(record.left);
                let mut y = (run.line_y * area.scale).round() as i32;
                y = y.saturating_add(physical.y).saturating_sub(record.top);
                let original_right = x.saturating_add(record.width as i32);
                let original_bottom = y.saturating_add(record.height as i32);

                if x >= clip_right
                    || original_right <= clip_left
                    || y >= clip_bottom
                    || original_bottom <= clip_top
                {
                    continue;
                }

                let (mut atlas_x, mut atlas_y) = allocation.position();

                if x < clip_left {
                    atlas_x += (clip_left - x) as u32;
                    x = clip_left;
                }
                if y < clip_top {
                    atlas_y += (clip_top - y) as u32;
                    y = clip_top;
                }

                let width = original_right.min(clip_right) - x;
                let height = original_bottom.min(clip_bottom) - y;
                if width <= 0 || height <= 0 {
                    continue;
                }

                let color = glyph.color_opt.unwrap_or(area.default_color);
                let color = instance_color(
                    color,
                    area.default_color,
                    glyph.color_opt.is_some(),
                    atlas.mode,
                );
                self.instances.push(GlyphInstance {
                    position: [x, y],
                    size: [width as u32, height as u32],
                    atlas_origin: [atlas_x, atlas_y],
                    color,
                    plane: match record.plane {
                        PlaneKind::Color => 0,
                        PlaneKind::Mask => 1,
                    },
                    padding: [0; 3],
                });
            }
        }

        Ok(())
    }

    /// Uploads the complete active batch using one queue write.
    pub fn finish(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.instances.is_empty() {
            return;
        }

        let bytes = bytemuck::cast_slice(self.instances.as_slice());
        if bytes.len() > self.buffer_capacity {
            self.buffer_capacity = bytes.len().next_power_of_two();
            self.buffer = create_instance_buffer(device, self.buffer_capacity);
        }
        queue.write_buffer(&self.buffer, 0, bytes);
    }

    /// Draws the last prepared instance stream.
    pub fn render(&self, atlas: &TextAtlas, viewport: &Viewport, pass: &mut wgpu::RenderPass<'_>) {
        if self.instances.is_empty() {
            return;
        }

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &atlas.bind_group, &[]);
        pass.set_bind_group(1, &viewport.bind_group, &[]);
        pass.set_vertex_buffer(0, self.buffer.slice(..));
        pass.draw(0..4, 0..self.instances.len() as u32);
    }
}

fn create_instance_buffer(device: &wgpu::Device, size: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("taim_text_gpu.instances"),
        size: size.max(wgpu::COPY_BUFFER_ALIGNMENT as usize) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn instance_color(
    color: cosmic_text::Color,
    default: cosmic_text::Color,
    has_explicit_color: bool,
    mode: ColorMode,
) -> [f32; 4] {
    let [r, g, b, a] = color.as_rgba();
    let default_alpha = if has_explicit_color {
        f32::from(default.as_rgba()[3]) / 255.0
    } else {
        1.0
    };
    let convert = |channel: u8| {
        let value = f32::from(channel) / 255.0;
        match mode {
            ColorMode::Accurate if value <= 0.04045 => value / 12.92,
            ColorMode::Accurate => ((value + 0.055) / 1.055).powf(2.4),
            ColorMode::Web => value,
        }
    };

    [
        convert(r),
        convert(g),
        convert(b),
        f32::from(a) / 255.0 * default_alpha,
    ]
}

/// A bounded-atlas preparation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrepareError {
    /// A glyph cannot fit after growth and inactive-glyph eviction.
    AtlasFull {
        /// Requested glyph width.
        width: u32,
        /// Requested glyph height.
        height: u32,
        /// Maximum atlas side supported by this engine/device pair.
        limit: u32,
    },
}

impl fmt::Display for PrepareError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AtlasFull {
                width,
                height,
                limit,
            } => write!(
                formatter,
                "glyph {width}x{height} does not fit in the bounded {limit}x{limit} atlas"
            ),
        }
    }
}

impl std::error::Error for PrepareError {}

/// The shaped text buffer type accepted by [`TextArea`].
pub type Buffer = cosmic_text::Buffer;

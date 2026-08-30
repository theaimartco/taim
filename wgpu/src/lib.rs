//! A [`wgpu`] renderer for [Taim].
//!
//! ![The native path of the Taim ecosystem](https://github.com/theaimartco/taim/blob/main/docs/graphs/native.png?raw=true)
//!
//! [`wgpu`] supports most modern graphics backends: Vulkan, Metal, DX11, and
//! DX12 (OpenGL and WebGL are still WIP). Additionally, it will support the
//! incoming [WebGPU API].
//!
//! Currently, `taim_wgpu` supports the following primitives:
//! - Text, which is rendered using [`glyphon`].
//! - Quads or rectangles, with rounded borders and a solid background color.
//! - Clip areas, useful to implement scrollables or hide overflowing content.
//! - Images and SVG, loaded from memory or the file system.
//! - Meshes of triangles, useful to draw geometry freely.
//!
//! [Taim]: https://github.com/theaimartco/taim
//! [`wgpu`]: https://github.com/gfx-rs/wgpu-rs
//! [WebGPU API]: https://gpuweb.github.io/gpuweb/
//! [`glyphon`]: https://github.com/grovesNL/glyphon
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/theaimartco/taim/main/docs/logo.svg"
)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(missing_docs)]
pub mod display;
pub mod layer;
pub mod primitive;
pub mod window;

#[cfg(feature = "geometry")]
pub mod geometry;

pub mod batch;
pub mod effects;

mod buffer;
mod color;
mod engine;
pub mod metrics;
mod quad;
pub mod resource_budget;
mod text;
mod triangle;

#[cfg(any(feature = "image", feature = "svg"))]
#[path = "image/mod.rs"]
mod image;

#[cfg(not(any(feature = "image", feature = "svg")))]
#[path = "image/null.rs"]
mod image;

use buffer::Buffer;

use taim_debug as debug;
pub use taim_graphics as graphics;
pub use taim_graphics::core;

pub use wgpu;

pub use engine::Engine;
pub use layer::Layer;
pub use primitive::Primitive;

#[cfg(feature = "geometry")]
pub use geometry::Geometry;

use crate::core::renderer;
use crate::core::{Background, Color, Font, Pixels, Point, Rectangle, Size, Transformation};
use crate::graphics::mesh;
use crate::graphics::text::{Editor, Paragraph};
use crate::graphics::{Shell, Viewport};

#[derive(Debug)]
struct RetainedFrame {
    key: u64,
    revision: u64,
    bounds: Rectangle,
    scale_factor: Option<f32>,
    display_list: layer::Stack,
}

#[derive(Debug)]
struct ActiveRetainedFrame {
    key: u64,
    revision: u64,
    bounds: Rectangle,
    scale_factor: Option<f32>,
    range: std::ops::Range<usize>,
}

#[derive(Debug)]
struct RecordingRetainedFrame {
    key: u64,
    revision: u64,
    bounds: Rectangle,
    scale_factor: Option<f32>,
    start: usize,
}

/// What is behind a transmissive quad, captured from the frame.
///
/// The whole frame at frame resolution rather than a scissored region: a
/// refraction offset reaches outside the quad's own bounds by design, and a
/// region-sized capture would clip exactly the displacement that makes the
/// effect visible.
struct QuadBackdrop {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    size: wgpu::Extent3d,
    format: wgpu::TextureFormat,
}

impl QuadBackdrop {
    /// Whether this capture can still receive a copy of `texture`.
    ///
    /// `copy_texture_to_texture` requires both sides agree on format and
    /// size, so a resize or a format change has to rebuild rather than
    /// silently sample a stale frame.
    fn matches(&self, texture: &wgpu::Texture) -> bool {
        self.format == texture.format() && self.size == texture.size()
    }
}

/// A [`wgpu`] graphics renderer for [`taim`].
///
/// [`wgpu`]: https://github.com/gfx-rs/wgpu-rs
/// [`taim`]: https://github.com/theaimartco/taim
pub struct Renderer {
    engine: Engine,
    settings: renderer::Settings,

    layers: layer::Stack,
    scale_factor: Option<f32>,
    retained_frame: Option<RetainedFrame>,
    active_retained_frame: Option<ActiveRetainedFrame>,
    recording_retained_frame: Option<RecordingRetainedFrame>,
    retained_layer_statistics: core::renderer::RetainedLayerStatistics,
    /// The damage of the last few frames, deep enough to cover the swapchain.
    /// See [`Renderer::accumulate_damage`].
    damage_history: std::collections::VecDeque<Rectangle<u32>>,
    /// The region this frame is confined to, if it is a partial one.
    frame_damage: Option<Rectangle<u32>>,

    /// The active visual-quality tier.
    ///
    /// Gates effect cost in two ways: `Effect::allowed`/`bounds` clamp blur
    /// radius and backdrop area, and `Policy::texture_budget_bytes` bounds
    /// what the effect intermediates may allocate. Previously
    /// `Tier::default()` was passed literally at the call site, so no
    /// application could ever select another one and thermal or battery
    /// pressure had nowhere to land.
    quality: crate::graphics::quality::Tier,

    /// The one light every material in the interface responds to.
    ///
    /// Frame-level rather than per-widget: a per-application light produces
    /// surfaces that disagree about where the window is, which reads as
    /// inconsistency rather than as lighting. Set it from
    /// `taim_design::tokens::MaterialTokens::light`.
    light: core::material::SceneLight,

    /// Pipelines compatible with the format currently being drawn into.
    ///
    /// Re-resolved by [`Renderer::use_format`] when the target's format
    /// changes, which is what lets one engine serve an SDR and an HDR display
    /// without being rebuilt.
    pipelines: engine::Pipelines,

    /// The half-float frame HDR presentation draws into, and the transform
    /// that resolves it onto the surface. `None` on the standard-range path,
    /// which draws straight to the surface exactly as it always has.
    hdr: Option<(display::Target, core::color::DisplayTransform)>,

    /// A copy of the frame as it stood before a transmissive layer drew,
    /// and the bind group that reads it.
    ///
    /// Allocated the first time a quad actually transmits and reused after,
    /// so an interface with no glass in it never pays for this. Kept keyed
    /// by size and format so a resize or a format change rebuilds it rather
    /// than sampling a stale one.
    quad_backdrop: Option<QuadBackdrop>,

    quad: quad::State,
    triangle: triangle::State,
    text: text::State,
    text_viewport: text::Viewport,

    #[cfg(any(feature = "svg", feature = "image"))]
    image: image::State,

    // TODO: Centralize all the image feature handling
    #[cfg(any(feature = "svg", feature = "image"))]
    image_cache: std::cell::RefCell<image::Cache>,

    staging_belt: wgpu::util::StagingBelt,

    metrics: metrics::Counters,
}

impl Renderer {
    pub fn new(engine: Engine, settings: renderer::Settings) -> Self {
        let pipelines = engine.pipelines(engine.format);

        Self {
            quality: crate::graphics::quality::Tier::default(),
            pipelines: pipelines.clone(),
            settings,
            layers: layer::Stack::new(),
            scale_factor: None,
            retained_frame: None,
            active_retained_frame: None,
            recording_retained_frame: None,
            retained_layer_statistics: core::renderer::RetainedLayerStatistics::default(),
            damage_history: std::collections::VecDeque::new(),
            frame_damage: None,
            light: core::material::SceneLight::KEY,
            hdr: None,
            quad_backdrop: None,

            quad: quad::State::new(),
            triangle: triangle::State::new(&engine.device, &pipelines.triangle),
            text: text::State::new(),
            text_viewport: pipelines.text.create_viewport(&engine.device),

            #[cfg(any(feature = "svg", feature = "image"))]
            image: image::State::new(),

            #[cfg(any(feature = "svg", feature = "image"))]
            image_cache: std::cell::RefCell::new(engine.create_image_cache()),

            // TODO: Resize belt smartly (?)
            // It would be great if the `StagingBelt` API exposed methods
            // for introspection to detect when a resize may be worth it.
            staging_belt: wgpu::util::StagingBelt::new(
                engine.device.clone(),
                buffer::MAX_WRITE_SIZE as u64,
            ),

            metrics: metrics::Counters::new(),

            engine,
        }
    }

    /// Returns this frame's GPU hot-path [`metrics::Counters`] (primitive,
    /// batch, upload, atlas-miss, pipeline-switch, and draw counts).
    ///
    /// Reset at the start of every [`Renderer::draw`] call; read this
    /// after `draw` returns (and before the next one) to see the counts
    /// for the frame just recorded. Always reports zero for every metric
    /// unless the `profiling` feature is enabled (see
    /// [`metrics::Counters::is_active`]).
    pub fn metrics(&self) -> &metrics::Counters {
        &self.metrics
    }

    /// Record commands that draw the current primitives to the target texture view.
    ///
    /// You must call [`finish`](Self::finish) and [`recall`](Self::recall) when submitting
    /// the resulting [`wgpu::CommandEncoder`].
    pub fn draw(
        &mut self,
        clear_color: Option<Color>,
        target: &wgpu::TextureView,
        target_texture: &wgpu::Texture,
        viewport: &Viewport,
    ) -> wgpu::CommandEncoder {
        // The target is the authority on what pipelines are compatible. This
        // is the line that replaces `Engine`'s single global format.
        self.use_format(target_texture.format());

        // Once per frame, unconditionally. A frame that queues no effects --
        // because the tier rejected them all, which is exactly what
        // `Recovery` does -- must still move the ledger to the tier's budget,
        // or telemetry reports a budget the renderer is no longer using.
        self.pipelines.effect.sync_budget(self.quality);

        self.metrics.reset();

        let mut encoder =
            self.engine
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("taim_wgpu encoder"),
                });

        self.prepare(&mut encoder, viewport);
        self.render(&mut encoder, target, target_texture, clear_color, viewport);

        self.quad.trim();
        self.triangle.trim();
        self.text.trim();

        // TODO: Provide window id (?)
        self.engine.trim();

        #[cfg(any(feature = "svg", feature = "image"))]
        {
            self.image.trim();
            self.image_cache.borrow_mut().trim();
        }

        encoder
    }

    pub fn present(
        &mut self,
        clear_color: Option<Color>,
        _format: wgpu::TextureFormat,
        frame: &wgpu::TextureView,
        frame_texture: &wgpu::Texture,
        viewport: &Viewport,
    ) -> wgpu::SubmissionIndex {
        let encoder = self.draw(clear_color, frame, frame_texture, viewport);

        self.staging_belt.finish();
        let submission = self.engine.queue.submit([encoder.finish()]);
        self.staging_belt.recall();
        submission
    }

    /// Returns the bytes of effect intermediates currently accounted, and
    /// the budget they are accounted against.
    ///
    /// The budget comes from the active [`Renderer::quality`] tier.
    pub fn effect_memory(&self) -> (u64, u64) {
        self.pipelines.effect.memory()
    }

    /// Returns the size the effect intermediates were allocated at.
    ///
    /// Smaller than the surface when the budget forced a downscale, and
    /// `None` when no effect has been prepared or the budget refused the
    /// smallest useful pair.
    pub fn effect_target_size(&self) -> Option<(u32, u32)> {
        self.pipelines.effect.target_size()
    }

    /// Returns the active visual-quality tier.
    pub const fn quality(&self) -> crate::graphics::quality::Tier {
        self.quality
    }

    /// Sets the visual-quality tier.
    ///
    /// Takes effect on the next frame. Lowering it both restricts which
    /// effects are admitted and shrinks the budget their intermediates are
    /// allowed to occupy, so a caller responding to thermal pressure or a
    /// battery signal has one lever rather than several.
    pub fn set_quality(&mut self, tier: crate::graphics::quality::Tier) {
        if self.quality != tier {
            log::info!("visual quality tier: {:?} -> {tier:?}", self.quality);
        }

        self.quality = tier;
    }

    /// Ensures this renderer's pipelines are compatible with `format`.
    ///
    /// A no-op in the steady state -- one integer comparison per frame. When
    /// the format really has changed, this rebuilds only what depends on it:
    /// the pipelines themselves, the triangle pipeline's MSAA state, and the
    /// text pipeline's glyph atlas viewport. Nothing device-global is touched,
    /// so a window moving between an SDR and an HDR display keeps its caches,
    /// its images, and its primitive storage.
    ///
    /// Before this existed, drawing into a differently formatted target
    /// produced a `wgpu` render-pass validation error naming pipeline
    /// compatibility, which said nothing about the actual cause.
    fn use_format(&mut self, format: wgpu::TextureFormat) {
        if self.pipelines.format == format {
            return;
        }

        log::info!(
            "render target format changed from {:?} to {:?}; selecting compatible pipelines",
            self.pipelines.format,
            format
        );

        self.pipelines = self.engine.pipelines(format);

        // Both of these are derived from a pipeline rather than from the
        // device, so they follow the format. Missing either is a validation
        // error at the next draw.
        self.triangle = triangle::State::new(&self.engine.device, &self.pipelines.triangle);
        self.text_viewport = self.pipelines.text.create_viewport(&self.engine.device);
    }

    /// Returns the format this renderer's pipelines currently target.
    ///
    /// Fixed when the [`Engine`] is built, and the same for every pipeline it
    /// owns, so any intermediate render target has to match it.
    pub fn frame_format(&self) -> wgpu::TextureFormat {
        self.pipelines.format
    }

    /// Returns the [`wgpu::Device`] this renderer draws with.
    pub fn device(&self) -> &wgpu::Device {
        &self.engine.device
    }

    /// Returns the [`wgpu::Queue`] this renderer submits to.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.engine.queue
    }

    /// Presents through the high-dynamic-range path.
    ///
    /// Draws the frame into a half-float target rather than straight onto the
    /// surface, then resolves it through `display`'s transform. The extra
    /// target is what keeps a highlight brighter than paper: the renderer
    /// emits linear light, and every `Unorm` surface clamps it at `1.0`.
    ///
    /// One encoder and one submission, same as [`Renderer::present`] — the
    /// resolve is appended to the frame's own encoder rather than costing a
    /// second submit.
    pub fn present_hdr(
        &mut self,
        clear_color: Option<Color>,
        surface: &wgpu::TextureView,
        surface_format: wgpu::TextureFormat,
        viewport: &Viewport,
        display: core::color::Display,
    ) -> wgpu::SubmissionIndex {
        // Always half float, regardless of what the surface or the engine
        // prefer. This is only correct because pipelines are now resolved
        // from the target being drawn into rather than from one engine-global
        // format: `draw` sees a half-float intermediate and builds pipelines
        // for it. Before that, an engine created for an 8-bit surface could
        // not draw here at all, and HDR required the surface to have been
        // half float when the engine was built.
        let format = display::Target::FORMAT;

        let size = viewport.physical_size();

        let stale = match &self.hdr {
            Some((target, _)) => !target.fits(size.width, size.height),
            None => true,
        };

        if stale {
            self.hdr = Some((
                display::Target::new(
                    &self.engine.device,
                    size.width,
                    size.height,
                    format,
                    surface_format,
                ),
                display::transform_for(surface_format, display),
            ));
        }

        // Taken out for the duration so that `draw` can borrow `self` mutably.
        // Put back before returning, so the target survives to the next frame
        // and a steady-state frame allocates nothing.
        let Some((target, _)) = self.hdr.take() else {
            unreachable!("the target was just ensured to exist")
        };

        // Rebuilt every frame rather than cached: it is a handful of matrix
        // rows, and a stale one would keep tone mapping to the panel the
        // window used to be on.
        let transform = display::transform_for(surface_format, display);

        let mut encoder = self.draw(clear_color, target.view(), target.texture(), viewport);

        target.resolve(
            &self.engine.device,
            &self.engine.queue,
            &mut encoder,
            surface,
            &transform,
        );

        self.staging_belt.finish();
        let submission = self.engine.queue.submit([encoder.finish()]);
        self.staging_belt.recall();

        self.hdr = Some((target, transform));

        submission
    }

    /// Renders the current surface to an offscreen buffer.
    ///
    /// Returns RGBA bytes of the texture data.
    pub fn screenshot(&mut self, viewport: &Viewport, background_color: Color) -> Vec<u8> {
        #[derive(Clone, Copy, Debug)]
        struct BufferDimensions {
            width: u32,
            height: u32,
            unpadded_bytes_per_row: usize,
            padded_bytes_per_row: usize,
        }

        impl BufferDimensions {
            fn new(size: Size<u32>) -> Self {
                let unpadded_bytes_per_row = size.width as usize * 4; //slice of buffer per row; always RGBA
                let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize; //256
                let padded_bytes_per_row_padding =
                    (alignment - unpadded_bytes_per_row % alignment) % alignment;
                let padded_bytes_per_row = unpadded_bytes_per_row + padded_bytes_per_row_padding;

                Self {
                    width: size.width,
                    height: size.height,
                    unpadded_bytes_per_row,
                    padded_bytes_per_row,
                }
            }
        }

        let dimensions = BufferDimensions::new(viewport.physical_size());

        let texture_extent = wgpu::Extent3d {
            width: dimensions.width,
            height: dimensions.height,
            depth_or_array_layers: 1,
        };

        let texture = self.engine.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("taim_wgpu.offscreen.source_texture"),
            size: texture_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.engine.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.draw(Some(background_color), &view, &texture, viewport);

        let texture = crate::color::convert(
            &self.engine.device,
            &mut encoder,
            texture,
            if graphics::color::GAMMA_CORRECTION {
                wgpu::TextureFormat::Rgba8UnormSrgb
            } else {
                wgpu::TextureFormat::Rgba8Unorm
            },
        );

        let output_buffer = self.engine.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taim_wgpu.offscreen.output_texture_buffer"),
            size: (dimensions.padded_bytes_per_row * dimensions.height as usize) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &output_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(dimensions.padded_bytes_per_row as u32),
                    rows_per_image: None,
                },
            },
            texture_extent,
        );

        self.staging_belt.finish();
        let index = self.engine.queue.submit([encoder.finish()]);
        self.staging_belt.recall();

        let slice = output_buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});

        let _ = self.engine.device.poll(wgpu::PollType::Wait {
            submission_index: Some(index),
            timeout: None,
        });

        let mapped_buffer = slice.get_mapped_range();

        mapped_buffer
            .chunks(dimensions.padded_bytes_per_row)
            .fold(vec![], |mut acc, row| {
                acc.extend(&row[..dimensions.unpadded_bytes_per_row]);
                acc
            })
    }

    fn prepare(&mut self, encoder: &mut wgpu::CommandEncoder, viewport: &Viewport) {
        let scale_factor = viewport.scale_factor();

        self.text_viewport
            .update(&self.engine.queue, viewport.physical_size());

        let physical_bounds =
            Rectangle::<f32>::from(Rectangle::with_size(viewport.physical_size()));

        // A retained range must keep its layer boundaries intact until the
        // next frame moves it into the cache. Merging would drain commands
        // across that boundary and make the cached display list incomplete.
        if self.active_retained_frame.is_some() {
            self.layers.flush();
        } else {
            self.layers.merge();
        }

        for (index, layer) in self.layers.iter().enumerate() {
            let clip_bounds = layer.bounds * scale_factor;

            if physical_bounds
                .intersection(&clip_bounds)
                .and_then(Rectangle::snap)
                .is_none()
            {
                continue;
            }

            if !layer.quads.is_empty() {
                let prepare_span = debug::prepare(debug::Primitive::Quad);
                self.metrics.increment(metrics::Metric::Primitive);
                self.metrics.increment(metrics::Metric::Upload);

                self.quad.prepare(
                    &self.pipelines.quad,
                    &self.engine.device,
                    &mut self.staging_belt,
                    encoder,
                    &layer.quads,
                    viewport.projection(),
                    scale_factor,
                    self.light,
                );

                prepare_span.finish();
            }

            if !layer.triangles.is_empty() {
                let prepare_span = debug::prepare(debug::Primitive::Triangle);
                self.metrics.increment(metrics::Metric::Primitive);
                self.metrics.increment(metrics::Metric::Upload);

                self.triangle.prepare(
                    &self.pipelines.triangle,
                    &self.engine.device,
                    &mut self.staging_belt,
                    encoder,
                    &layer.triangles,
                    Transformation::scale(scale_factor),
                    viewport.physical_size(),
                );

                prepare_span.finish();
            }

            if !layer.primitives.is_empty() {
                let prepare_span = debug::prepare(debug::Primitive::Shader);
                self.metrics
                    .increment_by(metrics::Metric::Primitive, layer.primitives.len() as u32);

                let mut primitive_storage = self
                    .engine
                    .primitive_storage
                    .write()
                    .expect("Write primitive storage");

                for instance in &layer.primitives {
                    instance.primitive.prepare(
                        &mut primitive_storage,
                        &self.engine.device,
                        &self.engine.queue,
                        self.engine.format,
                        &instance.bounds,
                        viewport,
                    );
                }

                prepare_span.finish();
            }

            #[cfg(any(feature = "svg", feature = "image"))]
            if !layer.images.is_empty() {
                let prepare_span = debug::prepare(debug::Primitive::Image);
                self.metrics.increment(metrics::Metric::Primitive);
                self.metrics.increment(metrics::Metric::Upload);

                self.image.prepare(
                    &self.pipelines.image,
                    &self.engine.device,
                    &mut self.staging_belt,
                    encoder,
                    &mut self.image_cache.borrow_mut(),
                    &layer.images,
                    viewport.projection(),
                    scale_factor,
                );

                prepare_span.finish();
            }

            if !layer.text.is_empty() {
                let prepare_span = debug::prepare(debug::Primitive::Text);
                self.metrics.increment(metrics::Metric::Primitive);
                self.metrics.increment(metrics::Metric::Upload);

                self.text.prepare(
                    &self.pipelines.text,
                    &self.engine.device,
                    &self.engine.queue,
                    &self.text_viewport,
                    encoder,
                    &layer.text,
                    layer.bounds,
                    Transformation::scale(scale_factor),
                );

                prepare_span.finish();
            }

            if !layer.effects.is_empty() {
                let physical = viewport.physical_size();
                let surface = Rectangle::with_size(crate::core::Size::new(
                    physical.width as f32,
                    physical.height as f32,
                ));

                self.pipelines.effect.prepare(
                    &self.engine.device,
                    index,
                    &layer.effects,
                    layer.bounds * scale_factor,
                    surface,
                    self.quality,
                    (physical.width, physical.height),
                );
            }
        }
    }

    /// Allocates the quad backdrop if any layer will transmit this frame.
    ///
    /// Hoisted out of the draw loop, which holds `self.layers` borrowed, and
    /// done once per frame rather than once per transmissive layer -- every
    /// layer captures the same frame texture at the same size, so they share
    /// one allocation and differ only in what has been drawn into it by the
    /// time each copy runs.
    ///
    /// Leaves `quad_backdrop` untouched when nothing transmits, so an
    /// interface with no glass in it never allocates a frame-sized texture.
    fn prepare_quad_backdrop(&mut self, frame_texture: &wgpu::Texture) {
        let stale = self
            .quad_backdrop
            .as_ref()
            .is_none_or(|backdrop| !backdrop.matches(frame_texture));

        if !stale {
            return;
        }

        let size = frame_texture.size();
        let format = frame_texture.format();

        let texture = self.engine.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("taim_wgpu::quad backdrop"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let bind_group = self.pipelines.quad.backdrop_bind_group(
            &self.engine.device,
            &texture.create_view(&wgpu::TextureViewDescriptor::default()),
        );

        self.quad_backdrop = Some(QuadBackdrop {
            texture,
            bind_group,
            size,
            format,
        });
    }

    fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::TextureView,
        frame_texture: &wgpu::Texture,
        clear_color: Option<Color>,
        viewport: &Viewport,
    ) {
        use std::mem::ManuallyDrop;

        // Before the draw loop, which borrows `self.layers`.
        eprintln!("DBG layers={} any_needs={}", self.layers.iter().count(), self.layers.iter().any(|layer| layer.quads.needs_backdrop()));
        for (i, l) in self.layers.iter().enumerate() {
            eprintln!("DBG   layer {i}: quads_empty={} needs_backdrop={}", l.quads.is_empty(), l.quads.needs_backdrop());
        }
        if self.layers.iter().any(|layer| layer.quads.needs_backdrop()) {
            self.prepare_quad_backdrop(frame_texture);
            eprintln!("DBG prepared backdrop: {}", self.quad_backdrop.is_some());
        }

        let mut render_pass =
            ManuallyDrop::new(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("taim_wgpu render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: frame,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // A partial frame must preserve what is already on
                        // the surface: clearing would blank everything
                        // outside the damage, which is the whole screen.
                        load: match clear_color.filter(|_| self.frame_damage.is_none()) {
                            Some(background_color) => wgpu::LoadOp::Clear({
                                let [r, g, b, a] =
                                    graphics::color::pack(background_color).components();

                                wgpu::Color {
                                    r: f64::from(r * a),
                                    g: f64::from(g * a),
                                    b: f64::from(b * a),
                                    a: f64::from(a),
                                }
                            }),
                            None => wgpu::LoadOp::Load,
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            }));

        let mut quad_layer = 0;
        let mut mesh_layer = 0;
        let mut text_layer = 0;

        #[cfg(any(feature = "svg", feature = "image"))]
        let mut image_layer = 0;

        let scale_factor = viewport.scale_factor();
        let physical_bounds =
            Rectangle::<f32>::from(Rectangle::with_size(viewport.physical_size()));

        let scale = Transformation::scale(scale_factor);

        for (index, layer) in self.layers.iter().enumerate() {
            let Some(physical_bounds) =
                physical_bounds.intersection(&(layer.bounds * scale_factor))
            else {
                continue;
            };

            let Some(scissor_rect) = physical_bounds.snap() else {
                continue;
            };

            // Nothing outside the frame's damage may be touched, so a layer
            // that falls entirely outside it is skipped rather than drawn and
            // scissored away.
            let Some(scissor_rect) = intersect(scissor_rect, self.frame_damage) else {
                continue;
            };

            // Backdrop effects read what is behind this layer, so they run
            // before it draws over them. A backdrop blur frosts the page
            // behind a sheet; running it after the sheet has drawn would
            // blur the sheet instead.
            if self.pipelines.effect.has_backdrop(index) {
                let physical = viewport.physical_size();

                let _ = ManuallyDrop::into_inner(render_pass);

                self.pipelines.effect.render_backdrop(
                    &self.engine.device,
                    &self.engine.queue,
                    encoder,
                    frame_texture,
                    (physical.width, physical.height),
                    index,
                );

                render_pass =
                    ManuallyDrop::new(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("taim_wgpu render pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: frame,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    }));
            }

            // A transmissive quad refracts what is behind it, so what is
            // behind it has to be captured before it draws over it. Unlike a
            // backdrop *effect*, which replaces its region in its own pass,
            // this is a plain copy: the bending happens per-fragment inside
            // the quad's own draw.
            let backdrop = (!layer.quads.is_empty() && layer.quads.needs_backdrop())
                .then(|| self.quad_backdrop.as_ref())
                .flatten();

            if let Some(backdrop) = backdrop {
                let _ = ManuallyDrop::into_inner(render_pass);

                encoder.copy_texture_to_texture(
                    frame_texture.as_image_copy(),
                    backdrop.texture.as_image_copy(),
                    backdrop.size,
                );

                render_pass =
                    ManuallyDrop::new(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("taim_wgpu render pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: frame,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    }));
            }

            if !layer.quads.is_empty() {
                let render_span = debug::render(debug::Primitive::Quad);
                self.quad.render(
                    &self.pipelines.quad,
                    quad_layer,
                    scissor_rect,
                    &layer.quads,
                    backdrop.map(|backdrop| &backdrop.bind_group),
                    &mut render_pass,
                    &self.metrics,
                );
                render_span.finish();

                quad_layer += 1;
            }

            if !layer.triangles.is_empty() {
                let _ = ManuallyDrop::into_inner(render_pass);

                let render_span = debug::render(debug::Primitive::Triangle);
                mesh_layer += self.triangle.render(
                    &self.pipelines.triangle,
                    encoder,
                    frame,
                    mesh_layer,
                    &layer.triangles,
                    physical_bounds,
                    scale,
                );
                render_span.finish();

                render_pass =
                    ManuallyDrop::new(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("taim_wgpu render pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: frame,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    }));
            }

            if !layer.primitives.is_empty() {
                let render_span = debug::render(debug::Primitive::Shader);

                let primitive_storage = self
                    .engine
                    .primitive_storage
                    .read()
                    .expect("Read primitive storage");

                let mut need_render = Vec::new();

                for instance in &layer.primitives {
                    let bounds = instance.bounds * scale;

                    if let Some(clip_bounds) = (instance.bounds * scale)
                        .intersection(&physical_bounds)
                        .and_then(Rectangle::snap)
                    {
                        render_pass.set_viewport(
                            bounds.x,
                            bounds.y,
                            bounds.width,
                            bounds.height,
                            0.0,
                            1.0,
                        );

                        render_pass.set_scissor_rect(
                            clip_bounds.x,
                            clip_bounds.y,
                            clip_bounds.width,
                            clip_bounds.height,
                        );

                        let drawn = instance
                            .primitive
                            .draw(&primitive_storage, &mut render_pass);

                        if !drawn {
                            need_render.push((instance, clip_bounds));
                        }
                    }
                }

                render_pass.set_viewport(
                    0.0,
                    0.0,
                    viewport.physical_width() as f32,
                    viewport.physical_height() as f32,
                    0.0,
                    1.0,
                );

                render_pass.set_scissor_rect(
                    0,
                    0,
                    viewport.physical_width(),
                    viewport.physical_height(),
                );

                if !need_render.is_empty() {
                    let _ = ManuallyDrop::into_inner(render_pass);

                    for (instance, clip_bounds) in need_render {
                        instance
                            .primitive
                            .render(&primitive_storage, encoder, frame, &clip_bounds);
                    }

                    render_pass =
                        ManuallyDrop::new(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("taim_wgpu render pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: frame,
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Load,
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                            multiview_mask: None,
                        }));
                }

                render_span.finish();
            }

            #[cfg(any(feature = "svg", feature = "image"))]
            if !layer.images.is_empty() {
                let render_span = debug::render(debug::Primitive::Image);
                self.image.render(
                    &self.pipelines.image,
                    image_layer,
                    scissor_rect,
                    &mut render_pass,
                );
                render_span.finish();

                image_layer += 1;
            }

            if !layer.text.is_empty() {
                let render_span = debug::render(debug::Primitive::Text);
                text_layer += self.text.render(
                    &self.pipelines.text,
                    &self.text_viewport,
                    text_layer,
                    &layer.text,
                    scissor_rect,
                    &mut render_pass,
                );
                render_span.finish();
            }

            if !layer.effects.is_empty() {
                let _ = ManuallyDrop::into_inner(render_pass);
                let physical = viewport.physical_size();

                self.pipelines.effect.render(
                    &self.engine.device,
                    &self.engine.queue,
                    encoder,
                    frame_texture,
                    (physical.width, physical.height),
                );
                render_pass =
                    ManuallyDrop::new(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("taim_wgpu render pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: frame,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    }));
            }
        }

        let _ = ManuallyDrop::into_inner(render_pass);

        debug::layers_rendered(|| {
            self.layers
                .iter()
                .filter(|layer| {
                    !layer.is_empty()
                        && physical_bounds
                            .intersection(&(layer.bounds * scale_factor))
                            .is_some_and(|viewport| viewport.snap().is_some())
                })
                .count()
        });
    }

    /// Prepares currently mapped buffers for use in a submission.
    ///
    /// Usually, this method is only needed if you are calling [`Renderer::draw`] directly,
    /// instead of relying on [`Renderer::present`].
    ///
    /// You must call this method _before_ submitting the resulting [`wgpu::CommandEncoder`]
    /// of [`Renderer::draw`] to a [`wgpu::Queue`].
    pub fn finish(&mut self) {
        self.staging_belt.finish();
    }

    /// Recalls all of the closed buffers back to be reused.
    ///
    /// Usually, this method is only needed if you are calling [`Renderer::draw`] directly,
    /// instead of relying on [`Renderer::present`] to a [`wgpu::Queue`].
    ///
    /// You must call this method _after_ submitting the resulting [`wgpu::CommandEncoder`]
    /// of [`Renderer::draw`] to a [`wgpu::Queue`].
    pub fn recall(&mut self) {
        self.staging_belt.recall();
    }

    /// Uploads a coverage mask for [`effects::Effect::Mask`] to clip to.
    ///
    /// See [`effects::pipeline::Pipeline::upload_mask`]; the mask must be
    /// [`Renderer::mask_size`] square, one byte of coverage per pixel.
    pub fn upload_mask(&self, slot: u32, coverage: &[u8]) -> Option<u32> {
        // The mask atlas belongs to the effect pipeline currently resolved,
        // which is the one this frame's effects will read from.
        self.pipelines
            .effect
            .upload_mask(&self.engine.queue, slot, coverage)
    }

    /// The edge length every uploaded mask must have.
    pub const fn mask_size() -> u32 {
        crate::effects::pipeline::Pipeline::mask_size()
    }

    /// How many mask slots the atlas holds.
    pub const fn mask_slots() -> u32 {
        crate::effects::pipeline::Pipeline::mask_slots()
    }

    /// Narrows this frame to `damage`, preserving what is already on the
    /// surface outside it.
    ///
    /// # Why a history rather than the frame's own damage
    ///
    /// A swapchain hands back one of several rotating buffers, and the one
    /// acquired this frame holds whatever was drawn to it *N frames ago*,
    /// not last frame. Scissoring to this frame's damage alone would leave
    /// everything else showing a stale image.
    ///
    /// wgpu exposes no buffer age, so the safe construction is to redraw the
    /// union of the last `depth` frames' damage, where `depth` is the
    /// swapchain's length: whichever buffer comes back, everything that has
    /// changed since it was last drawn is inside that union.
    ///
    /// Returns the region to redraw, or `None` when the whole surface must
    /// be — which is what an unknown or full-surface damage means, and what
    /// the first frames after a resize mean.
    pub fn accumulate_damage(
        &mut self,
        damage: Option<&graphics::damage::DamageRegion>,
        surface: Size<u32>,
        depth: usize,
    ) -> Option<Rectangle<u32>> {
        let bounded = damage.and_then(|damage| {
            if damage.is_full() {
                None
            } else {
                damage.bounding_rect()
            }
        });

        let Some(bounds) = bounded else {
            // Unknown or full: the history is meaningless now, because a full
            // redraw makes every buffer current.
            self.damage_history.clear();
            return None;
        };

        let region = Rectangle {
            x: bounds.x.floor().max(0.0) as u32,
            y: bounds.y.floor().max(0.0) as u32,
            width: bounds.width.ceil().max(0.0) as u32,
            height: bounds.height.ceil().max(0.0) as u32,
        };

        self.damage_history.push_back(region);

        while self.damage_history.len() > depth.max(1) {
            let _ = self.damage_history.pop_front();
        }

        // The history is not yet as deep as the swapchain, so at least one
        // buffer has never been drawn with a partial frame. Redraw all of it.
        if self.damage_history.len() < depth.max(1) {
            return None;
        }

        let mut union = region;

        for past in &self.damage_history {
            let right = union.x.max(past.x + past.width).max(union.x + union.width);
            let bottom = union
                .y
                .max(past.y + past.height)
                .max(union.y + union.height);

            union.x = union.x.min(past.x);
            union.y = union.y.min(past.y);
            union.width = right - union.x;
            union.height = bottom - union.y;
        }

        let clipped = Rectangle {
            x: union.x.min(surface.width),
            y: union.y.min(surface.height),
            width: union.width.min(surface.width.saturating_sub(union.x)),
            height: union.height.min(surface.height.saturating_sub(union.y)),
        };

        // A union that covers the surface is a full redraw with extra steps.
        if clipped.width >= surface.width && clipped.height >= surface.height {
            return None;
        }

        (clipped.width > 0 && clipped.height > 0).then_some(clipped)
    }

    /// Sets the scissor every layer in the next frame is clipped to.
    pub fn set_frame_damage(&mut self, damage: Option<Rectangle<u32>>) {
        self.frame_damage = damage;
    }

    /// Submits an effect node to the current layer's effect graph.
    pub fn add_effect(
        &mut self,
        effect: crate::effects::Effect,
        tier: crate::graphics::quality::Tier,
    ) -> bool {
        let (layer, _) = self.layers.current_mut();
        layer.effects.push(effect, tier)
    }
}

impl core::Renderer for Renderer {
    fn start_layer(&mut self, bounds: Rectangle) {
        self.layers.push_clip(bounds);
    }

    fn end_layer(&mut self) {
        self.layers.pop_clip();
    }

    fn start_transformation(&mut self, transformation: Transformation) {
        self.layers.push_transformation(transformation);
    }

    fn end_transformation(&mut self) {
        self.layers.pop_transformation();
    }

    fn start_opacity(&mut self, opacity: f32) {
        self.layers.push_opacity(opacity);
    }

    fn end_opacity(&mut self) {
        self.layers.pop_opacity();
    }

    fn fill_quad(&mut self, mut quad: core::renderer::Quad, background: impl Into<Background>) {
        let opacity = self.layers.opacity();
        quad.border.color = quad.border.color.scale_alpha(opacity);
        quad.shadow.color = quad.shadow.color.scale_alpha(opacity);
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_quad(quad, background.into().scale_alpha(opacity), transformation);
    }

    fn replay_retained_layer(&mut self, key: u64, revision: u64, bounds: Rectangle) -> bool {
        let scale_factor = self.scale_factor;
        let had_cached_frame = self.retained_frame.is_some();

        if let Some(retained) = self.retained_frame.take()
            && retained.key == key
            && retained.revision == revision
            && retained.bounds == bounds
            && retained.scale_factor == scale_factor
        {
            let range = self.layers.replay_owned(retained.display_list);
            self.retained_layer_statistics.hits += 1;
            self.active_retained_frame = Some(ActiveRetainedFrame {
                key,
                revision,
                bounds,
                scale_factor,
                range,
            });
            return true;
        }

        self.retained_layer_statistics.misses += 1;
        if had_cached_frame {
            // The sole cached display list was incompatible with this draw
            // context and was dropped by `take` above.
            self.retained_layer_statistics.evictions += 1;
        }

        false
    }

    fn start_retained_layer(&mut self, key: u64, revision: u64, bounds: Rectangle) {
        let start = self.layers.active_count();
        self.layers.push_clip(bounds);
        self.recording_retained_frame = Some(RecordingRetainedFrame {
            key,
            revision,
            bounds,
            scale_factor: self.scale_factor,
            start,
        });
    }

    fn end_retained_layer(&mut self) {
        self.layers.pop_clip();
        let end = self.layers.active_count();
        let recording = self
            .recording_retained_frame
            .take()
            .expect("end_retained_layer without start_retained_layer");

        self.active_retained_frame = Some(ActiveRetainedFrame {
            key: recording.key,
            revision: recording.revision,
            bounds: recording.bounds,
            scale_factor: recording.scale_factor,
            range: recording.start..end,
        });
    }

    fn retained_layer_statistics(&self) -> core::renderer::RetainedLayerStatistics {
        self.retained_layer_statistics
    }

    fn allocate_image(
        &mut self,
        _handle: &core::image::Handle,
        _callback: impl FnOnce(Result<core::image::Allocation, core::image::Error>) + Send + 'static,
    ) {
        #[cfg(feature = "image")]
        self.image_cache
            .get_mut()
            .allocate_image(_handle, _callback);
    }

    fn hint(&mut self, scale_factor: f32) {
        self.scale_factor = Some(scale_factor);
    }

    fn scale_factor(&self) -> Option<f32> {
        Some(self.scale_factor? * self.layers.transformation().scale_factor())
    }

    fn set_light(&mut self, light: core::material::SceneLight) {
        self.light = light;
    }

    fn light(&self) -> core::material::SceneLight {
        self.light
    }

    fn tick(&mut self) {
        #[cfg(feature = "image")]
        self.image_cache.get_mut().receive();
    }

    fn reset(&mut self, new_bounds: Rectangle) {
        if let Some(active) = self.active_retained_frame.take() {
            let display_list = self.layers.take_range(active.range, new_bounds);
            self.retained_frame = Some(RetainedFrame {
                key: active.key,
                revision: active.revision,
                bounds: active.bounds,
                scale_factor: active.scale_factor,
                display_list,
            });
        } else {
            self.layers.reset(new_bounds);
        }
    }
}

impl core::text::Renderer for Renderer {
    type Font = Font;
    type Paragraph = Paragraph;
    type Editor = Editor;

    const ICON_FONT: Font = Font::new("Taim-Icons");
    const CHECKMARK_ICON: char = '\u{f00c}';
    const ARROW_DOWN_ICON: char = '\u{e800}';
    const TAIM_LOGO: char = '\u{e801}';
    const SCROLL_UP_ICON: char = '\u{e802}';
    const SCROLL_DOWN_ICON: char = '\u{e803}';
    const SCROLL_LEFT_ICON: char = '\u{e804}';
    const SCROLL_RIGHT_ICON: char = '\u{e805}';

    fn default_font(&self) -> Self::Font {
        self.settings.default_font
    }

    fn default_size(&self) -> Pixels {
        self.settings.default_text_size
    }

    fn fill_paragraph(
        &mut self,
        text: &Self::Paragraph,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    ) {
        let color = color.scale_alpha(self.layers.opacity());
        let (layer, transformation) = self.layers.current_mut();

        layer.draw_paragraph(text, position, color, clip_bounds, transformation);
    }

    fn fill_editor(
        &mut self,
        editor: &Self::Editor,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    ) {
        let color = color.scale_alpha(self.layers.opacity());
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_editor(editor, position, color, clip_bounds, transformation);
    }

    fn fill_text(
        &mut self,
        text: core::Text,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    ) {
        let color = color.scale_alpha(self.layers.opacity());
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_text(text, position, color, clip_bounds, transformation);
    }
}

impl graphics::text::Renderer for Renderer {
    fn fill_raw(&mut self, mut raw: graphics::text::Raw) {
        raw.color = raw.color.scale_alpha(self.layers.opacity());
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_text_raw(raw, transformation);
    }
}

#[cfg(feature = "image")]
impl core::image::Renderer for Renderer {
    type Handle = core::image::Handle;

    fn load_image(
        &self,
        handle: &Self::Handle,
    ) -> Result<core::image::Allocation, core::image::Error> {
        self.image_cache
            .borrow_mut()
            .load_image(&self.engine.device, &self.engine.queue, handle)
    }

    fn measure_image(&self, handle: &Self::Handle) -> Option<core::Size<u32>> {
        self.image_cache.borrow_mut().measure_image(handle)
    }

    fn draw_image(&mut self, mut image: core::Image, bounds: Rectangle, clip_bounds: Rectangle) {
        image.opacity *= self.layers.opacity();
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_raster(image, bounds, clip_bounds, transformation);
    }
}

#[cfg(feature = "svg")]
impl core::svg::Renderer for Renderer {
    fn measure_svg(&self, handle: &core::svg::Handle) -> core::Size<u32> {
        self.image_cache.borrow_mut().measure_svg(handle)
    }

    fn draw_svg(&mut self, mut svg: core::Svg, bounds: Rectangle, clip_bounds: Rectangle) {
        svg.opacity *= self.layers.opacity();
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_svg(svg, bounds, clip_bounds, transformation);
    }
}

impl graphics::mesh::Renderer for Renderer {
    fn draw_mesh(&mut self, mesh: graphics::Mesh) {
        debug_assert!(
            !mesh.indices().is_empty(),
            "Mesh must not have empty indices"
        );

        debug_assert!(
            mesh.indices().len().is_multiple_of(3),
            "Mesh indices length must be a multiple of 3"
        );

        let mesh = mesh.scale_alpha(self.layers.opacity());
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_mesh(mesh, transformation);
    }

    fn draw_mesh_cache(&mut self, cache: mesh::Cache) {
        let cache = cache.scale_alpha(self.layers.opacity());
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_mesh_cache(cache, transformation);
    }
}

#[cfg(feature = "geometry")]
impl graphics::geometry::Renderer for Renderer {
    type Geometry = Geometry;
    type Frame = geometry::Frame;

    fn new_frame(&self, bounds: Rectangle) -> Self::Frame {
        geometry::Frame::new(bounds)
    }

    fn draw_geometry(&mut self, geometry: Self::Geometry) {
        let opacity = self.layers.opacity();
        let (layer, transformation) = self.layers.current_mut();

        match geometry {
            Geometry::Live {
                meshes,
                images,
                text,
            } => {
                layer.draw_mesh_group(
                    meshes
                        .into_iter()
                        .map(|mesh| mesh.scale_alpha(opacity))
                        .collect(),
                    transformation,
                );

                for image in images {
                    layer.draw_image(image.scale_alpha(opacity), transformation);
                }

                layer.draw_text_group(
                    text.into_iter()
                        .map(|text| text.scale_alpha(opacity))
                        .collect(),
                    transformation,
                );
            }
            Geometry::Cached(cache) => {
                if let Some(meshes) = cache.meshes {
                    layer.draw_mesh_cache(meshes.scale_alpha(opacity), transformation);
                }

                if let Some(images) = cache.images {
                    for image in images.iter().cloned() {
                        layer.draw_image(image.scale_alpha(opacity), transformation);
                    }
                }

                if let Some(text) = cache.text {
                    layer.draw_text_cache(text.scale_alpha(opacity), transformation);
                }
            }
        }
    }
}

impl primitive::Renderer for Renderer {
    fn draw_primitive(&mut self, bounds: Rectangle, primitive: impl Primitive) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_primitive(bounds, primitive, transformation);
    }
}

impl graphics::compositor::Default for crate::Renderer {
    type Compositor = window::Compositor;
}

/// `None`, honestly: this backend shapes glyphs through `cryoglyph`'s
/// own GPU-side cache, not `graphics::text::Cache` — a different
/// structure this trait's default answer knows nothing about, not a gap
/// waiting to be filled in. See [`graphics::text::CacheStatistics`]'s
/// own documentation.
impl graphics::text::CacheStatistics for Renderer {}

impl renderer::Headless for Renderer {
    async fn new(settings: renderer::Settings, backend: Option<&str>) -> Option<Self> {
        if backend.is_some_and(|backend| backend != "wgpu") {
            return None;
        }

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY),
            flags: wgpu::InstanceFlags::empty(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .ok()?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("taim_wgpu [headless]"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits {
                    max_bind_groups: 2,
                    ..wgpu::Limits::default()
                },
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            })
            .await
            .ok()?;

        let engine = Engine::new(
            &adapter,
            device,
            queue,
            if graphics::color::GAMMA_CORRECTION {
                wgpu::TextureFormat::Rgba8UnormSrgb
            } else {
                wgpu::TextureFormat::Rgba8Unorm
            },
            Some(graphics::Antialiasing::MSAAx4),
            Shell::headless(),
        );

        Some(Self::new(engine, settings))
    }

    fn name(&self) -> String {
        "wgpu".to_owned()
    }

    fn screenshot(
        &mut self,
        size: Size<u32>,
        scale_factor: f32,
        background_color: Color,
    ) -> Vec<u8> {
        self.screenshot(
            &Viewport::with_physical_size(size, scale_factor),
            background_color,
        )
    }
}

/// Intersects a scissor rectangle with the frame's damage, if any.
///
/// Returns `None` when they do not overlap, which means the layer has nothing
/// to contribute to this frame and can be skipped outright.
fn intersect(scissor: Rectangle<u32>, damage: Option<Rectangle<u32>>) -> Option<Rectangle<u32>> {
    let Some(damage) = damage else {
        return Some(scissor);
    };

    let x = scissor.x.max(damage.x);
    let y = scissor.y.max(damage.y);
    let right = (scissor.x + scissor.width).min(damage.x + damage.width);
    let bottom = (scissor.y + scissor.height).min(damage.y + damage.height);

    (right > x && bottom > y).then(|| Rectangle {
        x,
        y,
        width: right - x,
        height: bottom - y,
    })
}

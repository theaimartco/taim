#![allow(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]
pub mod window;

mod engine;
mod layer;
mod primitive;
mod text;

#[cfg(feature = "image")]
mod raster;

#[cfg(feature = "svg")]
mod vector;

#[cfg(feature = "geometry")]
pub mod geometry;

use aura_debug as debug;
pub use aura_graphics as graphics;
pub use aura_graphics::core;

pub use layer::Layer;
pub use primitive::Primitive;

#[cfg(feature = "geometry")]
pub use geometry::Geometry;

use crate::core::renderer;
use crate::core::{Background, Color, Font, Pixels, Point, Rectangle, Size, Transformation};
use crate::engine::Engine;
use crate::graphics::Viewport;
use crate::graphics::compositor;
use crate::graphics::text::{Editor, Paragraph};

#[derive(Debug)]
struct RetainedFrame {
    key: u64,
    revision: u64,
    bounds: Rectangle,
    display_list: layer::Stack,
}

#[derive(Debug)]
struct ActiveRetainedFrame {
    key: u64,
    revision: u64,
    bounds: Rectangle,
    range: std::ops::Range<usize>,
}

#[derive(Debug)]
struct RecordingRetainedFrame {
    key: u64,
    revision: u64,
    bounds: Rectangle,
    start: usize,
}

/// A [`tiny-skia`] graphics renderer for [`aura`].
///
/// [`tiny-skia`]: https://github.com/RazrFalcon/tiny-skia
/// [`aura`]: https://github.com/theaimartco/vyom-aura
#[derive(Debug)]
pub struct Renderer {
    settings: renderer::Settings,
    layers: layer::Stack,
    retained_frame: Option<RetainedFrame>,
    active_retained_frame: Option<ActiveRetainedFrame>,
    recording_retained_frame: Option<RecordingRetainedFrame>,
    engine: Engine, // TODO: Shared engine
}

impl Renderer {
    pub fn new(settings: renderer::Settings) -> Self {
        Self {
            settings,
            layers: layer::Stack::new(),
            retained_frame: None,
            active_retained_frame: None,
            recording_retained_frame: None,
            engine: Engine::new(),
        }
    }

    pub fn layers(&mut self) -> &[Layer] {
        self.layers.flush();
        self.layers.as_slice()
    }

    pub fn draw(
        &mut self,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: &mut tiny_skia::Mask,
        viewport: &Viewport,
        damage: &[Rectangle],
        background_color: Color,
    ) {
        let scale_factor = viewport.scale_factor();
        self.layers.flush();

        for &damage_bounds in damage {
            let damage_bounds = damage_bounds * scale_factor;

            let path = tiny_skia::PathBuilder::from_rect(
                tiny_skia::Rect::from_xywh(
                    damage_bounds.x,
                    damage_bounds.y,
                    damage_bounds.width,
                    damage_bounds.height,
                )
                .expect("Create damage rectangle"),
            );

            pixels.fill_path(
                &path,
                &tiny_skia::Paint {
                    shader: tiny_skia::Shader::SolidColor(engine::into_color(background_color)),
                    anti_alias: false,
                    blend_mode: tiny_skia::BlendMode::Source,
                    ..Default::default()
                },
                tiny_skia::FillRule::default(),
                tiny_skia::Transform::identity(),
                None,
            );

            for layer in self.layers.iter() {
                let Some(layer_bounds) = damage_bounds.intersection(&(layer.bounds * scale_factor))
                else {
                    continue;
                };

                engine::adjust_clip_mask(clip_mask, layer_bounds);

                if !layer.quads.is_empty() {
                    let render_span = debug::render(debug::Primitive::Quad);
                    for (quad, background) in &layer.quads {
                        self.engine.draw_quad(
                            quad,
                            background,
                            Transformation::scale(scale_factor),
                            pixels,
                            clip_mask,
                            layer_bounds,
                        );
                    }
                    render_span.finish();
                }

                if !layer.primitives.is_empty() {
                    let render_span = debug::render(debug::Primitive::Triangle);

                    for group in &layer.primitives {
                        let Some(group_bounds) =
                            (group.clip_bounds() * scale_factor).intersection(&layer_bounds)
                        else {
                            continue;
                        };

                        engine::adjust_clip_mask(clip_mask, group_bounds);

                        for primitive in group.as_slice() {
                            self.engine.draw_primitive(
                                primitive,
                                Transformation::scale(scale_factor) * group.transformation(),
                                pixels,
                                clip_mask,
                                group_bounds,
                            );
                        }

                        engine::adjust_clip_mask(clip_mask, layer_bounds);
                    }

                    render_span.finish();
                }

                if !layer.images.is_empty() {
                    let render_span = debug::render(debug::Primitive::Image);

                    for image in &layer.images {
                        self.engine.draw_image(
                            image,
                            Transformation::scale(scale_factor),
                            pixels,
                            clip_mask,
                            layer_bounds,
                        );
                    }

                    render_span.finish();
                }

                if !layer.text.is_empty() {
                    let render_span = debug::render(debug::Primitive::Image);

                    for group in &layer.text {
                        for text in group.as_slice() {
                            self.engine.draw_text(
                                text,
                                Transformation::scale(scale_factor) * group.transformation(),
                                pixels,
                                clip_mask,
                                layer_bounds,
                            );
                        }
                    }

                    render_span.finish();
                }
            }
        }

        self.engine.trim();
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

    fn fill_quad(&mut self, mut quad: renderer::Quad, background: impl Into<Background>) {
        let opacity = self.layers.opacity();
        quad.border.color = quad.border.color.scale_alpha(opacity);
        quad.shadow.color = quad.shadow.color.scale_alpha(opacity);
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_quad(quad, background.into().scale_alpha(opacity), transformation);
    }

    fn replay_retained_layer(&mut self, key: u64, revision: u64, bounds: Rectangle) -> bool {
        if let Some(retained) = self.retained_frame.take()
            && retained.key == key
            && retained.revision == revision
            && retained.bounds == bounds
        {
            let range = self.layers.replay_owned(retained.display_list);
            self.active_retained_frame = Some(ActiveRetainedFrame {
                key,
                revision,
                bounds,
                range,
            });
            return true;
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
            range: recording.start..end,
        });
    }

    fn allocate_image(
        &mut self,
        _handle: &core::image::Handle,
        callback: impl FnOnce(Result<core::image::Allocation, core::image::Error>) + Send + 'static,
    ) {
        #[cfg(feature = "image")]
        #[allow(unsafe_code)]
        // TODO: Concurrency
        callback(self.engine.raster_pipeline.load(_handle));

        #[cfg(not(feature = "image"))]
        callback(Err(core::image::Error::Unsupported));
    }

    fn hint(&mut self, _scale_factor: f32) {
        // TODO: No hinting supported
        // We'll replace `tiny-skia` with `vello_cpu` soon
    }

    fn scale_factor(&self) -> Option<f32> {
        None
    }

    fn reset(&mut self, new_bounds: Rectangle) {
        if let Some(active) = self.active_retained_frame.take() {
            let display_list = self.layers.take_range(active.range, new_bounds);
            self.retained_frame = Some(RetainedFrame {
                key: active.key,
                revision: active.revision,
                bounds: active.bounds,
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

    const ICON_FONT: Font = Font::new("Aura-Icons");
    const CHECKMARK_ICON: char = '\u{f00c}';
    const ARROW_DOWN_ICON: char = '\u{e800}';
    const AURA_LOGO: char = '\u{e801}';
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
                primitives,
                images,
                text,
                clip_bounds,
            } => {
                layer.draw_primitive_group(
                    primitives
                        .into_iter()
                        .map(|primitive| primitive.scale_alpha(opacity))
                        .collect(),
                    clip_bounds,
                    transformation,
                );

                for image in images {
                    layer.draw_image(image.scale_alpha(opacity), transformation);
                }

                layer.draw_text_group(
                    text.into_iter()
                        .map(|text| text.scale_alpha(opacity))
                        .collect(),
                    clip_bounds,
                    transformation,
                );
            }
            Geometry::Cache(cache) => {
                let primitives = cache
                    .primitives
                    .iter()
                    .cloned()
                    .map(|primitive| primitive.scale_alpha(opacity))
                    .collect();
                layer.draw_primitive_group(primitives, cache.clip_bounds, transformation);

                for image in cache.images.iter() {
                    layer.draw_image(image.clone().scale_alpha(opacity), transformation);
                }

                let text = cache
                    .text
                    .iter()
                    .cloned()
                    .map(|text| text.scale_alpha(opacity))
                    .collect();
                layer.draw_text_group(text, cache.clip_bounds, transformation);
            }
        }
    }
}

impl graphics::mesh::Renderer for Renderer {
    fn draw_mesh(&mut self, _mesh: graphics::Mesh) {
        log::warn!("aura_tiny_skia does not support drawing meshes");
    }

    fn draw_mesh_cache(&mut self, _cache: aura_graphics::mesh::Cache) {
        log::warn!("aura_tiny_skia does not support drawing meshes");
    }
}

#[cfg(feature = "image")]
impl core::image::Renderer for Renderer {
    type Handle = core::image::Handle;

    fn load_image(
        &self,
        handle: &Self::Handle,
    ) -> Result<core::image::Allocation, core::image::Error> {
        self.engine.raster_pipeline.load(handle)
    }

    fn measure_image(&self, handle: &Self::Handle) -> Option<crate::core::Size<u32>> {
        self.engine.raster_pipeline.dimensions(handle)
    }

    fn draw_image(&mut self, mut image: core::Image, bounds: Rectangle, clip_bounds: Rectangle) {
        image.opacity *= self.layers.opacity();
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_raster(image, bounds, clip_bounds, transformation);
    }
}

#[cfg(feature = "svg")]
impl core::svg::Renderer for Renderer {
    fn measure_svg(&self, handle: &core::svg::Handle) -> crate::core::Size<u32> {
        self.engine.vector_pipeline.viewport_dimensions(handle)
    }

    fn draw_svg(&mut self, mut svg: core::Svg, bounds: Rectangle, clip_bounds: Rectangle) {
        svg.opacity *= self.layers.opacity();
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_svg(svg, bounds, clip_bounds, transformation);
    }
}

impl compositor::Default for Renderer {
    type Compositor = window::Compositor;
}

impl crate::graphics::text::CacheStatistics for Renderer {
    fn text_cache_statistics(&self) -> Option<crate::graphics::text::cache::Statistics> {
        Some(self.engine.text_cache_statistics())
    }
}

impl renderer::Headless for Renderer {
    async fn new(settings: renderer::Settings, backend: Option<&str>) -> Option<Self> {
        if backend.is_some_and(|backend| !["tiny-skia", "tiny_skia", "software"].contains(&backend))
        {
            return None;
        }

        Some(Self::new(settings))
    }

    fn name(&self) -> String {
        "tiny-skia".to_owned()
    }

    fn screenshot(
        &mut self,
        size: Size<u32>,
        scale_factor: f32,
        background_color: Color,
    ) -> Vec<u8> {
        let viewport = Viewport::with_physical_size(size, scale_factor);

        window::compositor::screenshot(self, &viewport, background_color)
    }
}

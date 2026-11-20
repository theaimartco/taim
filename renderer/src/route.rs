//! Zero-allocation renderer routing with an explicit recovery backend.
//!
//! A route owns exactly one backend. The preferred backend is attempted first
//! during initialization; the recovery backend is constructed only when that
//! attempt fails. No trait object, heap allocation, or per-command indirect
//! dispatch is introduced by this layer.

use crate::core::backend;
use crate::core::font;
use crate::core::image;
use crate::core::renderer;
use crate::core::svg;
use crate::core::{self, Background, Color, Image, Point, Rectangle, Size, Svg, Transformation};
use crate::graphics::compositor;
use crate::graphics::mesh;
use crate::graphics::text;
use crate::graphics::{self, Shell};

use std::borrow::Cow;

/// Identifies which half of a [`Route`] is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RouteKind {
    /// The preferred, normally hardware-accelerated backend.
    Preferred,
    /// The recovery backend selected after preferred initialization failed.
    Recovery,
}

/// Stores one active implementation without boxing or allocating.
///
/// The inactive implementation does not occupy a second heap allocation: this
/// is a regular Rust enum whose size is the larger backend plus a discriminant.
#[derive(Debug, Clone)]
pub enum Route<Preferred, Recovery> {
    /// The preferred implementation is active.
    Preferred(Preferred),
    /// The recovery implementation is active.
    Recovery(Recovery),
}

impl<Preferred, Recovery> Route<Preferred, Recovery> {
    /// Returns the active route.
    pub const fn kind(&self) -> RouteKind {
        match self {
            Self::Preferred(_) => RouteKind::Preferred,
            Self::Recovery(_) => RouteKind::Recovery,
        }
    }

    /// Returns whether initialization selected the preferred backend.
    pub const fn is_preferred(&self) -> bool {
        matches!(self, Self::Preferred(_))
    }
}

/// The renderer selected for the current process.
pub type Renderer<Preferred, Recovery> = Route<Preferred, Recovery>;

/// The compositor selected for the current process.
pub type Compositor<Preferred, Recovery> = Route<Preferred, Recovery>;

/// A surface owned by the selected compositor.
pub type Surface<Preferred, Recovery> = Route<Preferred, Recovery>;

impl<Preferred, Recovery> text::CacheStatistics for Route<Preferred, Recovery>
where
    Preferred: text::CacheStatistics,
    Recovery: text::CacheStatistics,
{
    fn text_cache_statistics(&self) -> Option<text::cache::Statistics> {
        match self {
            Self::Preferred(renderer) => renderer.text_cache_statistics(),
            Self::Recovery(renderer) => renderer.text_cache_statistics(),
        }
    }
}

impl<Preferred, Recovery> core::Renderer for Route<Preferred, Recovery>
where
    Preferred: core::Renderer,
    Recovery: core::Renderer,
{
    fn start_layer(&mut self, bounds: Rectangle) {
        match self {
            Self::Preferred(renderer) => renderer.start_layer(bounds),
            Self::Recovery(renderer) => renderer.start_layer(bounds),
        }
    }

    fn end_layer(&mut self) {
        match self {
            Self::Preferred(renderer) => renderer.end_layer(),
            Self::Recovery(renderer) => renderer.end_layer(),
        }
    }

    fn start_transformation(&mut self, transformation: Transformation) {
        match self {
            Self::Preferred(renderer) => renderer.start_transformation(transformation),
            Self::Recovery(renderer) => renderer.start_transformation(transformation),
        }
    }

    fn end_transformation(&mut self) {
        match self {
            Self::Preferred(renderer) => renderer.end_transformation(),
            Self::Recovery(renderer) => renderer.end_transformation(),
        }
    }

    fn start_opacity(&mut self, opacity: f32) {
        match self {
            Self::Preferred(renderer) => renderer.start_opacity(opacity),
            Self::Recovery(renderer) => renderer.start_opacity(opacity),
        }
    }

    fn start_opacity_reset(&mut self, opacity: f32) {
        match self {
            Self::Preferred(renderer) => renderer.start_opacity_reset(opacity),
            Self::Recovery(renderer) => renderer.start_opacity_reset(opacity),
        }
    }

    fn end_opacity(&mut self) {
        match self {
            Self::Preferred(renderer) => renderer.end_opacity(),
            Self::Recovery(renderer) => renderer.end_opacity(),
        }
    }

    fn replay_retained_layer(&mut self, key: u64, revision: u64, bounds: Rectangle) -> bool {
        match self {
            Self::Preferred(renderer) => renderer.replay_retained_layer(key, revision, bounds),
            Self::Recovery(renderer) => renderer.replay_retained_layer(key, revision, bounds),
        }
    }

    fn start_retained_layer(&mut self, key: u64, revision: u64, bounds: Rectangle) {
        match self {
            Self::Preferred(renderer) => renderer.start_retained_layer(key, revision, bounds),
            Self::Recovery(renderer) => renderer.start_retained_layer(key, revision, bounds),
        }
    }

    fn end_retained_layer(&mut self) {
        match self {
            Self::Preferred(renderer) => renderer.end_retained_layer(),
            Self::Recovery(renderer) => renderer.end_retained_layer(),
        }
    }

    fn retained_layer_statistics(&self) -> renderer::RetainedLayerStatistics {
        match self {
            Self::Preferred(renderer) => renderer.retained_layer_statistics(),
            Self::Recovery(renderer) => renderer.retained_layer_statistics(),
        }
    }

    fn fill_quad(&mut self, quad: renderer::Quad, background: impl Into<Background>) {
        let background = background.into();

        match self {
            Self::Preferred(renderer) => renderer.fill_quad(quad, background),
            Self::Recovery(renderer) => renderer.fill_quad(quad, background),
        }
    }

    fn allocate_image(
        &mut self,
        handle: &image::Handle,
        callback: impl FnOnce(Result<image::Allocation, image::Error>) + Send + 'static,
    ) {
        match self {
            Self::Preferred(renderer) => renderer.allocate_image(handle, callback),
            Self::Recovery(renderer) => renderer.allocate_image(handle, callback),
        }
    }

    fn hint(&mut self, scale_factor: f32) {
        match self {
            Self::Preferred(renderer) => renderer.hint(scale_factor),
            Self::Recovery(renderer) => renderer.hint(scale_factor),
        }
    }

    fn scale_factor(&self) -> Option<f32> {
        match self {
            Self::Preferred(renderer) => renderer.scale_factor(),
            Self::Recovery(renderer) => renderer.scale_factor(),
        }
    }

    fn reset(&mut self, new_bounds: Rectangle) {
        match self {
            Self::Preferred(renderer) => renderer.reset(new_bounds),
            Self::Recovery(renderer) => renderer.reset(new_bounds),
        }
    }

    fn set_light(&mut self, light: core::material::SceneLight) {
        match self {
            Self::Preferred(renderer) => renderer.set_light(light),
            Self::Recovery(renderer) => renderer.set_light(light),
        }
    }

    fn light(&self) -> core::material::SceneLight {
        match self {
            Self::Preferred(renderer) => renderer.light(),
            Self::Recovery(renderer) => renderer.light(),
        }
    }

    fn tick(&mut self) {
        match self {
            Self::Preferred(renderer) => renderer.tick(),
            Self::Recovery(renderer) => renderer.tick(),
        }
    }
}

impl<Preferred, Recovery> core::text::Renderer for Route<Preferred, Recovery>
where
    Preferred: core::text::Renderer,
    Recovery: core::text::Renderer<
            Font = Preferred::Font,
            Paragraph = Preferred::Paragraph,
            Editor = Preferred::Editor,
        >,
{
    type Font = Preferred::Font;
    type Paragraph = Preferred::Paragraph;
    type Editor = Preferred::Editor;

    const ICON_FONT: Self::Font = Preferred::ICON_FONT;
    const CHECKMARK_ICON: char = Preferred::CHECKMARK_ICON;
    const ARROW_DOWN_ICON: char = Preferred::ARROW_DOWN_ICON;
    const SCROLL_UP_ICON: char = Preferred::SCROLL_UP_ICON;
    const SCROLL_DOWN_ICON: char = Preferred::SCROLL_DOWN_ICON;
    const SCROLL_LEFT_ICON: char = Preferred::SCROLL_LEFT_ICON;
    const SCROLL_RIGHT_ICON: char = Preferred::SCROLL_RIGHT_ICON;
    const TAIM_LOGO: char = Preferred::TAIM_LOGO;

    fn default_font(&self) -> Self::Font {
        match self {
            Self::Preferred(renderer) => renderer.default_font(),
            Self::Recovery(renderer) => renderer.default_font(),
        }
    }

    fn default_size(&self) -> core::Pixels {
        match self {
            Self::Preferred(renderer) => renderer.default_size(),
            Self::Recovery(renderer) => renderer.default_size(),
        }
    }

    fn fill_paragraph(
        &mut self,
        text: &Self::Paragraph,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    ) {
        match self {
            Self::Preferred(renderer) => {
                renderer.fill_paragraph(text, position, color, clip_bounds);
            }
            Self::Recovery(renderer) => {
                renderer.fill_paragraph(text, position, color, clip_bounds);
            }
        }
    }

    fn fill_editor(
        &mut self,
        editor: &Self::Editor,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    ) {
        match self {
            Self::Preferred(renderer) => {
                renderer.fill_editor(editor, position, color, clip_bounds);
            }
            Self::Recovery(renderer) => {
                renderer.fill_editor(editor, position, color, clip_bounds);
            }
        }
    }

    fn fill_text(
        &mut self,
        text: core::Text<String, Self::Font>,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    ) {
        match self {
            Self::Preferred(renderer) => renderer.fill_text(text, position, color, clip_bounds),
            Self::Recovery(renderer) => renderer.fill_text(text, position, color, clip_bounds),
        }
    }
}

impl<Preferred, Recovery> text::Renderer for Route<Preferred, Recovery>
where
    Preferred: text::Renderer,
    Recovery: text::Renderer,
{
    fn fill_raw(&mut self, raw: text::Raw) {
        match self {
            Self::Preferred(renderer) => renderer.fill_raw(raw),
            Self::Recovery(renderer) => renderer.fill_raw(raw),
        }
    }
}

impl<Preferred, Recovery> image::Renderer for Route<Preferred, Recovery>
where
    Preferred: image::Renderer,
    Recovery: image::Renderer<Handle = Preferred::Handle>,
{
    type Handle = Preferred::Handle;

    fn load_image(&self, handle: &Self::Handle) -> Result<image::Allocation, image::Error> {
        match self {
            Self::Preferred(renderer) => renderer.load_image(handle),
            Self::Recovery(renderer) => renderer.load_image(handle),
        }
    }

    fn measure_image(&self, handle: &Self::Handle) -> Option<Size<u32>> {
        match self {
            Self::Preferred(renderer) => renderer.measure_image(handle),
            Self::Recovery(renderer) => renderer.measure_image(handle),
        }
    }

    fn draw_image(
        &mut self,
        image: Image<Preferred::Handle>,
        bounds: Rectangle,
        clip_bounds: Rectangle,
    ) {
        match self {
            Self::Preferred(renderer) => renderer.draw_image(image, bounds, clip_bounds),
            Self::Recovery(renderer) => renderer.draw_image(image, bounds, clip_bounds),
        }
    }
}

impl<Preferred, Recovery> svg::Renderer for Route<Preferred, Recovery>
where
    Preferred: svg::Renderer,
    Recovery: svg::Renderer,
{
    fn measure_svg(&self, handle: &svg::Handle) -> Size<u32> {
        match self {
            Self::Preferred(renderer) => renderer.measure_svg(handle),
            Self::Recovery(renderer) => renderer.measure_svg(handle),
        }
    }

    fn draw_svg(&mut self, svg: Svg, bounds: Rectangle, clip_bounds: Rectangle) {
        match self {
            Self::Preferred(renderer) => renderer.draw_svg(svg, bounds, clip_bounds),
            Self::Recovery(renderer) => renderer.draw_svg(svg, bounds, clip_bounds),
        }
    }
}

impl<Preferred, Recovery> mesh::Renderer for Route<Preferred, Recovery>
where
    Preferred: mesh::Renderer,
    Recovery: mesh::Renderer,
{
    fn draw_mesh(&mut self, mesh: graphics::Mesh) {
        match self {
            Self::Preferred(renderer) => renderer.draw_mesh(mesh),
            Self::Recovery(renderer) => renderer.draw_mesh(mesh),
        }
    }

    fn draw_mesh_cache(&mut self, cache: mesh::Cache) {
        match self {
            Self::Preferred(renderer) => renderer.draw_mesh_cache(cache),
            Self::Recovery(renderer) => renderer.draw_mesh_cache(cache),
        }
    }
}

impl<Preferred, Recovery> graphics::Compositor for Route<Preferred, Recovery>
where
    Preferred: graphics::Compositor,
    Recovery: graphics::Compositor,
{
    type Renderer = Route<Preferred::Renderer, Recovery::Renderer>;
    type Surface = Route<Preferred::Surface, Recovery::Surface>;

    async fn new(
        settings: backend::Settings,
        display: impl compositor::Display + Clone,
        compatible_window: impl compositor::Window + Clone,
        shell: Shell,
    ) -> Result<Self, backend::Error> {
        let preferred_error = match Preferred::new(
            settings.clone(),
            display.clone(),
            compatible_window.clone(),
            shell.clone(),
        )
        .await
        {
            Ok(compositor) => return Ok(Self::Preferred(compositor)),
            Err(error) => error,
        };

        match Recovery::new(settings, display, compatible_window, shell).await {
            Ok(compositor) => Ok(Self::Recovery(compositor)),
            Err(recovery_error) => Err(backend::Error::List(vec![preferred_error, recovery_error])),
        }
    }

    fn create_renderer(&self, settings: renderer::Settings) -> Self::Renderer {
        match self {
            Self::Preferred(compositor) => Route::Preferred(compositor.create_renderer(settings)),
            Self::Recovery(compositor) => Route::Recovery(compositor.create_renderer(settings)),
        }
    }

    fn create_surface(
        &mut self,
        window: impl compositor::Window + Clone,
        width: u32,
        height: u32,
    ) -> Result<Self::Surface, backend::Error> {
        Ok(match self {
            Self::Preferred(compositor) => {
                Route::Preferred(compositor.create_surface(window, width, height)?)
            }
            Self::Recovery(compositor) => {
                Route::Recovery(compositor.create_surface(window, width, height)?)
            }
        })
    }

    fn configure_surface(&mut self, surface: &mut Self::Surface, width: u32, height: u32) {
        match (self, surface) {
            (Self::Preferred(compositor), Route::Preferred(surface)) => {
                compositor.configure_surface(surface, width, height);
            }
            (Self::Recovery(compositor), Route::Recovery(surface)) => {
                compositor.configure_surface(surface, width, height);
            }
            _ => debug_assert!(false, "TAIM renderer route invariant was violated"),
        }
    }

    fn is_device_lost(&self) -> bool {
        match self {
            Self::Preferred(compositor) => compositor.is_device_lost(),
            Self::Recovery(compositor) => compositor.is_device_lost(),
        }
    }

    fn information(&self) -> compositor::Information {
        match self {
            Self::Preferred(compositor) => compositor.information(),
            Self::Recovery(compositor) => compositor.information(),
        }
    }

    fn load_font(&mut self, font: Cow<'static, [u8]>) -> Result<(), font::Error> {
        match self {
            Self::Preferred(compositor) => compositor.load_font(font),
            Self::Recovery(compositor) => compositor.load_font(font),
        }
    }

    fn list_fonts(&mut self) -> Result<Vec<font::Family>, font::Error> {
        match self {
            Self::Preferred(compositor) => compositor.list_fonts(),
            Self::Recovery(compositor) => compositor.list_fonts(),
        }
    }

    fn present(
        &mut self,
        renderer: &mut Self::Renderer,
        surface: &mut Self::Surface,
        viewport: &graphics::Viewport,
        background_color: Color,
        on_pre_present: impl FnOnce(),
    ) -> Result<(), compositor::SurfaceError> {
        match (self, renderer, surface) {
            (
                Self::Preferred(compositor),
                Route::Preferred(renderer),
                Route::Preferred(surface),
            ) => compositor.present(
                renderer,
                surface,
                viewport,
                background_color,
                on_pre_present,
            ),
            (Self::Recovery(compositor), Route::Recovery(renderer), Route::Recovery(surface)) => {
                compositor.present(
                    renderer,
                    surface,
                    viewport,
                    background_color,
                    on_pre_present,
                )
            }
            _ => Err(compositor::SurfaceError::Other),
        }
    }

    fn present_with_damage(
        &mut self,
        renderer: &mut Self::Renderer,
        surface: &mut Self::Surface,
        viewport: &graphics::Viewport,
        background_color: Color,
        frame_context: &compositor::FrameContext,
        on_pre_present: impl FnOnce(),
    ) -> Result<(), compositor::SurfaceError> {
        match (self, renderer, surface) {
            (
                Self::Preferred(compositor),
                Route::Preferred(renderer),
                Route::Preferred(surface),
            ) => compositor.present_with_damage(
                renderer,
                surface,
                viewport,
                background_color,
                frame_context,
                on_pre_present,
            ),
            (Self::Recovery(compositor), Route::Recovery(renderer), Route::Recovery(surface)) => {
                compositor.present_with_damage(
                    renderer,
                    surface,
                    viewport,
                    background_color,
                    frame_context,
                    on_pre_present,
                )
            }
            _ => Err(compositor::SurfaceError::Other),
        }
    }

    fn screenshot(
        &mut self,
        renderer: &mut Self::Renderer,
        viewport: &graphics::Viewport,
        background_color: Color,
    ) -> Vec<u8> {
        match (self, renderer) {
            (Self::Preferred(compositor), Route::Preferred(renderer)) => {
                compositor.screenshot(renderer, viewport, background_color)
            }
            (Self::Recovery(compositor), Route::Recovery(renderer)) => {
                compositor.screenshot(renderer, viewport, background_color)
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(feature = "wgpu-bare")]
impl<Preferred, Recovery> taim_wgpu::primitive::Renderer for Route<Preferred, Recovery>
where
    Preferred: taim_wgpu::primitive::Renderer,
    Recovery: core::Renderer,
{
    fn draw_primitive(&mut self, bounds: Rectangle, primitive: impl taim_wgpu::Primitive) {
        match self {
            Self::Preferred(renderer) => renderer.draw_primitive(bounds, primitive),
            Self::Recovery(_) => {
                log::warn!("TAIM recovery renderer cannot execute a custom GPU primitive");
            }
        }
    }
}

#[cfg(feature = "geometry")]
mod geometry {
    use super::Route;
    use crate::core::{Point, Radians, Rectangle, Size, Svg, Vector};
    use crate::graphics::cache::{self, Cached};
    use crate::graphics::geometry::{self, Fill, Image, Path, Stroke, Text};

    impl<Preferred, Recovery> geometry::Renderer for Route<Preferred, Recovery>
    where
        Preferred: geometry::Renderer,
        Recovery: geometry::Renderer,
    {
        type Geometry = Route<Preferred::Geometry, Recovery::Geometry>;
        type Frame = Route<Preferred::Frame, Recovery::Frame>;

        fn new_frame(&self, bounds: Rectangle) -> Self::Frame {
            match self {
                Self::Preferred(renderer) => Route::Preferred(renderer.new_frame(bounds)),
                Self::Recovery(renderer) => Route::Recovery(renderer.new_frame(bounds)),
            }
        }

        fn draw_geometry(&mut self, geometry: Self::Geometry) {
            match (self, geometry) {
                (Self::Preferred(renderer), Route::Preferred(geometry)) => {
                    renderer.draw_geometry(geometry);
                }
                (Self::Recovery(renderer), Route::Recovery(geometry)) => {
                    renderer.draw_geometry(geometry);
                }
                _ => debug_assert!(false, "geometry belongs to a different TAIM route"),
            }
        }
    }

    impl<Preferred, Recovery> Cached for Route<Preferred, Recovery>
    where
        Preferred: Cached,
        Recovery: Cached,
    {
        type Cache = Route<Preferred::Cache, Recovery::Cache>;

        fn load(cache: &Self::Cache) -> Self {
            match cache {
                Route::Preferred(cache) => Route::Preferred(Preferred::load(cache)),
                Route::Recovery(cache) => Route::Recovery(Recovery::load(cache)),
            }
        }

        fn cache(self, group: cache::Group, previous: Option<Self::Cache>) -> Self::Cache {
            match (self, previous) {
                (Route::Preferred(geometry), Some(Route::Preferred(previous))) => {
                    Route::Preferred(geometry.cache(group, Some(previous)))
                }
                (Route::Recovery(geometry), Some(Route::Recovery(previous))) => {
                    Route::Recovery(geometry.cache(group, Some(previous)))
                }
                (Route::Preferred(geometry), _) => Route::Preferred(geometry.cache(group, None)),
                (Route::Recovery(geometry), _) => Route::Recovery(geometry.cache(group, None)),
            }
        }
    }

    impl<Preferred, Recovery> geometry::frame::Backend for Route<Preferred, Recovery>
    where
        Preferred: geometry::frame::Backend,
        Recovery: geometry::frame::Backend,
    {
        type Geometry = Route<Preferred::Geometry, Recovery::Geometry>;

        fn width(&self) -> f32 {
            match self {
                Self::Preferred(frame) => frame.width(),
                Self::Recovery(frame) => frame.width(),
            }
        }

        fn height(&self) -> f32 {
            match self {
                Self::Preferred(frame) => frame.height(),
                Self::Recovery(frame) => frame.height(),
            }
        }

        fn size(&self) -> Size {
            match self {
                Self::Preferred(frame) => frame.size(),
                Self::Recovery(frame) => frame.size(),
            }
        }

        fn center(&self) -> Point {
            match self {
                Self::Preferred(frame) => frame.center(),
                Self::Recovery(frame) => frame.center(),
            }
        }

        fn fill(&mut self, path: &Path, fill: impl Into<Fill>) {
            let fill = fill.into();
            match self {
                Self::Preferred(frame) => frame.fill(path, fill),
                Self::Recovery(frame) => frame.fill(path, fill),
            }
        }

        fn fill_rectangle(&mut self, top_left: Point, size: Size, fill: impl Into<Fill>) {
            let fill = fill.into();
            match self {
                Self::Preferred(frame) => frame.fill_rectangle(top_left, size, fill),
                Self::Recovery(frame) => frame.fill_rectangle(top_left, size, fill),
            }
        }

        fn stroke<'a>(&mut self, path: &Path, stroke: impl Into<Stroke<'a>>) {
            let stroke = stroke.into();
            match self {
                Self::Preferred(frame) => frame.stroke(path, stroke),
                Self::Recovery(frame) => frame.stroke(path, stroke),
            }
        }

        fn stroke_rectangle<'a>(
            &mut self,
            top_left: Point,
            size: Size,
            stroke: impl Into<Stroke<'a>>,
        ) {
            let stroke = stroke.into();
            match self {
                Self::Preferred(frame) => frame.stroke_rectangle(top_left, size, stroke),
                Self::Recovery(frame) => frame.stroke_rectangle(top_left, size, stroke),
            }
        }

        fn stroke_text<'a>(&mut self, text: impl Into<Text>, stroke: impl Into<Stroke<'a>>) {
            let text = text.into();
            let stroke = stroke.into();
            match self {
                Self::Preferred(frame) => frame.stroke_text(text, stroke),
                Self::Recovery(frame) => frame.stroke_text(text, stroke),
            }
        }

        fn fill_text(&mut self, text: impl Into<Text>) {
            let text = text.into();
            match self {
                Self::Preferred(frame) => frame.fill_text(text),
                Self::Recovery(frame) => frame.fill_text(text),
            }
        }

        fn draw_image(&mut self, bounds: Rectangle, image: impl Into<Image>) {
            let image = image.into();
            match self {
                Self::Preferred(frame) => frame.draw_image(bounds, image),
                Self::Recovery(frame) => frame.draw_image(bounds, image),
            }
        }

        fn draw_svg(&mut self, bounds: Rectangle, svg: impl Into<Svg>) {
            let svg = svg.into();
            match self {
                Self::Preferred(frame) => frame.draw_svg(bounds, svg),
                Self::Recovery(frame) => frame.draw_svg(bounds, svg),
            }
        }

        fn push_transform(&mut self) {
            match self {
                Self::Preferred(frame) => frame.push_transform(),
                Self::Recovery(frame) => frame.push_transform(),
            }
        }

        fn pop_transform(&mut self) {
            match self {
                Self::Preferred(frame) => frame.pop_transform(),
                Self::Recovery(frame) => frame.pop_transform(),
            }
        }

        fn draft(&mut self, bounds: Rectangle) -> Self {
            match self {
                Self::Preferred(frame) => Self::Preferred(frame.draft(bounds)),
                Self::Recovery(frame) => Self::Recovery(frame.draft(bounds)),
            }
        }

        fn paste(&mut self, frame: Self) {
            match (self, frame) {
                (Self::Preferred(target), Self::Preferred(source)) => target.paste(source),
                (Self::Recovery(target), Self::Recovery(source)) => target.paste(source),
                _ => debug_assert!(false, "frame belongs to a different TAIM route"),
            }
        }

        fn translate(&mut self, translation: Vector) {
            match self {
                Self::Preferred(frame) => frame.translate(translation),
                Self::Recovery(frame) => frame.translate(translation),
            }
        }

        fn rotate(&mut self, angle: impl Into<Radians>) {
            let angle = angle.into();
            match self {
                Self::Preferred(frame) => frame.rotate(angle),
                Self::Recovery(frame) => frame.rotate(angle),
            }
        }

        fn scale(&mut self, scale: f32) {
            match self {
                Self::Preferred(frame) => frame.scale(scale),
                Self::Recovery(frame) => frame.scale(scale),
            }
        }

        fn scale_nonuniform(&mut self, scale: impl Into<Vector>) {
            let scale = scale.into();
            match self {
                Self::Preferred(frame) => frame.scale_nonuniform(scale),
                Self::Recovery(frame) => frame.scale_nonuniform(scale),
            }
        }

        fn into_geometry(self) -> Self::Geometry {
            match self {
                Self::Preferred(frame) => Route::Preferred(frame.into_geometry()),
                Self::Recovery(frame) => Route::Recovery(frame.into_geometry()),
            }
        }
    }
}

impl<Preferred, Recovery> renderer::Headless for Route<Preferred, Recovery>
where
    Preferred: renderer::Headless,
    Recovery: renderer::Headless,
{
    async fn new(settings: renderer::Settings, backend: Option<&str>) -> Option<Self> {
        if let Some(renderer) = Preferred::new(settings, backend).await {
            return Some(Self::Preferred(renderer));
        }

        Recovery::new(settings, backend).await.map(Self::Recovery)
    }

    fn name(&self) -> String {
        match self {
            Self::Preferred(renderer) => renderer.name(),
            Self::Recovery(renderer) => renderer.name(),
        }
    }

    fn screenshot(
        &mut self,
        size: Size<u32>,
        scale_factor: f32,
        background_color: Color,
    ) -> Vec<u8> {
        match self {
            Self::Preferred(renderer) => renderer.screenshot(size, scale_factor, background_color),
            Self::Recovery(renderer) => renderer.screenshot(size, scale_factor, background_color),
        }
    }
}

impl<Preferred, Recovery> compositor::Default for Route<Preferred, Recovery>
where
    Preferred: compositor::Default,
    Recovery: compositor::Default,
{
    type Compositor = Route<Preferred::Compositor, Recovery::Compositor>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct LightRenderer(core::material::SceneLight);

    impl Default for LightRenderer {
        fn default() -> Self {
            Self(core::material::SceneLight::KEY)
        }
    }

    impl core::Renderer for LightRenderer {
        fn start_layer(&mut self, _bounds: Rectangle) {}
        fn end_layer(&mut self) {}
        fn start_transformation(&mut self, _transformation: Transformation) {}
        fn end_transformation(&mut self) {}
        fn fill_quad(&mut self, _quad: renderer::Quad, _background: impl Into<Background>) {}

        fn allocate_image(
            &mut self,
            _handle: &image::Handle,
            _callback: impl FnOnce(Result<image::Allocation, image::Error>) + Send + 'static,
        ) {
        }

        fn hint(&mut self, _scale_factor: f32) {}
        fn scale_factor(&self) -> Option<f32> {
            None
        }
        fn reset(&mut self, _new_bounds: Rectangle) {}
        fn set_light(&mut self, light: core::material::SceneLight) {
            self.0 = light;
        }
        fn light(&self) -> core::material::SceneLight {
            self.0
        }
    }

    #[test]
    fn light_state_follows_the_selected_route() {
        let key = core::material::SceneLight::KEY;
        let moved = core::material::SceneLight {
            direction: [-key.direction[0], key.direction[1], key.direction[2]],
            ..key
        };

        let mut preferred: Route<LightRenderer, LightRenderer> =
            Route::Preferred(LightRenderer::default());
        core::Renderer::set_light(&mut preferred, moved);
        assert_eq!(preferred.kind(), RouteKind::Preferred);
        assert_eq!(core::Renderer::light(&preferred), moved);

        let mut recovery: Route<LightRenderer, LightRenderer> =
            Route::Recovery(LightRenderer::default());
        core::Renderer::set_light(&mut recovery, moved);
        assert_eq!(recovery.kind(), RouteKind::Recovery);
        assert_eq!(core::Renderer::light(&recovery), moved);
    }
}

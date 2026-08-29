//! Write your own renderer.
#[cfg(debug_assertions)]
mod null;

use crate::image;
use crate::{
    Background, Border, Color, Font, Pixels, Rectangle, Shadow, Size, Transformation, Vector,
};

/// Whether anti-aliasing should be avoided by snapping primitive coordinates to the
/// pixel grid.
pub const CRISP: bool = cfg!(feature = "crisp");

/// Snapshot of retained-layer reuse in a renderer.
///
/// These counters are monotonic for the renderer's lifetime. They make
/// retained rendering observable to diagnostics without exposing a backend's
/// display-list implementation or cache ownership.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RetainedLayerStatistics {
    /// Number of display lists replayed without re-entering widget paint.
    pub hits: u64,
    /// Number of replay attempts that required a fresh recording.
    pub misses: u64,
    /// Number of cached display lists discarded as stale or incompatible.
    pub evictions: u64,
}

/// A component that can be used by widgets to draw themselves on a screen.
pub trait Renderer {
    /// Starts recording a new layer.
    fn start_layer(&mut self, bounds: Rectangle);

    /// Ends recording a new layer.
    ///
    /// The new layer will clip its contents to the provided `bounds`.
    fn end_layer(&mut self);

    /// Draws the primitives recorded in the given closure in a new layer.
    ///
    /// The layer will clip its contents to the provided `bounds`.
    fn with_layer(&mut self, bounds: Rectangle, f: impl FnOnce(&mut Self)) {
        self.start_layer(bounds);
        f(self);
        self.end_layer();
    }

    /// Starts recording with a new [`Transformation`].
    fn start_transformation(&mut self, transformation: Transformation);

    /// Ends recording a new layer.
    ///
    /// The new layer will clip its contents to the provided `bounds`.
    fn end_transformation(&mut self);

    /// Applies a [`Transformation`] to the primitives recorded in the given closure.
    fn with_transformation(&mut self, transformation: Transformation, f: impl FnOnce(&mut Self)) {
        self.start_transformation(transformation);
        f(self);
        self.end_transformation();
    }

    /// Applies a translation to the primitives recorded in the given closure.
    fn with_translation(&mut self, translation: Vector, f: impl FnOnce(&mut Self)) {
        self.with_transformation(Transformation::translate(translation.x, translation.y), f);
    }

    /// Applies an opacity multiplier to all primitives recorded by `f`.
    fn with_opacity(&mut self, opacity: f32, f: impl FnOnce(&mut Self)) {
        self.start_opacity(opacity);
        f(self);
        self.end_opacity();
    }

    /// Pushes an opacity multiplier for subsequent primitives.
    fn start_opacity(&mut self, opacity: f32) {
        let _ = opacity;
    }

    /// Pops the current opacity multiplier.
    fn end_opacity(&mut self) {}

    /// Attempts to replay a retained display list for a stable subtree.
    fn replay_retained_layer(&mut self, key: u64, revision: u64, bounds: Rectangle) -> bool {
        let _ = (key, revision, bounds);
        false
    }

    /// Starts recording a retained display list after a replay miss.
    fn start_retained_layer(&mut self, key: u64, revision: u64, bounds: Rectangle) {
        let _ = (key, revision);
        self.start_layer(bounds);
    }

    /// Finishes recording a retained display list.
    fn end_retained_layer(&mut self) {
        self.end_layer();
    }

    /// Returns retained-layer reuse counters collected by this renderer.
    fn retained_layer_statistics(&self) -> RetainedLayerStatistics {
        RetainedLayerStatistics::default()
    }

    /// Fills a [`Quad`] with the provided [`Background`].
    fn fill_quad(&mut self, quad: Quad, background: impl Into<Background>);

    /// Creates an [`image::Allocation`] for the given [`image::Handle`] and calls the given callback with it.
    fn allocate_image(
        &mut self,
        handle: &image::Handle,
        callback: impl FnOnce(Result<image::Allocation, image::Error>) + Send + 'static,
    );

    /// Provides hints to the [`Renderer`] about the rendering target.
    ///
    /// This may be used internally by the [`Renderer`] to perform optimizations
    /// and/or improve rendering quality.
    ///
    /// For instance, providing a `scale_factor` may be used by some renderers to
    /// perform metrics hinting internally in physical coordinates while keeping
    /// layout coordinates logical and, therefore, maintain linearity.
    fn hint(&mut self, scale_factor: f32);

    /// Returns the last scale factor provided as a [`hint`](Self::hint).
    fn scale_factor(&self) -> Option<f32>;

    /// Resets the [`Renderer`] to start drawing in the `new_bounds` from scratch.
    fn reset(&mut self, new_bounds: Rectangle);

    /// Polls any concurrent computations that may be pending in the [`Renderer`].
    ///
    /// By default, it does nothing.
    fn tick(&mut self) {}
}

/// A polygon with four sides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    /// The bounds of the [`Quad`].
    pub bounds: Rectangle,

    /// The [`Border`] of the [`Quad`]. The border is drawn on the inside of the [`Quad`].
    pub border: Border,

    /// The [`Shadow`] of the [`Quad`].
    pub shadow: Shadow,

    /// Whether the [`Quad`] should be snapped to the pixel grid.
    pub snap: bool,

    /// How much light this [`Quad`] emits, relative to SDR diffuse white.
    ///
    /// `1.0` -- the default -- is an ordinary surface, and is what every
    /// widget in the framework draws today. Larger values emit: on a
    /// half-float surface a `4.93` white is a 1,000-nit highlight, brighter
    /// than the page it sits on rather than merely lighter.
    ///
    /// Use [`aura_core::color::space::nits_to_scene`] to convert an absolute
    /// luminance into this scale.
    ///
    /// Applies to the fill and the border, which are the quad's own surfaces.
    /// It deliberately does *not* apply to the shadow: a shadow is where
    /// light is missing, and a glowing one is not a thing.
    ///
    /// What this [`Quad`] is made of.
    ///
    /// [`Material::OPAQUE`] by default, which is what every widget already
    /// is and which shades exactly as it did before materials existed.
    ///
    /// A material makes the quad respond to the scene light: its bevel
    /// catches a specular highlight whose shape comes from the material's
    /// roughness and whose strength comes from its index of refraction. The
    /// bevel is analytic, derived from the same rounded-rectangle signed
    /// distance field the quad is already drawn with, so a lit edge costs no
    /// geometry and no extra attachment.
    ///
    /// Ignored, like [`Quad::luminance`], when the renderer is not
    /// compositing in linear light: a specular highlight added to
    /// gamma-encoded components is not a highlight.
    pub material: crate::material::Material,

    /// Values above `1.0` are only preserved when two things hold, and both
    /// are currently opt-in:
    ///
    /// - The surface can carry them. Every `Unorm` format clamps at `1.0`
    ///   whatever its precision, so on a standard-range surface this
    ///   brightens up to white and no further.
    /// - The renderer composites in linear light. The `web-colors` feature
    ///   packs sRGB-encoded components and blends those directly, and
    ///   scaling a gamma-encoded component changes hue rather than
    ///   luminance -- so this is ignored under it rather than applied
    ///   wrongly. `web-colors` is a default feature of the `aura` crate
    ///   today, which means the default build ignores this field.
    pub luminance: f32,
}

impl Default for Quad {
    fn default() -> Self {
        Self {
            bounds: Rectangle::with_size(Size::ZERO),
            border: Border::default(),
            shadow: Shadow::default(),
            snap: CRISP,
            luminance: 1.0,
            material: crate::material::Material::OPAQUE,
        }
    }
}

/// The styling attributes of a [`Renderer`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The text color
    pub text_color: Color,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            text_color: Color::BLACK,
        }
    }
}

/// A headless renderer is a renderer that can render offscreen without
/// a window nor a compositor.
pub trait Headless {
    /// Creates a new [`Headless`] renderer;
    fn new(settings: Settings, backend: Option<&str>) -> impl Future<Output = Option<Self>>
    where
        Self: Sized;

    /// Returns the unique name of the renderer.
    ///
    /// This name may be used by testing libraries to uniquely identify
    /// snapshots.
    fn name(&self) -> String;

    /// Draws offscreen into a screenshot, returning a collection of
    /// bytes representing the rendered pixels in RGBA order.
    fn screenshot(
        &mut self,
        size: Size<u32>,
        scale_factor: f32,
        background_color: Color,
    ) -> Vec<u8>;
}

/// The settings of a [`Renderer`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    /// The default [`Font`] to use.
    pub default_font: Font,

    /// The default size of text.
    ///
    /// By default, it will be set to `16.0`.
    pub default_text_size: Pixels,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_font: Font::DEFAULT,
            default_text_size: Pixels(16.0),
        }
    }
}

//! Write your own renderer.
mod api;
#[cfg(debug_assertions)]
mod null;

pub use api::{Renderer, RetainedLayerStatistics};

use crate::{Border, Color, Font, Pixels, Rectangle, Shadow, Size};

/// Whether anti-aliasing should be avoided by snapping primitive coordinates to the
/// pixel grid.
pub const CRISP: bool = cfg!(feature = "crisp");
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
    /// Use [`taim_core::color::space::nits_to_scene`] to convert an absolute
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
    ///   wrongly. `web-colors` is a default feature of the `taim` crate
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

impl Quad {
    /// Creates a default quad at `bounds`.
    pub fn new(bounds: Rectangle) -> Self {
        Self {
            bounds,
            ..Self::default()
        }
    }

    /// Sets the quad border.
    pub fn with_border(mut self, border: Border) -> Self {
        self.border = border;
        self
    }

    /// Sets the quad shadow.
    pub fn with_shadow(mut self, shadow: Shadow) -> Self {
        self.shadow = shadow;
        self
    }

    /// Sets whether coordinates are snapped to physical pixels.
    pub fn with_snapping(mut self, snap: bool) -> Self {
        self.snap = snap;
        self
    }

    /// Sets scene-relative surface luminance, sanitizing invalid values.
    pub fn with_luminance(mut self, luminance: f32) -> Self {
        self.luminance = if luminance.is_finite() {
            luminance.max(0.0)
        } else {
            1.0
        };
        self
    }

    /// Returns whether the quad owns a positive finite paint area.
    pub fn is_visible(&self) -> bool {
        self.bounds.width.is_finite()
            && self.bounds.height.is_finite()
            && self.bounds.width > 0.0
            && self.bounds.height > 0.0
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

    /// Rasterization path requested by the application.
    pub rasterizer: Rasterizer,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_font: Font::DEFAULT,
            default_text_size: Pixels(16.0),
            rasterizer: Rasterizer::Automatic,
        }
    }
}

impl Settings {
    /// Sets the default text font.
    pub fn with_default_font(mut self, font: Font) -> Self {
        self.default_font = font;
        self
    }

    /// Sets a finite, non-negative default text size.
    pub fn with_default_text_size(mut self, size: impl Into<Pixels>) -> Self {
        let size = size.into().0;
        self.default_text_size = Pixels(if size.is_finite() { size.max(0.0) } else { 16.0 });
        self
    }

    /// Selects the rasterization strategy.
    pub fn with_rasterizer(mut self, rasterizer: Rasterizer) -> Self {
        self.rasterizer = rasterizer;
        self
    }
}

/// Rasterization path selected for a renderer.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Rasterizer {
    /// Prefer the tiled compute path when the backend supports it and fall
    /// back to the established quad/triangle path otherwise.
    #[default]
    Automatic,
    /// Request the tiled compute path, falling back when unavailable.
    Compute,
    /// Always use the established quad/triangle path.
    Legacy,
}

impl Rasterizer {
    /// Returns whether compute rasterization may be selected.
    pub const fn allows_compute(self) -> bool {
        !matches!(self, Self::Legacy)
    }
}

#[cfg(test)]
mod settings_tests {
    use super::{Rasterizer, Settings};

    #[test]
    fn automatic_rasterization_is_the_fallback_safe_default() {
        assert_eq!(Settings::default().rasterizer, Rasterizer::Automatic);
    }
}

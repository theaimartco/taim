//! A compositor is responsible for initializing a renderer and managing window
//! surfaces.
use crate::core::Color;
use crate::core::backend;
use crate::core::font;
use crate::core::renderer;
use crate::{Shell, Viewport};

use std::borrow::Cow;

mod frame;
#[cfg(debug_assertions)]
mod null;
mod types;

pub use frame::FrameContext;
pub use types::{Display, Information, SurfaceError, Window};

/// A graphics compositor that can draw to windows.
pub trait Compositor: Sized {
    /// The taim renderer of the backend.
    type Renderer;

    /// The surface of the backend.
    type Surface;

    /// Creates a new [`Compositor`] with the given [`backend::Settings`].
    fn new(
        settings: backend::Settings,
        display: impl Display + Clone,
        compatible_window: impl Window + Clone,
        shell: Shell,
    ) -> impl Future<Output = Result<Self, backend::Error>>;

    /// Creates a [`Self::Renderer`] for the [`Compositor`].
    fn create_renderer(&self, settings: renderer::Settings) -> Self::Renderer;

    /// Crates a new [`Surface`] for the given window.
    ///
    /// [`Surface`]: Self::Surface
    fn create_surface(
        &mut self,
        window: impl Window + Clone,
        width: u32,
        height: u32,
    ) -> Self::Surface;

    /// Configures a new [`Surface`] with the given dimensions.
    ///
    /// [`Surface`]: Self::Surface
    fn configure_surface(&mut self, surface: &mut Self::Surface, width: u32, height: u32);

    /// Whether all device-bound resources require recreation. Backends without
    /// device-loss notifications keep the default false.
    fn is_device_lost(&self) -> bool {
        false
    }

    /// Returns [`Information`] used by this [`Compositor`].
    fn information(&self) -> Information;

    /// Loads a font from its bytes.
    fn load_font(&mut self, font: Cow<'static, [u8]>) -> Result<(), font::Error> {
        crate::text::font_system()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .load_font(font);

        // `font::Error` has no variants yet; data without a usable face is
        // logged and skipped by the font system.
        Ok(())
    }

    /// Lists all the available font families.
    fn list_fonts(&mut self) -> Result<Vec<font::Family>, font::Error> {
        let font_system = crate::text::font_system()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut families = font_system.families().collect::<Vec<_>>();

        // Contiguous sort + dedup is substantially cheaper than allocating one
        // B-tree node per face and still preserves deterministic API ordering.
        families.sort_unstable();
        families.dedup();

        Ok(families.into_iter().map(font::Family::name).collect())
    }

    /// Presents the [`Renderer`] primitives to the next frame of the given [`Surface`].
    ///
    /// [`Renderer`]: Self::Renderer
    /// [`Surface`]: Self::Surface
    fn present(
        &mut self,
        renderer: &mut Self::Renderer,
        surface: &mut Self::Surface,
        viewport: &Viewport,
        background_color: Color,
        on_pre_present: impl FnOnce(),
    ) -> Result<(), SurfaceError>;

    /// Presents the current [`Renderer`] primitives to the given [`Surface`],
    /// with damage-aware hints from the [`FrameContext`].
    ///
    /// The default implementation ignores the damage and falls back to a
    /// full-surface present via [`present`](Self::present). Backends that
    /// support partial presentation should override this to forward damage
    /// rects to the platform compositor.
    ///
    /// [`Renderer`]: Self::Renderer
    /// [`Surface`]: Self::Surface
    fn present_with_damage(
        &mut self,
        renderer: &mut Self::Renderer,
        surface: &mut Self::Surface,
        viewport: &Viewport,
        background_color: Color,
        _frame_context: &FrameContext,
        on_pre_present: impl FnOnce(),
    ) -> Result<(), SurfaceError> {
        self.present(
            renderer,
            surface,
            viewport,
            background_color,
            on_pre_present,
        )
    }

    /// Screenshots the current [`Renderer`] primitives to an offscreen texture, and returns the bytes of
    /// the texture ordered as `RGBA` in the `sRGB` color space.
    ///
    /// [`Renderer`]: Self::Renderer
    fn screenshot(
        &mut self,
        renderer: &mut Self::Renderer,
        viewport: &Viewport,
        background_color: Color,
    ) -> Vec<u8>;
}

/// Defines the default compositor of a renderer.
pub trait Default {
    /// The compositor of the renderer.
    type Compositor: Compositor<Renderer = Self>;
}

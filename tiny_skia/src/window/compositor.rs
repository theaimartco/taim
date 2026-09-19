mod present;
mod screenshot;
mod state;

pub use state::{Compositor, Surface};

use crate::core::{backend, renderer};
use crate::graphics::compositor::{self, Information};
use crate::graphics::{Shell, Viewport};
use crate::{Renderer, core::Color};

impl crate::graphics::Compositor for Compositor {
    type Renderer = Renderer;
    type Surface = Surface;

    async fn new(
        settings: backend::Settings,
        display: impl compositor::Display,
        _compatible_window: impl compositor::Window,
        _shell: Shell,
    ) -> Result<Self, backend::Error> {
        if !settings.backend.is_software() && !settings.backend.matches("tiny-skia") {
            return Err(backend::Error::GraphicsAdapterNotFound {
                backend: "tiny-skia",
                reason: backend::Reason::DidNotMatch {
                    preferred_backend: settings.backend,
                },
            });
        }
        Ok(new(display))
    }

    fn create_renderer(&self, settings: renderer::Settings) -> Self::Renderer {
        Renderer::new(settings)
    }

    fn create_surface(
        &mut self,
        window: impl compositor::Window + Clone,
        width: u32,
        height: u32,
    ) -> Self::Surface {
        state::create_surface(self, window, width, height)
    }

    fn configure_surface(&mut self, surface: &mut Self::Surface, width: u32, height: u32) {
        state::configure(surface, width, height);
    }

    fn information(&self) -> Information {
        Information {
            adapter: String::from("CPU"),
            backend: String::from("tiny-skia"),
        }
    }

    fn present(
        &mut self,
        renderer: &mut Self::Renderer,
        surface: &mut Self::Surface,
        viewport: &Viewport,
        background_color: Color,
        on_pre_present: impl FnOnce(),
    ) -> Result<(), compositor::SurfaceError> {
        present::frame(
            renderer,
            surface,
            viewport,
            background_color,
            on_pre_present,
        )
    }

    fn screenshot(
        &mut self,
        renderer: &mut Self::Renderer,
        viewport: &Viewport,
        background_color: Color,
    ) -> Vec<u8> {
        screenshot::capture(renderer, viewport, background_color)
    }
}

pub fn new(display: impl compositor::Display) -> Compositor {
    state::create_compositor(display)
}

pub fn present(
    renderer: &mut Renderer,
    surface: &mut Surface,
    viewport: &Viewport,
    background: Color,
    on_pre_present: impl FnOnce(),
) -> Result<(), compositor::SurfaceError> {
    present::frame(renderer, surface, viewport, background, on_pre_present)
}

pub fn screenshot(
    renderer: &mut Renderer,
    viewport: &Viewport,
    background_color: Color,
) -> Vec<u8> {
    screenshot::capture(renderer, viewport, background_color)
}

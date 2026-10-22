//! What the shell knows about one window, kept in two parts: its
//! [`Geometry`] (size, scale, surface generation) and its [`Appearance`]
//! (title, theme, style).
use crate::conversion;
use crate::core::{Color, Size};
use crate::core::{keyboard, mouse, theme, window};
use crate::graphics::Viewport;
use crate::program::{App, AppRunner, WindowContext};

use winit::dpi::PhysicalPosition;
use winit::event::{Touch, WindowEvent};
use winit::window::Window;

use std::fmt;

/// Per-window state derived from the platform window and the [`App`].
pub struct State<A: App>
where
    A::Theme: theme::Base,
{
    geometry: Geometry,
    appearance: Appearance<A>,
    /// Last physical pointer location; `None` once the pointer leaves.
    pointer: Option<PhysicalPosition<f64>>,
    modifiers: keyboard::Modifiers,
}

/// Size and scale of the drawable surface.
#[derive(Debug)]
struct Geometry {
    viewport: Viewport,
    /// The application's own zoom, on top of the platform scale factor.
    app_scale: f32,
    /// Bumped whenever the surface must be reconfigured.
    surface_version: u64,
}

/// Title, theme and the colors derived from it.
struct Appearance<A: App> {
    title: String,
    /// The theme the application chose, if it chose one.
    chosen: Option<A::Theme>,
    /// The theme used when the application does not choose; follows the
    /// system color scheme.
    fallback: A::Theme,
    /// The mode last requested from the platform for this window.
    mode: theme::Mode,
    style: theme::Style,
}

impl<A: App> fmt::Debug for State<A>
where
    A::Theme: theme::Base,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("window::State")
            .field("title", &self.appearance.title)
            .field("geometry", &self.geometry)
            .field("pointer", &self.pointer)
            .field("style", &self.appearance.style)
            .finish()
    }
}

impl<A: App> State<A>
where
    A::Theme: theme::Base,
{
    /// Reads the initial state of `window` from the application.
    pub fn new(
        runner: &AppRunner<A>,
        id: window::Id,
        window: &Window,
        system_theme: theme::Mode,
    ) -> Self {
        let context = WindowContext::new(id, window.scale_factor() as f32);
        let app_scale = positive_or_one(runner.scale_factor(&context));

        let chosen = runner.theme(&context);
        let fallback = <A::Theme as theme::Base>::default(system_theme);
        let style = runner.style(chosen.as_ref().unwrap_or(&fallback));
        let surface = window.inner_size();

        Self {
            geometry: Geometry {
                viewport: Viewport::with_physical_size(
                    Size::new(surface.width, surface.height),
                    effective_scale(window.scale_factor(), app_scale),
                ),
                app_scale,
                surface_version: 0,
            },
            appearance: Appearance {
                title: runner.title(&context),
                mode: chosen.as_ref().map(theme::Base::mode).unwrap_or_default(),
                chosen,
                fallback,
                style,
            },
            pointer: None,
            modifiers: keyboard::Modifiers::empty(),
        }
    }

    pub fn viewport(&self) -> &Viewport {
        &self.geometry.viewport
    }

    pub fn surface_version(&self) -> u64 {
        self.geometry.surface_version
    }

    pub fn physical_size(&self) -> Size<u32> {
        self.geometry.viewport.physical_size()
    }

    pub fn logical_size(&self) -> Size<f32> {
        self.geometry.viewport.logical_size()
    }

    pub fn scale_factor(&self) -> f32 {
        self.geometry.viewport.scale_factor()
    }

    pub fn cursor(&self) -> mouse::Cursor {
        match self.pointer {
            Some(at) => {
                mouse::Cursor::Available(conversion::cursor_position(at, self.scale_factor()))
            }
            None => mouse::Cursor::Unavailable,
        }
    }

    pub fn modifiers(&self) -> keyboard::Modifiers {
        self.modifiers
    }

    pub fn theme(&self) -> &A::Theme {
        self.appearance.theme()
    }

    pub fn theme_mode(&self) -> theme::Mode {
        self.appearance.mode
    }

    pub fn background_color(&self) -> Color {
        self.appearance.style.background_color
    }

    pub fn text_color(&self) -> Color {
        self.appearance.style.text_color
    }

    /// Folds a platform event into the state.
    pub fn update(&mut self, runner: &AppRunner<A>, window: &Window, event: &WindowEvent) {
        match event {
            WindowEvent::Resized(surface) => {
                let size = Size::new(surface.width, surface.height);
                self.geometry.resize(size, window.scale_factor());
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let size = self.physical_size();
                self.geometry.resize(size, *scale_factor);
            }
            WindowEvent::CursorMoved { position, .. }
            | WindowEvent::Touch(Touch {
                location: position, ..
            }) => self.pointer = Some(*position),
            WindowEvent::CursorLeft { .. } => self.pointer = None,
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = conversion::modifiers(modifiers.state());
            }
            WindowEvent::ThemeChanged(system) => {
                self.appearance
                    .follow_system(runner, conversion::theme_mode(*system));
            }
            _ => {}
        }
    }

    /// Re-reads title, scale and theme from the application after an update
    /// and pushes any change to the platform window.
    pub fn synchronize(&mut self, runner: &AppRunner<A>, id: window::Id, window: &Window) {
        let context = WindowContext::new(id, window.scale_factor() as f32);

        let title = runner.title(&context);
        if title != self.appearance.title {
            window.set_title(&title);
            self.appearance.title = title;
        }

        let app_scale = positive_or_one(runner.scale_factor(&context));
        if app_scale != self.geometry.app_scale {
            self.geometry.app_scale = app_scale;
            let size = self.physical_size();
            self.geometry.resize(size, window.scale_factor());
        }

        self.appearance
            .choose(runner, runner.theme(&context), window);
    }
}

impl Geometry {
    /// Applies a new surface size or platform scale; bumps the surface
    /// version only if the viewport actually changed.
    fn resize(&mut self, physical: Size<u32>, platform_scale: f64) {
        let scale = effective_scale(platform_scale, self.app_scale);

        if self.viewport.update(physical, scale) {
            self.surface_version = self.surface_version.wrapping_add(1);
        }
    }
}

impl<A: App> Appearance<A>
where
    A::Theme: theme::Base,
{
    fn theme(&self) -> &A::Theme {
        self.chosen.as_ref().unwrap_or(&self.fallback)
    }

    fn restyle(&mut self, runner: &AppRunner<A>) {
        self.style = runner.style(self.theme());
    }

    /// The system color scheme changed.
    fn follow_system(&mut self, runner: &AppRunner<A>, system: theme::Mode) {
        self.fallback = <A::Theme as theme::Base>::default(system);

        if self.chosen.is_none() {
            self.restyle(runner);
        }
    }

    /// The application (re)chose its theme.
    fn choose(&mut self, runner: &AppRunner<A>, chosen: Option<A::Theme>, window: &Window) {
        self.chosen = chosen;
        self.restyle(runner);

        let wanted = self
            .chosen
            .as_ref()
            .map(theme::Base::mode)
            .unwrap_or_default();

        if wanted == self.mode {
            return;
        }

        // Linux learns every system change from the desktop portal, so
        // "follow the system" can be answered locally with the fallback's
        // mode. Elsewhere the platform is asked to follow the system, and the
        // fallback keeps the previous mode until the platform reports
        // otherwise.
        #[cfg(target_os = "linux")]
        let wanted = match wanted {
            theme::Mode::None => theme::Base::mode(&self.fallback),
            mode => mode,
        };

        #[cfg(not(target_os = "linux"))]
        if wanted == theme::Mode::None {
            self.follow_system(runner, self.mode);
        }

        window.set_theme(conversion::window_theme(wanted));
        self.mode = wanted;
    }
}

#[inline]
fn positive_or_one(scale: f32) -> f32 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

/// Platform scale times application zoom, never zero or non-finite.
#[inline]
fn effective_scale(platform_scale: f64, app_scale: f32) -> f32 {
    positive_or_one(platform_scale as f32 * app_scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broken_scales_collapse_to_one() {
        assert_eq!(effective_scale(2.0, 1.5), 3.0);
        assert_eq!(effective_scale(f64::NAN, 1.0), 1.0);
        assert_eq!(effective_scale(1.0, 0.0), 1.0);
        assert_eq!(effective_scale(f64::INFINITY, 1.0), 1.0);
    }

    #[test]
    fn surface_version_moves_only_on_real_changes() {
        let mut geometry = Geometry {
            viewport: Viewport::with_physical_size(Size::new(100, 100), 1.0),
            app_scale: 1.0,
            surface_version: 0,
        };

        geometry.resize(Size::new(100, 100), 1.0);
        assert_eq!(geometry.surface_version, 0);

        geometry.resize(Size::new(200, 100), 1.0);
        assert_eq!(geometry.surface_version, 1);

        geometry.resize(Size::new(200, 100), 2.0);
        assert_eq!(geometry.surface_version, 2);
    }
}

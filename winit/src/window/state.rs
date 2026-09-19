use crate::conversion;
use crate::core::{Color, Size};
use crate::core::{keyboard, mouse, theme, window};
use crate::graphics::Viewport;
use crate::program::App;

use winit::event::{Touch, WindowEvent};
use winit::window::Window;

use std::fmt::{Debug, Formatter};

/// The state of the window of an [`App`].
pub struct State<A: App>
where
    A::Theme: theme::Base,
{
    title: String,
    scale_factor: f32,
    viewport: Viewport,
    surface_version: u64,
    cursor_position: Option<winit::dpi::PhysicalPosition<f64>>,
    modifiers: keyboard::Modifiers,
    theme: Option<A::Theme>,
    theme_mode: theme::Mode,
    default_theme: A::Theme,
    style: theme::Style,
}

impl<A: App> Debug for State<A>
where
    A::Theme: theme::Base,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("window::State")
            .field("title", &self.title)
            .field("scale_factor", &self.scale_factor)
            .field("viewport", &self.viewport)
            .field("cursor_position", &self.cursor_position)
            .field("style", &self.style)
            .finish()
    }
}

impl<A: App> State<A>
where
    A::Theme: theme::Base,
{
    /// Creates a new [`State`] for the provided [`App`]'s `window`.
    pub fn new(
        runner: &crate::program::AppRunner<A>,
        window_id: window::Id,
        window: &Window,
        system_theme: theme::Mode,
    ) -> Self {
        let win_ctx = crate::program::WindowContext::new(window_id, window.scale_factor() as f32);
        let title = runner.title(&win_ctx);
        let scale_factor = valid_application_scale(runner.scale_factor(&win_ctx));
        let theme = runner.theme(&win_ctx);
        let theme_mode = theme.as_ref().map(theme::Base::mode).unwrap_or_default();
        let default_theme = <A::Theme as theme::Base>::default(system_theme);
        let style = runner.style(theme.as_ref().unwrap_or(&default_theme));

        let viewport = {
            let physical_size = window.inner_size();

            Viewport::with_physical_size(
                Size::new(physical_size.width, physical_size.height),
                combined_scale(window.scale_factor(), scale_factor),
            )
        };

        Self {
            title,
            scale_factor,
            viewport,
            surface_version: 0,
            cursor_position: None,
            modifiers: keyboard::Modifiers::empty(),
            theme,
            theme_mode,
            default_theme,
            style,
        }
    }

    pub fn viewport(&self) -> &Viewport {
        &self.viewport
    }

    pub fn surface_version(&self) -> u64 {
        self.surface_version
    }

    pub fn physical_size(&self) -> Size<u32> {
        self.viewport.physical_size()
    }

    pub fn logical_size(&self) -> Size<f32> {
        self.viewport.logical_size()
    }

    pub fn scale_factor(&self) -> f32 {
        self.viewport.scale_factor()
    }

    pub fn cursor(&self) -> mouse::Cursor {
        self.cursor_position
            .map(|cursor_position| {
                conversion::cursor_position(cursor_position, self.viewport.scale_factor())
            })
            .map(mouse::Cursor::Available)
            .unwrap_or(mouse::Cursor::Unavailable)
    }

    pub fn modifiers(&self) -> keyboard::Modifiers {
        self.modifiers
    }

    pub fn theme(&self) -> &A::Theme {
        self.theme.as_ref().unwrap_or(&self.default_theme)
    }

    pub fn theme_mode(&self) -> theme::Mode {
        self.theme_mode
    }

    pub fn background_color(&self) -> Color {
        self.style.background_color
    }

    pub fn text_color(&self) -> Color {
        self.style.text_color
    }

    pub fn update(
        &mut self,
        runner: &crate::program::AppRunner<A>,
        window: &Window,
        event: &WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(new_size) => {
                let size = Size::new(new_size.width, new_size.height);
                let scale = combined_scale(window.scale_factor(), self.scale_factor);

                if self.viewport.update(size, scale) {
                    self.surface_version = self.surface_version.wrapping_add(1);
                }
            }
            WindowEvent::ScaleFactorChanged {
                scale_factor: new_scale_factor,
                ..
            } => {
                let size = self.viewport.physical_size();
                let scale = combined_scale(*new_scale_factor, self.scale_factor);

                if self.viewport.update(size, scale) {
                    self.surface_version = self.surface_version.wrapping_add(1);
                }
            }
            WindowEvent::CursorMoved { position, .. }
            | WindowEvent::Touch(Touch {
                location: position, ..
            }) => {
                self.cursor_position = Some(*position);
            }
            WindowEvent::CursorLeft { .. } => {
                self.cursor_position = None;
            }
            WindowEvent::ModifiersChanged(new_modifiers) => {
                self.modifiers = conversion::modifiers(new_modifiers.state());
            }
            WindowEvent::ThemeChanged(theme) => {
                self.default_theme =
                    <A::Theme as theme::Base>::default(conversion::theme_mode(*theme));

                if self.theme.is_none() {
                    self.style = runner.style(&self.default_theme);
                }
            }
            _ => {}
        }
    }

    pub fn synchronize(
        &mut self,
        runner: &crate::program::AppRunner<A>,
        window_id: window::Id,
        window: &Window,
    ) {
        let win_ctx = crate::program::WindowContext::new(window_id, window.scale_factor() as f32);

        // Update window title
        let new_title = runner.title(&win_ctx);

        if self.title != new_title {
            window.set_title(&new_title);
            self.title = new_title;
        }

        // Update scale factor
        let new_scale_factor = valid_application_scale(runner.scale_factor(&win_ctx));

        if self.scale_factor != new_scale_factor {
            if self.viewport.update(
                self.viewport.physical_size(),
                combined_scale(window.scale_factor(), new_scale_factor),
            ) {
                self.surface_version = self.surface_version.wrapping_add(1);
            }

            self.scale_factor = new_scale_factor;
        }

        // Update theme and appearance
        self.theme = runner.theme(&win_ctx);
        self.style = runner.style(self.theme());

        let new_mode = self
            .theme
            .as_ref()
            .map(theme::Base::mode)
            .unwrap_or_default();

        if self.theme_mode != new_mode {
            #[cfg(not(target_os = "linux"))]
            {
                window.set_theme(conversion::window_theme(new_mode));

                // Assume the old mode matches the system one
                // We will be notified otherwise
                if new_mode == theme::Mode::None {
                    self.default_theme = <A::Theme as theme::Base>::default(self.theme_mode);

                    if self.theme.is_none() {
                        self.style = runner.style(&self.default_theme);
                    }
                }
            }

            #[cfg(target_os = "linux")]
            {
                // mundy always notifies system theme changes, so we
                // just restore the default theme mode.
                let new_mode = if new_mode == theme::Mode::None {
                    theme::Base::mode(&self.default_theme)
                } else {
                    new_mode
                };

                window.set_theme(conversion::window_theme(new_mode));
            }

            self.theme_mode = new_mode;
        }
    }
}

#[inline]
fn valid_application_scale(scale: f32) -> f32 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

#[inline]
fn combined_scale(platform_scale: f64, application_scale: f32) -> f32 {
    let scale = platform_scale as f32 * application_scale;

    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

//! The event loop: opens the window, feeds it platform events and draws it
//! when its frame clock says so.
use crate::conversion;
use crate::core::time::Instant;
use crate::core::{self, backend, clipboard, theme, window};
use crate::graphics::{self, Compositor as _};
use crate::surface::Surface;
use crate::wake::{ChangeSignal, RendererSignal, Wake};
use crate::{Clipboard, Compositor, Error};

use taim_view::{AnyView, Root, Scope};

use std::sync::Arc;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::WindowId;

/// An application: one window showing the views `build` returns.
pub struct App {
    /// The window title.
    pub title: String,
    /// Fonts, default text, renderer backend.
    pub settings: core::Settings,
    /// Size, position and decorations of the window.
    pub window: window::Settings,
    /// Builds the window's views. Runs once, when the window opens.
    pub build: Box<dyn FnOnce(&mut Scope) -> AnyView>,
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("title", &self.title)
            .field("window", &self.window)
            .finish_non_exhaustive()
    }
}

/// Runs `app` until its window closes.
pub fn run(app: App) -> Result<(), Error> {
    let event_loop = EventLoop::<Wake>::with_user_event()
        .build()
        .map_err(Error::EventLoop)?;

    let mut shell = Shell {
        settings: app.settings.clone(),
        pending: Some(app),
        proxy: event_loop.create_proxy(),
        display: event_loop.owned_display_handle(),
        compositor: None,
        windows: Vec::new(),
        clipboard: Clipboard::new(),
        system: theme::Mode::None,
        failure: None,
    };

    event_loop.run_app(&mut shell).map_err(Error::EventLoop)?;
    shell.failure.map_or(Ok(()), Err)
}

struct Shell {
    settings: core::Settings,
    /// The application until its window opens.
    pending: Option<App>,
    proxy: EventLoopProxy<Wake>,
    display: winit::event_loop::OwnedDisplayHandle,
    compositor: Option<Compositor>,
    windows: Vec<Surface>,
    clipboard: Clipboard,
    /// The desktop's light or dark preference.
    system: theme::Mode,
    failure: Option<Error>,
}

impl Shell {
    fn open(&mut self, event_loop: &ActiveEventLoop, app: App) -> Result<(), Error> {
        let App {
            title,
            settings,
            window: window_settings,
            build,
        } = app;

        let root = Root::new(build);
        let exit_on_close = window_settings.exit_on_close_request;
        let visible = window_settings.visible;

        let attributes = conversion::window_attributes(
            window_settings,
            &title,
            root.scale(),
            event_loop.primary_monitor(),
            settings.id.clone(),
        )
        .with_visible(false);

        let raw = Arc::new(
            event_loop
                .create_window(attributes)
                .map_err(Error::WindowCreationFailed)?,
        );

        if self.compositor.is_none() {
            self.compositor = Some(self.create_compositor(&raw)?);
        }
        let compositor = self.compositor.as_mut().expect("created above");

        let id = window::Id::unique();
        let mut surface = Surface::new(
            id,
            raw,
            root,
            compositor,
            core::renderer::Settings::from(&settings),
            exit_on_close,
            ChangeSignal::new(id, self.proxy.clone()),
            self.system,
        );

        if visible {
            surface.raw.set_visible(true);
        }
        surface.request_frame();
        self.windows.push(surface);

        Ok(())
    }

    fn create_compositor(&self, window: &Arc<winit::window::Window>) -> Result<Compositor, Error> {
        let mut compositor = futures::executor::block_on(Compositor::new(
            backend::Settings::from(&self.settings),
            self.display.clone(),
            window.clone(),
            graphics::Shell::new(RendererSignal(self.proxy.clone())),
        ))?;

        for font in self.settings.fonts.iter().cloned() {
            if let Err(error) = compositor.load_font(font) {
                log::warn!("a font failed to load: {error:?}");
            }
        }

        Ok(compositor)
    }

    fn window_by_id(&mut self, id: window::Id) -> Option<&mut Surface> {
        self.windows.iter_mut().find(|surface| surface.id == id)
    }

    /// Hands a views' clipboard and focus wishes to the platform.
    fn serve(&mut self, window: window::Id, response: taim_view::Response) {
        if let Some(text) = response.copy {
            self.clipboard
                .write(clipboard::Content::Text(text), |result| {
                    if let Err(error) = result {
                        log::warn!("copying failed: {error:?}");
                    }
                });
        }

        if response.paste {
            let proxy = self.proxy.clone();
            self.clipboard.read(clipboard::Kind::Text, move |result| {
                let event = clipboard::Event::Read(result.map(Arc::new));
                let _ = proxy.send_event(Wake::Clipboard(window, event));
            });
        }
    }

    fn set_system_theme(&mut self, mode: theme::Mode) {
        if mode == self.system {
            return;
        }

        self.system = mode;
        for surface in &mut self.windows {
            surface.restyle(mode);
        }
    }

    /// Learns the desktop's color scheme and keeps following it.
    fn follow_system_theme(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(theme) = event_loop.system_theme() {
            self.system = conversion::theme_mode(theme);
        }

        #[cfg(all(feature = "linux-theme-detection", target_os = "linux"))]
        {
            use futures::StreamExt as _;

            let to_mode = |scheme| match scheme {
                mundy::ColorScheme::NoPreference => theme::Mode::None,
                mundy::ColorScheme::Light => theme::Mode::Light,
                mundy::ColorScheme::Dark => theme::Mode::Dark,
            };

            if let Some(preferences) = mundy::Preferences::once_blocking(
                mundy::Interest::ColorScheme,
                core::time::Duration::from_millis(200),
            ) {
                self.system = to_mode(preferences.color_scheme);
            }

            let proxy = self.proxy.clone();
            let _ = std::thread::Builder::new()
                .name("taim-theme".into())
                .spawn(move || {
                    futures::executor::block_on(
                        mundy::Preferences::stream(mundy::Interest::ColorScheme).for_each(
                            |preferences| {
                                let mode = to_mode(preferences.color_scheme);
                                let _ = proxy.send_event(Wake::SystemTheme(mode));
                                std::future::ready(())
                            },
                        ),
                    );
                });
        }

        log::info!("system theme: {:?}", self.system);
    }
}

impl winit::application::ApplicationHandler<Wake> for Shell {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Some(app) = self.pending.take() else {
            return;
        };

        self.follow_system_theme(event_loop);

        if let Err(error) = self.open(event_loop, app) {
            self.failure = Some(error);
            event_loop.exit();
        }
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if let StartCause::ResumeTimeReached { .. } = cause {
            let now = Instant::now();
            for surface in &mut self.windows {
                surface.tick(now);
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let system = self.system;

        if let WindowEvent::ThemeChanged(theme) = &event {
            self.set_system_theme(conversion::theme_mode(*theme));
        }

        let Some(index) = self.windows.iter().position(|surface| surface.raw.id() == id) else {
            return;
        };
        let surface = &mut self.windows[index];

        match event {
            WindowEvent::RedrawRequested => {
                if let Some(compositor) = self.compositor.as_mut() {
                    surface.draw(compositor);
                }
            }
            WindowEvent::CloseRequested if surface.exit_on_close => {
                self.windows.retain(|surface| surface.raw.id() != id);
                if self.windows.is_empty() {
                    event_loop.exit();
                }
            }
            event => {
                let Some(event) = surface.observe(event, system) else {
                    return;
                };
                let response = surface.handle(&event);
                surface.absorb_changes(system);

                let window = surface.id;
                self.serve(window, response);
            }
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, wake: Wake) {
        let system = self.system;

        match wake {
            Wake::Changed(id) => {
                if let Some(surface) = self.window_by_id(id) {
                    surface.absorb_changes(system);
                }
            }
            Wake::Clipboard(id, event) => {
                let Some(surface) = self.window_by_id(id) else {
                    return;
                };
                let response = surface.handle(&core::Event::Clipboard(event));
                surface.absorb_changes(system);
                self.serve(id, response);
            }
            Wake::SystemTheme(mode) => self.set_system_theme(mode),
            Wake::Refresh => {
                for surface in &mut self.windows {
                    surface.request_frame();
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        for surface in &mut self.windows {
            surface.tick(now);
        }

        let flow = self
            .windows
            .iter()
            .filter_map(Surface::deadline)
            .min()
            .map_or(ControlFlow::Wait, ControlFlow::WaitUntil);
        event_loop.set_control_flow(flow);
    }
}

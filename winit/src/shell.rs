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

    run_on(event_loop, app, Platform::default())
}

/// Runs `app` as an Android activity, until the activity finishes.
///
/// Call it from the library's `android_main`, which android-activity calls
/// on its own thread once the activity starts:
///
/// ```ignore
/// #[unsafe(no_mangle)]
/// fn android_main(android: taim_winit::AndroidApp) {
///     let _ = taim_winit::run_android(app, android);
/// }
/// ```
///
/// Log records and panics go to logcat under the `taim` tag.
#[cfg(target_os = "android")]
pub fn run_android(app: App, android: crate::AndroidApp) -> Result<(), Error> {
    use winit::platform::android::EventLoopBuilderExtAndroid as _;

    crate::android::init_logging();

    let event_loop = EventLoop::<Wake>::with_user_event()
        .with_android_app(android.clone())
        .build()
        .map_err(Error::EventLoop)?;

    run_on(
        event_loop,
        app,
        Platform {
            android: Some(android),
        },
    )
}

/// What the shell needs from the platform beyond `winit`.
#[derive(Default)]
struct Platform {
    #[cfg(target_os = "android")]
    android: Option<crate::AndroidApp>,
}

fn run_on(event_loop: EventLoop<Wake>, app: App, platform: Platform) -> Result<(), Error> {
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
        platform,
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
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    platform: Platform,
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
        #[cfg(target_os = "android")]
        if let Some(android) = &self.platform.android {
            surface.attach_android(android);
        }
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

    /// Keeps views out from under the system bars, display cutout and
    /// on-screen keyboard. Android reports the visible content area rather
    /// than an event, so it is read whenever the event loop goes idle;
    /// reading it is a lock and a copy.
    #[cfg(target_os = "android")]
    fn follow_insets(&mut self) {
        let Some(android) = &self.platform.android else {
            return;
        };
        let content = android.content_rect();

        for surface in &mut self.windows {
            let size = surface.raw.inner_size();
            // Before the activity reports a content area it is all zeroes.
            if content.right <= content.left || content.bottom <= content.top {
                surface.set_insets(0.0, 0.0, 0.0, 0.0);
                continue;
            }

            let edge = |value: i32| value.max(0) as f32;
            surface.set_insets(
                edge(content.top),
                edge(size.width as i32 - content.right),
                edge(size.height as i32 - content.bottom),
                edge(content.left),
            );
        }
    }

    /// The platform's light or dark preference, where `winit` does not
    /// report one.
    #[cfg(target_os = "android")]
    fn android_theme(&self) -> Option<theme::Mode> {
        use winit::platform::android::activity::ndk::configuration::UiModeNight;

        let android = self.platform.android.as_ref()?;
        match android.config().ui_mode_night() {
            UiModeNight::Yes => Some(theme::Mode::Dark),
            UiModeNight::No => Some(theme::Mode::Light),
            _ => None,
        }
    }

    /// Learns the desktop's color scheme and keeps following it.
    fn follow_system_theme(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(theme) = event_loop.system_theme() {
            self.system = conversion::theme_mode(theme);
        }

        #[cfg(target_os = "android")]
        if let Some(mode) = self.android_theme() {
            self.system = mode;
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
            // Back from the background: the platform handed the native
            // window back, so every window gets a new GPU surface.
            #[cfg(target_os = "android")]
            if let Some(mode) = self.android_theme() {
                self.set_system_theme(mode);
            }

            if let Some(compositor) = self.compositor.as_mut() {
                for surface in &mut self.windows {
                    surface.resume(compositor);
                }
            }
            return;
        };

        self.follow_system_theme(event_loop);

        if let Err(error) = self.open(event_loop, app) {
            self.failure = Some(error);
            event_loop.exit();
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        // On Android the native window is destroyed after this returns; a
        // surface drawing to it would crash the process.
        for surface in &mut self.windows {
            surface.suspend();
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

        let Some(index) = self
            .windows
            .iter()
            .position(|surface| surface.raw.id() == id)
        else {
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
                let window = surface.id;

                for event in surface.observe(event, system).into_iter().flatten() {
                    let Some(surface) = self.window_by_id(window) else {
                        return;
                    };
                    let response = surface.handle(&event);
                    surface.absorb_changes(system);

                    // A back press nothing in the window used leaves the
                    // application, like the root route of a navigator.
                    #[cfg(target_os = "android")]
                    if !response.captured && taim_view::is_back(&event) {
                        event_loop.exit();
                    }

                    self.serve(window, response);
                }
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
            #[cfg(target_os = "android")]
            Wake::Describe(id) => {
                if let Some(surface) = self.window_by_id(id) {
                    surface.describe_again();
                }
            }
            #[cfg(target_os = "android")]
            Wake::Accessibility(id, request) => {
                let Some(surface) = self.window_by_id(id) else {
                    return;
                };
                let responses = surface.accessibility_action(&request);
                surface.absorb_changes(system);
                for response in responses {
                    self.serve(id, response);
                }
            }
            Wake::Refresh => {
                for surface in &mut self.windows {
                    surface.request_frame();
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(target_os = "android")]
        {
            self.follow_insets();

            // The input method edits its buffer on the UI thread and wakes
            // the event loop; its changes are read here.
            let system = self.system;
            for index in 0..self.windows.len() {
                let surface = &mut self.windows[index];
                let window = surface.id;
                let responses = surface.follow_typing();
                if responses.is_empty() {
                    continue;
                }
                surface.absorb_changes(system);
                for response in responses {
                    self.serve(window, response);
                }
            }

            // The manifest keeps the activity alive across a dark-mode
            // switch (`uiMode` in `configChanges`), so the switch is read
            // here rather than at the next start.
            if let Some(mode) = self.android_theme() {
                self.set_system_theme(mode);
            }
        }

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

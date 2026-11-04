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
#[cfg(not(target_os = "android"))]
pub fn run(app: App) -> Result<(), Error> {
    let event_loop = EventLoop::<Wake>::with_user_event()
        .build()
        .map_err(Error::EventLoop)?;
    run_event_loop(app, event_loop)
}

/// Reports a missing Android activity instead of panicking in winit.
#[cfg(target_os = "android")]
pub fn run(_app: App) -> Result<(), Error> {
    Err(Error::AndroidActivityRequired)
}

/// Runs an Android application using the activity supplied to `android_main`.
///
/// Call on the activity's native application thread. The activity handle must
/// come from this shell's re-exported winit version.
#[cfg(target_os = "android")]
pub fn run_android(
    app: App,
    activity: winit::platform::android::activity::AndroidApp,
) -> Result<(), Error> {
    use winit::platform::android::EventLoopBuilderExtAndroid;
    let event_loop = EventLoop::<Wake>::with_user_event()
        .with_android_app(activity.clone())
        .build()
        .map_err(Error::EventLoop)?;
    run_event_loop(app, event_loop, activity)
}

fn run_event_loop(
    app: App,
    event_loop: EventLoop<Wake>,
    #[cfg(target_os = "android")] activity: winit::platform::android::activity::AndroidApp,
) -> Result<(), Error> {
    let mut shell = Shell {
        #[cfg(target_os = "android")]
        android: crate::android::Bridge::new(activity, event_loop.create_proxy()),
        settings: app.settings.clone(),
        pending: Some(app),
        proxy: event_loop.create_proxy(),
        display: event_loop.owned_display_handle(),
        compositor: None,
        windows: Vec::new(),
        clipboard: Clipboard::new(),
        system: theme::Mode::None,
        failure: None,
        #[cfg(target_os = "android")]
        recovery: (Instant::now(), 0),
        #[cfg(all(feature = "linux-theme-detection", target_os = "linux"))]
        theme_watcher: None,
    };

    event_loop.run_app(&mut shell).map_err(Error::EventLoop)?;
    shell.failure.map_or(Ok(()), Err)
}

struct Shell {
    #[cfg(target_os = "android")]
    android: Arc<crate::android::Bridge>,
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
    #[cfg(target_os = "android")]
    recovery: (Instant, u8),
    #[cfg(all(feature = "linux-theme-detection", target_os = "linux"))]
    theme_watcher: Option<crate::system_theme::Watcher>,
}

impl Shell {
    #[cfg(target_os = "android")]
    fn native_updates(&mut self) {
        let pending = self.android.drain();
        if pending.native_blur || pending.native_focus.is_some() {
            for surface in &mut self.windows {
                surface.native_view_focus(pending.native_blur, pending.native_focus);
            }
        }
        if let Some((insets, dark, reduced)) = pending.insets {
            #[cfg(feature = "measurement")]
            log::info!(
                "TAIM_ANDROID_INSETS left={} top={} right={} bottom={} dark={dark} reduced_motion={reduced}",
                insets.left,
                insets.top,
                insets.right,
                insets.bottom
            );
            self.set_system_theme(if dark {
                theme::Mode::Dark
            } else {
                theme::Mode::Light
            });
            for surface in &mut self.windows {
                surface.set_insets(insets);
                surface.set_system_motion_reduced(reduced);
            }
        }
        for (owner, edit) in pending.edits {
            if owner == self.android.owner() {
                for surface in &mut self.windows {
                    let _ = surface.handle(&core::Event::InputMethod(
                        core::input_method::Event::Edit(edit.clone()),
                    ));
                }
            }
        }
        for (owner, event) in pending.operations {
            if owner == self.android.owner() {
                for surface in &mut self.windows {
                    let _ = surface.handle(&event);
                }
            }
        }
        let mut responses = Vec::new();
        for command in pending.semantics {
            use crate::android::SemanticCommand;
            for surface in &mut self.windows {
                let response = match &command {
                    SemanticCommand::Action(action) => surface.native_action(action),
                    SemanticCommand::Fill(target, text, autofill) => {
                        surface.autofill(target.clone(), text.clone(), *autofill)
                    }
                    SemanticCommand::Selection(target, start, end) => {
                        surface.semantic_selection(target.clone(), *start, *end)
                    }
                    SemanticCommand::Text(target, text) => {
                        surface.semantic_text_command(target.clone(), text)
                    }
                };
                responses.push((surface.id, response));
            }
        }
        for _ in 0..pending.back {
            let mut handled = false;
            for surface in &mut self.windows {
                let response = surface.back();
                handled |= response.captured;
                responses.push((surface.id, response));
            }
            if !handled {
                self.android.finish();
            }
        }
        if let Some(level) = pending.pressure
            && level >= 10
            && let Some(compositor) = self.compositor.as_mut()
            && !compositor.is_device_lost()
        {
            for surface in &mut self.windows {
                surface.release_memory(compositor);
            }
        }
        for (id, response) in responses {
            self.serve(id, response);
        }
        if pending.submit == Some(self.android.owner()) {
            for surface in &mut self.windows {
                let _ =
                    surface.handle(&core::Event::InputMethod(core::input_method::Event::Submit));
            }
        }
    }

    fn open(&mut self, event_loop: &ActiveEventLoop, app: App) -> Result<(), Error> {
        let App {
            title,
            settings,
            window: window_settings,
            build,
        } = app;

        #[cfg(not(target_os = "android"))]
        let root = Root::new(build);
        #[cfg(target_os = "android")]
        let root = Root::new_restored(
            self.android.restored_state().as_deref().unwrap_or("{}"),
            build,
        );
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
            #[cfg(target_os = "android")]
            self.android.clone(),
        );

        if visible {
            surface.raw.set_visible(true);
        }
        surface.request_frame();
        self.windows.push(surface);
        #[cfg(target_os = "android")]
        self.android.refresh_insets();

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

    #[cfg(target_os = "android")]
    fn recover_graphics(&mut self) -> Result<(), Error> {
        if self.recovery.0.elapsed() > std::time::Duration::from_secs(60) {
            self.recovery = (Instant::now(), 0);
        }
        if self.recovery.1 >= 3 {
            return Err(Error::PresentationFailed(
                graphics::compositor::SurfaceError::Lost,
            ));
        }
        self.recovery.1 += 1;
        let Some(raw) = self.windows.first().map(|surface| surface.raw.clone()) else {
            return Ok(());
        };
        let mut compositor = self.create_compositor(&raw)?;
        for surface in &mut self.windows {
            surface.recreate_graphics(&mut compositor);
        }
        self.compositor = Some(compositor);
        self.android.graphics_restored();
        log::info!("Android graphics device recreated; retained application state preserved");
        Ok(())
    }

    fn window_by_id(&mut self, id: window::Id) -> Option<&mut Surface> {
        self.windows.iter_mut().find(|surface| surface.id == id)
    }

    /// Hands a views' clipboard and focus wishes to the platform.
    fn serve(&mut self, window: window::Id, dispatch: crate::surface::Dispatch) {
        let response = dispatch.response;
        if let Some(text) = response.copy {
            self.clipboard
                .write(clipboard::Content::Text(text), |result| {
                    if let Err(error) = result {
                        log::warn!("copying failed: {error:?}");
                    }
                });
        }

        if let Some(token) = dispatch.paste {
            let proxy = self.proxy.clone();
            self.clipboard.read(clipboard::Kind::Text, move |result| {
                let event = clipboard::Event::Read(result.map(Arc::new));
                let _ = proxy.send_event(Wake::Clipboard(window, token, event));
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
            let proxy = self.proxy.clone();
            self.theme_watcher = crate::system_theme::Watcher::start(move |mode| {
                proxy.send_event(Wake::SystemTheme(mode)).is_ok()
            });
        }

        log::info!("system theme: {:?}", self.system);
    }
}

impl winit::application::ApplicationHandler<Wake> for Shell {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(target_os = "android")]
        if let Some(compositor) = self.compositor.as_mut() {
            for surface in &mut self.windows {
                surface.resume(compositor);
            }
        }
        let Some(app) = self.pending.take() else {
            return;
        };

        self.follow_system_theme(event_loop);

        if let Err(error) = self.open(event_loop, app) {
            self.failure = Some(error);
            event_loop.exit();
        }
    }

    #[cfg(target_os = "android")]
    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        // Android requires all native render surfaces to be released before
        // this callback returns. Retain views and signals across surface loss.
        for surface in &mut self.windows {
            surface.checkpoint();
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
        #[cfg(target_os = "android")]
        self.native_updates();
        let system = self.system;

        if let WindowEvent::ThemeChanged(theme) = &event {
            self.set_system_theme(conversion::theme_mode(*theme));
        }

        #[cfg(target_os = "android")]
        if matches!(event, WindowEvent::RedrawRequested)
            && self
                .compositor
                .as_ref()
                .is_some_and(Compositor::is_device_lost)
        {
            if let Err(error) = self.recover_graphics() {
                self.failure = Some(error);
                event_loop.exit();
            }
            return;
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
                if let Some(compositor) = self.compositor.as_mut()
                    && let Err(error) = surface.draw(compositor)
                {
                    #[cfg(target_os = "android")]
                    if matches!(
                        &error,
                        Error::PresentationFailed(
                            graphics::compositor::SurfaceError::OutOfMemory
                                | graphics::compositor::SurfaceError::Lost
                                | graphics::compositor::SurfaceError::Other
                        )
                    ) {
                        if let Err(error) = self.recover_graphics() {
                            self.failure = Some(error);
                            event_loop.exit();
                        }
                        return;
                    }
                    self.failure = Some(error);
                    event_loop.exit();
                }
            }
            WindowEvent::CloseRequested if surface.exit_on_close => {
                #[cfg(target_os = "android")]
                {
                    let response = surface.back();
                    if response.captured {
                        let window = surface.id;
                        self.serve(window, response);
                        return;
                    }
                }
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
                let accessibility = surface.absorb_changes(system);

                let window = surface.id;
                self.serve(window, response);
                for response in accessibility {
                    self.serve(window, response);
                }
            }
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, wake: Wake) {
        let system = self.system;

        match wake {
            #[cfg(target_os = "android")]
            Wake::Android => self.native_updates(),
            Wake::Changed(id) => {
                if let Some(surface) = self.window_by_id(id) {
                    let responses = surface.absorb_changes(system);
                    for response in responses {
                        self.serve(id, response);
                    }
                }
            }
            Wake::Clipboard(id, token, event) => {
                let Some(surface) = self.window_by_id(id) else {
                    return;
                };
                if !surface.accept_paste(&token) {
                    return;
                }
                let response = surface.handle(&core::Event::Clipboard(event));
                let accessibility = surface.absorb_changes(system);
                self.serve(id, response);
                for response in accessibility {
                    self.serve(id, response);
                }
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

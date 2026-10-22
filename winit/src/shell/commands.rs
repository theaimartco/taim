//! Carrying out [`Action`]s the application or the runtime asked for.
use super::engine::Engine;
use super::protocol::{Request, WindowRequest};
use super::views::{self, Views};
use crate::conversion;
use crate::core::widget::operation;
use crate::core::{self, Point, Renderer as _, Size, mouse, theme};
use crate::graphics::{self, Compositor, Shell};
use crate::program::{self, App, AppRunner};
use crate::runtime::{self, Action, backend, clipboard, font, image, system};
use crate::window::Window;

use runtime::window::Action as WindowAction;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalSize};

impl<A, C> Engine<A, C>
where
    A: App,
    A::Theme: theme::Base,
    C: Compositor<Renderer = A::Renderer> + 'static,
{
    pub(super) fn perform<'a>(
        &mut self,
        action: Action<A::Message>,
        runner: &'a AppRunner<A>,
        views: &mut Views<'a, A>,
    ) {
        self.proxy.acknowledge(&action);

        match action {
            Action::Output(message) => self.messages.push(message),
            Action::Event { window, event } => self.inbox.push(window, event),
            Action::Window(command) => self.window_command(command, runner, views),
            Action::Clipboard(command) => match command {
                clipboard::Action::Read { kind, channel } => {
                    self.clipboard.read(kind, move |result| {
                        let _ = channel.send(result);
                    });
                }
                clipboard::Action::Write { content, channel } => {
                    self.clipboard.write(content, move |result| {
                        let _ = channel.send(result);
                    });
                }
            },
            Action::System(command) => self.system_command(command, runner),
            Action::Font(command) => self.font_command(command, views),
            Action::Widget(operation) => {
                let mut pending = Some(operation);

                while let Some(mut operation) = pending.take() {
                    for (id, interface) in views.iter_mut() {
                        if let Some(window) = self.windows.get(id) {
                            interface.operate(&window.renderer, operation.as_mut());
                        }
                    }

                    if let operation::Outcome::Chain(next) = operation.finish() {
                        pending = Some(next);
                    }
                }

                self.redraw_all();
            }
            Action::Image(image::Action::Allocate(handle, reply)) => {
                // Renderers do not share an image cache yet; the first one
                // allocates on behalf of all of them.
                if let Some((_, window)) = self.windows.iter_mut().next() {
                    window.renderer.allocate_image(&handle, move |allocation| {
                        let _ = reply.send(allocation);
                    });
                }
            }
            Action::Backend(command) => self.backend_command(command),
            Action::Tick => {
                for (_, window) in self.windows.iter_mut() {
                    window.renderer.tick();
                }
            }
            Action::Reload => {
                for (id, window) in self.windows.iter_mut() {
                    let Some(interface) = views.remove(id) else {
                        continue;
                    };

                    views.insert(
                        id,
                        views::build(
                            runner,
                            interface.into_cache(),
                            &mut window.renderer,
                            window.state.logical_size(),
                            id,
                            window.state.scale_factor(),
                        ),
                    );
                    window.request_redraw(core::window::RedrawRequest::NextFrame);
                }
            }
            Action::Exit => self.request(Request::Quit),
        }
    }

    fn window_command<'a>(
        &mut self,
        command: WindowAction,
        runner: &'a AppRunner<A>,
        views: &mut Views<'a, A>,
    ) {
        match command {
            WindowAction::Open(id, settings, opened) => {
                let context = program::WindowContext::new(id, 1.0);

                self.request(Request::OpenWindow(WindowRequest {
                    id,
                    settings,
                    title: runner.title(&context),
                    scale_factor: runner.scale_factor(&context),
                    monitor: self.windows.last_monitor(),
                    opened,
                }));
                self.opening_window = true;
            }
            WindowAction::Close(id) => {
                let _ = views.remove(id);
                self.frames.untrack_window(id);

                if self.windows.remove(id).is_some() {
                    self.inbox
                        .push(id, core::Event::Window(core::window::Event::Closed));
                }

                // Surfaces and renderers are gone with the last window; a
                // compositor without them would pin the GPU device for nothing.
                if self.windows.is_empty() {
                    self.compositor = None;
                }
            }
            WindowAction::GetOldest(reply) => {
                let _ = reply.send(self.windows.iter_mut().next().map(|(id, _)| id));
            }
            WindowAction::GetLatest(reply) => {
                let _ = reply.send(self.windows.iter_mut().last().map(|(id, _)| id));
            }
            WindowAction::SetAllowAutomaticTabbing(enabled) => {
                self.request(Request::AutomaticTabbing(enabled));
            }
            WindowAction::RedrawAll => self.redraw_all(),
            WindowAction::RelayoutAll => {
                for (id, window) in self.windows.iter_mut() {
                    let size = window.state.logical_size();

                    if let Some(interface) =
                        views.replace(id, |ui| ui.relayout(size, &mut window.renderer))
                    {
                        window.hit_test_index = Some(interface.hit_test_index());
                    }

                    window.request_redraw(core::window::RedrawRequest::NextFrame);
                }
            }
            WindowAction::Screenshot(id, reply) => {
                if let (Some(window), Some(compositor)) =
                    (self.windows.get_mut(id), self.compositor.as_mut())
                {
                    let pixels = compositor.screenshot(
                        &mut window.renderer,
                        window.state.viewport(),
                        window.state.background_color(),
                    );

                    let _ = reply.send(core::window::Screenshot::new(
                        pixels,
                        window.state.physical_size(),
                        window.state.scale_factor(),
                    ));
                }
            }
            command => {
                if let Some(window) = addressed(&command).and_then(|id| self.windows.get_mut(id)) {
                    native_window_command(window, command);
                }
            }
        }
    }

    fn system_command(&mut self, command: system::Action, runner: &AppRunner<A>) {
        match command {
            system::Action::GetInformation(_reply) => {
                #[cfg(feature = "sysinfo")]
                if let Some(compositor) = &self.compositor {
                    let graphics = compositor.information();

                    // Probing the system is slow; keep it off the UI thread.
                    let _ = std::thread::spawn(move || {
                        let _ = _reply.send(system_information(graphics));
                    });
                }
            }
            system::Action::GetTheme(reply) => {
                let _ = reply.send(self.system_theme);
            }
            system::Action::NotifyTheme(mode) => {
                self.set_system_theme(mode);

                let Some(theme) = conversion::window_theme(mode) else {
                    return;
                };

                for (_, window) in self.windows.iter_mut() {
                    window.state.update(
                        runner,
                        &window.raw,
                        &winit::event::WindowEvent::ThemeChanged(theme),
                    );
                }
            }
        }
    }

    fn font_command(&mut self, command: font::Action, views: &mut Views<'_, A>) {
        match command {
            font::Action::Load { bytes, channel } => {
                if let Some(compositor) = &mut self.compositor {
                    let _ = channel.send(compositor.load_font(bytes));
                }
            }
            font::Action::List { channel } => {
                if let Some(compositor) = &mut self.compositor {
                    let _ = channel.send(compositor.list_fonts());
                }
            }
            font::Action::SetDefaults { font, text_size } => {
                self.renderer_settings.default_font = font;
                self.renderer_settings.default_text_size = text_size;

                let Some(compositor) = &mut self.compositor else {
                    return;
                };

                // Text metrics depend on the defaults: every window gets a
                // fresh renderer and a fresh layout.
                for (id, window) in self.windows.iter_mut() {
                    window.renderer = compositor.create_renderer(self.renderer_settings);

                    let size = window.state.logical_size();
                    let _ = views.replace(id, |ui| ui.relayout(size, &mut window.renderer));

                    window.request_redraw(core::window::RedrawRequest::NextFrame);
                }
            }
        }
    }

    fn backend_command(&mut self, command: backend::Action) {
        match command {
            #[cfg(not(target_arch = "wasm32"))]
            backend::Action::Configure(settings, reply) => {
                let Some(first) = self.windows.first() else {
                    return;
                };

                let shell = Shell::new(self.proxy.clone());
                let created = self.runtime.block_on(C::new(
                    settings,
                    first.raw.clone(),
                    first.raw.clone(),
                    shell,
                ));

                let mut compositor = match created {
                    Ok(compositor) => compositor,
                    Err(error) => {
                        let _ = reply.send(Err(error));
                        return;
                    }
                };

                // Everything cached against the old device is invalid now.
                graphics::cache::invalidate_all();

                let renderer_settings = self.renderer_settings;
                self.windows.replace_with(|mut window| {
                    let size = window.state.physical_size();

                    // Release the old device's resources before allocating
                    // on the new one.
                    drop(window.renderer);
                    drop(window.surface);

                    window.renderer = compositor.create_renderer(renderer_settings);
                    window.surface =
                        compositor.create_surface(window.raw.clone(), size.width, size.height);

                    window
                });

                self.compositor = Some(compositor);
                let _ = reply.send(Ok(()));
            }
            #[cfg(target_arch = "wasm32")]
            backend::Action::Configure(_, _) => {}
        }
    }
}

/// The window a window command is about, if it is about exactly one.
fn addressed(command: &WindowAction) -> Option<core::window::Id> {
    use WindowAction as W;

    match command {
        W::Open(id, ..)
        | W::Close(id)
        | W::Drag(id)
        | W::DragResize(id, _)
        | W::Resize(id, _)
        | W::GetSize(id, _)
        | W::GetMaximized(id, _)
        | W::Maximize(id, _)
        | W::GetMinimized(id, _)
        | W::Minimize(id, _)
        | W::GetPosition(id, _)
        | W::GetScaleFactor(id, _)
        | W::Move(id, _)
        | W::SetMode(id, _)
        | W::GetMode(id, _)
        | W::ToggleMaximize(id)
        | W::ToggleDecorations(id)
        | W::RequestUserAttention(id, _)
        | W::GainFocus(id)
        | W::SetLevel(id, _)
        | W::ShowSystemMenu(id)
        | W::GetRawId(id, _)
        | W::SetIcon(id, _)
        | W::Run(id, _)
        | W::Screenshot(id, _)
        | W::EnableMousePassthrough(id)
        | W::DisableMousePassthrough(id)
        | W::SetMinSize(id, _)
        | W::SetMaxSize(id, _)
        | W::SetResizable(id, _)
        | W::SetResizeIncrements(id, _)
        | W::GetMonitorSize(id, _) => Some(*id),
        W::GetOldest(_)
        | W::GetLatest(_)
        | W::SetAllowAutomaticTabbing(_)
        | W::RedrawAll
        | W::RelayoutAll => None,
    }
}

/// Window commands that only talk to the native window.
fn native_window_command<A, C>(window: &mut Window<A, C>, command: WindowAction)
where
    A: App,
    A::Theme: theme::Base,
    C: Compositor<Renderer = A::Renderer>,
{
    let scale = window.state.scale_factor();
    let physical = |size: Size| -> PhysicalSize<f32> {
        LogicalSize::new(size.width, size.height).to_physical(f64::from(scale))
    };
    let raw = &window.raw;

    match command {
        WindowAction::Drag(_) => {
            let _ = raw.drag_window();
        }
        WindowAction::DragResize(_, direction) => {
            let _ = raw.drag_resize_window(conversion::resize_direction(direction));
        }
        WindowAction::Resize(_, size) => {
            let _ = raw.request_inner_size(physical(size));
        }
        WindowAction::SetMinSize(_, size) => raw.set_min_inner_size(size.map(physical)),
        WindowAction::SetMaxSize(_, size) => raw.set_max_inner_size(size.map(physical)),
        WindowAction::SetResizeIncrements(_, step) => raw.set_resize_increments(step.map(physical)),
        WindowAction::SetResizable(_, resizable) => raw.set_resizable(resizable),
        WindowAction::GetSize(_, reply) => {
            let _ = reply.send(window.state.logical_size());
        }
        WindowAction::GetMaximized(_, reply) => {
            let _ = reply.send(raw.is_maximized());
        }
        WindowAction::Maximize(_, maximized) => raw.set_maximized(maximized),
        WindowAction::ToggleMaximize(_) => raw.set_maximized(!raw.is_maximized()),
        WindowAction::GetMinimized(_, reply) => {
            let _ = reply.send(raw.is_minimized());
        }
        WindowAction::Minimize(_, minimized) => raw.set_minimized(minimized),
        WindowAction::GetPosition(_, reply) => {
            let position = raw.outer_position().ok().map(|position| {
                let logical = position.to_logical::<f32>(raw.scale_factor());
                Point::new(logical.x, logical.y)
            });

            let _ = reply.send(position);
        }
        WindowAction::Move(_, position) => {
            raw.set_outer_position(LogicalPosition::new(position.x, position.y));
        }
        WindowAction::GetScaleFactor(_, reply) => {
            let _ = reply.send(raw.scale_factor() as f32);
        }
        WindowAction::GetMonitorSize(_, reply) => {
            let size = raw.current_monitor().map(|monitor| {
                let logical = monitor.size().to_logical::<f32>(f64::from(scale));
                Size::new(logical.width, logical.height)
            });

            let _ = reply.send(size);
        }
        WindowAction::SetMode(_, mode) => {
            raw.set_visible(conversion::visible(mode));
            raw.set_fullscreen(conversion::fullscreen(raw.current_monitor(), mode));
        }
        WindowAction::GetMode(_, reply) => {
            let mode = if raw.is_visible().unwrap_or(true) {
                conversion::mode(raw.fullscreen())
            } else {
                core::window::Mode::Hidden
            };

            let _ = reply.send(mode);
        }
        WindowAction::SetIcon(_, icon) => raw.set_window_icon(conversion::icon(icon)),
        WindowAction::ToggleDecorations(_) => raw.set_decorations(!raw.is_decorated()),
        WindowAction::RequestUserAttention(_, urgency) => {
            raw.request_user_attention(urgency.map(conversion::user_attention));
        }
        WindowAction::GainFocus(_) => raw.focus_window(),
        WindowAction::SetLevel(_, level) => raw.set_window_level(conversion::window_level(level)),
        WindowAction::ShowSystemMenu(_) => {
            if let mouse::Cursor::Available(point) = window.state.cursor() {
                raw.show_window_menu(LogicalPosition::new(point.x, point.y));
            }
        }
        WindowAction::GetRawId(_, reply) => {
            let _ = reply.send(raw.id().into());
        }
        WindowAction::Run(_, f) => f(raw),
        WindowAction::EnableMousePassthrough(_) => {
            let _ = raw.set_cursor_hittest(false);
        }
        WindowAction::DisableMousePassthrough(_) => {
            let _ = raw.set_cursor_hittest(true);
        }
        WindowAction::Open(..)
        | WindowAction::Close(_)
        | WindowAction::GetOldest(_)
        | WindowAction::GetLatest(_)
        | WindowAction::SetAllowAutomaticTabbing(_)
        | WindowAction::RedrawAll
        | WindowAction::RelayoutAll
        | WindowAction::Screenshot(..) => {
            unreachable!("engine-level window commands are handled by the engine")
        }
    }
}

#[cfg(feature = "sysinfo")]
fn system_information(graphics: graphics::compositor::Information) -> system::Information {
    use sysinfo::{Process, System};

    let mut probe = System::new_all();
    probe.refresh_all();

    let memory_used = sysinfo::get_current_pid()
        .ok()
        .and_then(|pid| probe.process(pid))
        .map(Process::memory);

    system::Information {
        system_name: System::name(),
        system_kernel: System::kernel_version(),
        system_version: System::long_os_version(),
        system_short_version: System::os_version(),
        cpu_brand: probe
            .cpus()
            .first()
            .map(|cpu| cpu.brand().to_owned())
            .unwrap_or_default(),
        cpu_cores: probe.physical_core_count(),
        memory_total: probe.total_memory(),
        memory_used,
        graphics_adapter: graphics.adapter,
        graphics_backend: graphics.backend,
    }
}

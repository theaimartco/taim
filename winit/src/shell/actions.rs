use super::runner::{Control, PendingEvents};
use super::*;

/// Builds a window's [`UserInterface`] for the [`App`].
pub(super) fn build_user_interface<'a, A: program::App>(
    runner: &'a program::AppRunner<A>,
    cache: user_interface::Cache,
    renderer: &mut A::Renderer,
    size: Size,
    id: window::Id,
    scale_factor: f32,
) -> UserInterface<'a, A::Message, A::Theme, A::Renderer>
where
    A::Theme: theme::Base,
{
    let view_span = debug::view(id);
    let win_ctx = program::WindowContext::new(id, scale_factor);
    let view = runner.view(&win_ctx);
    view_span.finish();

    let layout_span = debug::layout(id);
    let user_interface = UserInterface::build(view, size, cache, renderer);
    layout_span.finish();

    user_interface
}

pub(super) fn update<A: program::App>(
    runner: &mut program::AppRunner<A>,
    runtime: &mut Runtime<A::Executor, Proxy<A::Message>, Action<A::Message>>,
    messages: &mut Vec<A::Message>,
    actions: &mut Vec<Action<A::Message>>,
    outputs: &mut Vec<A::Message>,
) where
    A::Theme: theme::Base,
{
    use futures::futures;

    debug_assert!(actions.is_empty());
    debug_assert!(outputs.is_empty());

    while !messages.is_empty() {
        for message in messages.drain(..) {
            let task = runtime.enter(|| runner.update(message));

            if let Some(mut stream) = runtime::task::into_stream(task) {
                let waker = futures::task::noop_waker_ref();
                let mut context = futures::task::Context::from_waker(waker);

                // Run immediately available actions synchronously (e.g. widget operations)
                loop {
                    match runtime.enter(|| stream.poll_next_unpin(&mut context)) {
                        futures::task::Poll::Ready(Some(Action::Output(output))) => {
                            outputs.push(output);
                        }
                        futures::task::Poll::Ready(Some(action)) => {
                            actions.push(action);
                        }
                        futures::task::Poll::Ready(None) => {
                            break;
                        }
                        futures::task::Poll::Pending => {
                            let _ = runtime.run(stream);
                            break;
                        }
                    }
                }
            }
        }

        messages.append(outputs);
    }

    let subscription = runtime.enter(|| runner.subscription());
    let recipes = subscription::into_recipes(subscription.map(Action::Output));

    runtime.track(recipes);
}

pub(super) fn run_action<'a, A, C>(
    action: Action<A::Message>,
    runner: &'a program::AppRunner<A>,
    proxy: &Proxy<A::Message>,
    runtime: &mut Runtime<A::Executor, Proxy<A::Message>, Action<A::Message>>,
    compositor: &mut Option<C>,
    events: &mut PendingEvents,
    messages: &mut Vec<A::Message>,
    clipboard: &mut Clipboard,
    control_sender: &mut mpsc::UnboundedSender<Control>,
    interfaces: &mut FxHashMap<window::Id, UserInterface<'a, A::Message, A::Theme, A::Renderer>>,
    window_manager: &mut window::Manager<A, C>,
    frame_gate: &mut crate::frame_gate::FrameGate,
    is_window_opening: &mut bool,
    system_theme: &mut theme::Mode,
    renderer_settings: &mut renderer::Settings,
) where
    A: program::App,
    C: Compositor<Renderer = A::Renderer> + 'static,
    A::Theme: theme::Base,
{
    use crate::core::Renderer as _;
    use crate::runtime::backend;
    use crate::runtime::clipboard;
    use crate::runtime::window;

    proxy.acknowledge(&action);

    match action {
        Action::Output(message) => {
            messages.push(message);
        }
        Action::Clipboard(action) => match action {
            clipboard::Action::Read { kind, channel } => {
                clipboard.read(kind, move |result| {
                    let _ = channel.send(result);
                });
            }
            clipboard::Action::Write { content, channel } => {
                clipboard.write(content, move |result| {
                    let _ = channel.send(result);
                });
            }
        },
        Action::Window(action) => match action {
            window::Action::Open(id, settings, channel) => {
                let monitor = window_manager.last_monitor();
                let win_ctx = program::WindowContext::new(id, 1.0);

                control_sender
                    .start_send(Control::CreateWindow {
                        id,
                        settings,
                        title: runner.title(&win_ctx),
                        scale_factor: runner.scale_factor(&win_ctx),
                        monitor,
                        on_open: channel,
                    })
                    .expect("Send control action");

                *is_window_opening = true;
            }
            window::Action::Close(id) => {
                let _ = interfaces.remove(&id);
                frame_gate.untrack_window(id);

                if window_manager.remove(id).is_some() {
                    events.push(id, core::Event::Window(core::window::Event::Closed));
                }

                if window_manager.is_empty() {
                    *compositor = None;
                }
            }
            window::Action::GetOldest(channel) => {
                let id = window_manager.iter_mut().next().map(|(id, _window)| id);

                let _ = channel.send(id);
            }
            window::Action::GetLatest(channel) => {
                let id = window_manager.iter_mut().last().map(|(id, _window)| id);

                let _ = channel.send(id);
            }
            window::Action::Drag(id) => {
                if let Some(window) = window_manager.get_mut(id) {
                    let _ = window.raw.drag_window();
                }
            }
            window::Action::DragResize(id, direction) => {
                if let Some(window) = window_manager.get_mut(id) {
                    let _ = window
                        .raw
                        .drag_resize_window(conversion::resize_direction(direction));
                }
            }
            window::Action::Resize(id, size) => {
                if let Some(window) = window_manager.get_mut(id) {
                    let _ = window.raw.request_inner_size(
                        winit::dpi::LogicalSize {
                            width: size.width,
                            height: size.height,
                        }
                        .to_physical::<f32>(f64::from(window.state.scale_factor())),
                    );
                }
            }
            window::Action::SetMinSize(id, size) => {
                if let Some(window) = window_manager.get_mut(id) {
                    window.raw.set_min_inner_size(size.map(|size| {
                        winit::dpi::LogicalSize {
                            width: size.width,
                            height: size.height,
                        }
                        .to_physical::<f32>(f64::from(window.state.scale_factor()))
                    }));
                }
            }
            window::Action::SetMaxSize(id, size) => {
                if let Some(window) = window_manager.get_mut(id) {
                    window.raw.set_max_inner_size(size.map(|size| {
                        winit::dpi::LogicalSize {
                            width: size.width,
                            height: size.height,
                        }
                        .to_physical::<f32>(f64::from(window.state.scale_factor()))
                    }));
                }
            }
            window::Action::SetResizeIncrements(id, increments) => {
                if let Some(window) = window_manager.get_mut(id) {
                    window.raw.set_resize_increments(increments.map(|size| {
                        winit::dpi::LogicalSize {
                            width: size.width,
                            height: size.height,
                        }
                        .to_physical::<f32>(f64::from(window.state.scale_factor()))
                    }));
                }
            }
            window::Action::SetResizable(id, resizable) => {
                if let Some(window) = window_manager.get_mut(id) {
                    window.raw.set_resizable(resizable);
                }
            }
            window::Action::GetSize(id, channel) => {
                if let Some(window) = window_manager.get_mut(id) {
                    let size = window.state.logical_size();
                    let _ = channel.send(Size::new(size.width, size.height));
                }
            }
            window::Action::GetMaximized(id, channel) => {
                if let Some(window) = window_manager.get_mut(id) {
                    let _ = channel.send(window.raw.is_maximized());
                }
            }
            window::Action::Maximize(id, maximized) => {
                if let Some(window) = window_manager.get_mut(id) {
                    window.raw.set_maximized(maximized);
                }
            }
            window::Action::GetMinimized(id, channel) => {
                if let Some(window) = window_manager.get_mut(id) {
                    let _ = channel.send(window.raw.is_minimized());
                }
            }
            window::Action::Minimize(id, minimized) => {
                if let Some(window) = window_manager.get_mut(id) {
                    window.raw.set_minimized(minimized);
                }
            }
            window::Action::GetPosition(id, channel) => {
                if let Some(window) = window_manager.get(id) {
                    let position = window
                        .raw
                        .outer_position()
                        .map(|position| {
                            let position = position.to_logical::<f32>(window.raw.scale_factor());

                            Point::new(position.x, position.y)
                        })
                        .ok();

                    let _ = channel.send(position);
                }
            }
            window::Action::GetScaleFactor(id, channel) => {
                if let Some(window) = window_manager.get_mut(id) {
                    let scale_factor = window.raw.scale_factor();

                    let _ = channel.send(scale_factor as f32);
                }
            }
            window::Action::Move(id, position) => {
                if let Some(window) = window_manager.get_mut(id) {
                    window.raw.set_outer_position(winit::dpi::LogicalPosition {
                        x: position.x,
                        y: position.y,
                    });
                }
            }
            window::Action::SetMode(id, mode) => {
                if let Some(window) = window_manager.get_mut(id) {
                    window.raw.set_visible(conversion::visible(mode));
                    window
                        .raw
                        .set_fullscreen(conversion::fullscreen(window.raw.current_monitor(), mode));
                }
            }
            window::Action::SetIcon(id, icon) => {
                if let Some(window) = window_manager.get_mut(id) {
                    window.raw.set_window_icon(conversion::icon(icon));
                }
            }
            window::Action::GetMode(id, channel) => {
                if let Some(window) = window_manager.get_mut(id) {
                    let mode = if window.raw.is_visible().unwrap_or(true) {
                        conversion::mode(window.raw.fullscreen())
                    } else {
                        core::window::Mode::Hidden
                    };

                    let _ = channel.send(mode);
                }
            }
            window::Action::ToggleMaximize(id) => {
                if let Some(window) = window_manager.get_mut(id) {
                    window.raw.set_maximized(!window.raw.is_maximized());
                }
            }
            window::Action::ToggleDecorations(id) => {
                if let Some(window) = window_manager.get_mut(id) {
                    window.raw.set_decorations(!window.raw.is_decorated());
                }
            }
            window::Action::RequestUserAttention(id, attention_type) => {
                if let Some(window) = window_manager.get_mut(id) {
                    window
                        .raw
                        .request_user_attention(attention_type.map(conversion::user_attention));
                }
            }
            window::Action::GainFocus(id) => {
                if let Some(window) = window_manager.get_mut(id) {
                    window.raw.focus_window();
                }
            }
            window::Action::SetLevel(id, level) => {
                if let Some(window) = window_manager.get_mut(id) {
                    window.raw.set_window_level(conversion::window_level(level));
                }
            }
            window::Action::ShowSystemMenu(id) => {
                if let Some(window) = window_manager.get_mut(id)
                    && let mouse::Cursor::Available(point) = window.state.cursor()
                {
                    window.raw.show_window_menu(winit::dpi::LogicalPosition {
                        x: point.x,
                        y: point.y,
                    });
                }
            }
            window::Action::GetRawId(id, channel) => {
                if let Some(window) = window_manager.get_mut(id) {
                    let _ = channel.send(window.raw.id().into());
                }
            }
            window::Action::Run(id, f) => {
                if let Some(window) = window_manager.get_mut(id) {
                    f(&window.raw);
                }
            }
            window::Action::Screenshot(id, channel) => {
                if let Some(window) = window_manager.get_mut(id)
                    && let Some(compositor) = compositor
                {
                    let bytes = compositor.screenshot(
                        &mut window.renderer,
                        window.state.viewport(),
                        window.state.background_color(),
                    );

                    let _ = channel.send(core::window::Screenshot::new(
                        bytes,
                        window.state.physical_size(),
                        window.state.scale_factor(),
                    ));
                }
            }
            window::Action::EnableMousePassthrough(id) => {
                if let Some(window) = window_manager.get_mut(id) {
                    let _ = window.raw.set_cursor_hittest(false);
                }
            }
            window::Action::DisableMousePassthrough(id) => {
                if let Some(window) = window_manager.get_mut(id) {
                    let _ = window.raw.set_cursor_hittest(true);
                }
            }
            window::Action::GetMonitorSize(id, channel) => {
                if let Some(window) = window_manager.get(id) {
                    let size = window.raw.current_monitor().map(|monitor| {
                        let scale = window.state.scale_factor();
                        let size = monitor.size().to_logical(f64::from(scale));

                        Size::new(size.width, size.height)
                    });

                    let _ = channel.send(size);
                }
            }
            window::Action::SetAllowAutomaticTabbing(enabled) => {
                control_sender
                    .start_send(Control::SetAutomaticWindowTabbing(enabled))
                    .expect("Send control action");
            }
            window::Action::RedrawAll => {
                for (_id, window) in window_manager.iter_mut() {
                    window.request_redraw(core::window::RedrawRequest::NextFrame);
                }
            }
            window::Action::RelayoutAll => {
                for (id, window) in window_manager.iter_mut() {
                    if let Some(ui) = interfaces.remove(&id) {
                        let mut ui = ui.relayout(window.state.logical_size(), &mut window.renderer);
                        window.hit_test_index = Some(ui.hit_test_index());
                        let _ = interfaces.insert(id, ui);
                    }

                    window.request_redraw(core::window::RedrawRequest::NextFrame);
                }
            }
        },
        Action::System(action) => match action {
            system::Action::GetInformation(_channel) => {
                #[cfg(feature = "sysinfo")]
                {
                    if let Some(compositor) = compositor {
                        let graphics_info = compositor.information();

                        let _ = std::thread::spawn(move || {
                            let information = system_information(graphics_info);

                            let _ = _channel.send(information);
                        });
                    }
                }
            }
            system::Action::GetTheme(channel) => {
                let _ = channel.send(*system_theme);
            }
            system::Action::NotifyTheme(mode) => {
                if mode != *system_theme {
                    *system_theme = mode;

                    runtime.broadcast(subscription::Event::SystemThemeChanged(mode));
                }

                let Some(theme) = conversion::window_theme(mode) else {
                    return;
                };

                for (_id, window) in window_manager.iter_mut() {
                    window.state.update(
                        runner,
                        &window.raw,
                        &winit::event::WindowEvent::ThemeChanged(theme),
                    );
                }
            }
        },
        Action::Font(action) => match action {
            font::Action::Load { bytes, channel } => {
                if let Some(compositor) = compositor {
                    let result = compositor.load_font(bytes.clone());
                    let _ = channel.send(result);
                }
            }
            font::Action::List { channel } => {
                if let Some(compositor) = compositor {
                    let fonts = compositor.list_fonts();
                    let _ = channel.send(fonts);
                }
            }
            font::Action::SetDefaults { font, text_size } => {
                renderer_settings.default_font = font;
                renderer_settings.default_text_size = text_size;

                let Some(compositor) = compositor else {
                    return;
                };

                // Recreate renderers and relayout all windows
                for (id, window) in window_manager.iter_mut() {
                    window.renderer = compositor.create_renderer(*renderer_settings);

                    let Some(ui) = interfaces.remove(&id) else {
                        continue;
                    };

                    let size = window.state.logical_size();
                    let ui = ui.relayout(size, &mut window.renderer);
                    let _ = interfaces.insert(id, ui);

                    window.request_redraw(core::window::RedrawRequest::NextFrame);
                }
            }
        },
        Action::Widget(operation) => {
            let mut current_operation = Some(operation);

            while let Some(mut operation) = current_operation.take() {
                for (id, ui) in interfaces.iter_mut() {
                    if let Some(window) = window_manager.get_mut(*id) {
                        ui.operate(&window.renderer, operation.as_mut());
                    }
                }

                match operation.finish() {
                    operation::Outcome::None => {}
                    operation::Outcome::Some(()) => {}
                    operation::Outcome::Chain(next) => {
                        current_operation = Some(next);
                    }
                }
            }

            // Redraw all windows
            for (_, window) in window_manager.iter_mut() {
                window.request_redraw(core::window::RedrawRequest::NextFrame);
            }
        }
        Action::Image(action) => match action {
            image::Action::Allocate(handle, sender) => {
                // TODO: Shared image cache in compositor
                if let Some((_id, window)) = window_manager.iter_mut().next() {
                    window.renderer.allocate_image(&handle, move |allocation| {
                        let _ = sender.send(allocation);
                    });
                }
            }
        },
        Action::Backend(action) => match action {
            #[cfg(not(target_arch = "wasm32"))]
            backend::Action::Configure(settings, sender) => {
                let shell = Shell::new(proxy.clone());

                let mut new_compositor = if let Some(window) = window_manager.first() {
                    match runtime.block_on(C::new(
                        settings,
                        window.raw.clone(),
                        window.raw.clone(),
                        shell,
                    )) {
                        Ok(compositor) => compositor,
                        Err(error) => {
                            let _ = sender.send(Err(error));
                            return;
                        }
                    }
                } else {
                    return;
                };

                graphics::cache::invalidate_all();

                window_manager.replace_with(|mut window| {
                    let size = window.state.physical_size();

                    drop(window.renderer);
                    drop(window.surface);

                    window.renderer = new_compositor.create_renderer(*renderer_settings);
                    window.surface =
                        new_compositor.create_surface(window.raw.clone(), size.width, size.height);

                    window
                });

                *compositor = Some(new_compositor);

                let _ = sender.send(Ok(()));
            }
            #[cfg(target_arch = "wasm32")]
            backend::Action::Configure(_, _) => {}
        },
        Action::Event { window, event } => {
            events.push(window, event);
        }
        Action::Tick => {
            for (_id, window) in window_manager.iter_mut() {
                window.renderer.tick();
            }
        }
        Action::Reload => {
            for (id, window) in window_manager.iter_mut() {
                let Some(ui) = interfaces.remove(&id) else {
                    continue;
                };

                let cache = ui.into_cache();
                let size = window.state.logical_size();

                let _ = interfaces.insert(
                    id,
                    build_user_interface(
                        runner,
                        cache,
                        &mut window.renderer,
                        size,
                        id,
                        window.state.scale_factor(),
                    ),
                );

                window.request_redraw(core::window::RedrawRequest::NextFrame);
            }
        }
        Action::Exit => {
            control_sender
                .start_send(Control::Exit)
                .expect("Send control action");
        }
    }
}

/// Build the user interface for every window.
pub fn build_user_interfaces<'a, A: program::App, C>(
    runner: &'a program::AppRunner<A>,
    window_manager: &mut window::Manager<A, C>,
    mut cached_user_interfaces: FxHashMap<window::Id, user_interface::Cache>,
    proxy: &mut Proxy<A::Message>,
) -> FxHashMap<window::Id, UserInterface<'a, A::Message, A::Theme, A::Renderer>>
where
    C: Compositor<Renderer = A::Renderer>,
    A::Theme: theme::Base,
{
    for (id, window) in window_manager.iter_mut() {
        let old_size = window.state.logical_size();

        window.state.synchronize(runner, id, &window.raw);

        let new_size = window.state.logical_size();

        if old_size != new_size {
            proxy.send_action(Action::Event {
                window: id,
                event: core::Event::Window(window::Event::Resized(new_size)),
            });
        }

        #[cfg(feature = "hinting")]
        window.renderer.hint(window.state.scale_factor());

        // `synchronize` above re-reads the theme, so this is where a theme
        // change reaches the renderer. Without it a scheme switch would
        // move every colour and leave the light where it was.
        window
            .renderer
            .set_light(theme::Base::light(window.state.theme()));
    }

    debug::theme_changed(|| {
        window_manager
            .first()
            .and_then(|window| theme::Base::seed(window.state.theme()))
    });

    cached_user_interfaces
        .drain()
        .filter_map(|(id, cache)| {
            let window = window_manager.get_mut(id)?;

            Some((
                id,
                build_user_interface(
                    runner,
                    cache,
                    &mut window.renderer,
                    window.state.logical_size(),
                    id,
                    window.state.scale_factor(),
                ),
            ))
        })
        .collect()
}

/// Returns true if the provided event should cause a [`Program`] to
/// exit.
pub fn user_force_quit(
    event: &winit::event::WindowEvent,
    _modifiers: winit::keyboard::ModifiersState,
) -> bool {
    match event {
        #[cfg(target_os = "macos")]
        winit::event::WindowEvent::KeyboardInput {
            event:
                winit::event::KeyEvent {
                    logical_key: winit::keyboard::Key::Character(c),
                    state: winit::event::ElementState::Pressed,
                    ..
                },
            ..
        } if c == "q" && _modifiers.super_key() => true,
        _ => false,
    }
}

#[cfg(feature = "sysinfo")]
pub(super) fn system_information(graphics: compositor::Information) -> system::Information {
    use sysinfo::{Process, System};

    let mut system = System::new_all();
    system.refresh_all();

    let cpu_brand = system
        .cpus()
        .first()
        .map(|cpu| cpu.brand().to_string())
        .unwrap_or_default();

    let memory_used = sysinfo::get_current_pid()
        .and_then(|pid| system.process(pid).ok_or("Process not found"))
        .map(Process::memory)
        .ok();

    system::Information {
        system_name: System::name(),
        system_kernel: System::kernel_version(),
        system_version: System::long_os_version(),
        system_short_version: System::os_version(),
        cpu_brand,
        cpu_cores: system.physical_core_count(),
        memory_total: system.total_memory(),
        memory_used,
        graphics_adapter: graphics.adapter,
        graphics_backend: graphics.backend,
    }
}

pub(super) fn run_clipboard<Message: Send>(
    proxy: &mut Proxy<Message>,
    clipboard: &mut Clipboard,
    requests: core::Clipboard,
    window: window::Id,
) {
    for kind in requests.reads {
        let proxy = proxy.clone();

        clipboard.read(kind, move |result| {
            proxy.send_action(Action::Event {
                window,
                event: core::Event::Clipboard(core::clipboard::Event::Read(result.map(Arc::new))),
            });
        });
    }

    if let Some(content) = requests.write {
        let proxy = proxy.clone();

        clipboard.write(content, move |result| {
            proxy.send_action(Action::Event {
                window,
                event: core::Event::Clipboard(core::clipboard::Event::Written(result)),
            });
        });
    }
}

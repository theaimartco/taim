use super::actions::{
    build_user_interface, build_user_interfaces, run_action, run_clipboard, update,
};
use super::runner::{Control, Event, PendingEvents};
use super::*;

pub(super) async fn run_instance<A>(
    mut runner: program::AppRunner<A>,
    mut runtime: Runtime<A::Executor, Proxy<A::Message>, Action<A::Message>>,
    mut proxy: Proxy<A::Message>,
    mut event_receiver: mpsc::UnboundedReceiver<Event<Action<A::Message>>>,
    mut control_sender: mpsc::UnboundedSender<Control>,
    display_handle: winit::event_loop::OwnedDisplayHandle,
    is_daemon: bool,
    backend_settings: backend::Settings,
    mut renderer_settings: renderer::Settings,
    default_fonts: Vec<Cow<'static, [u8]>>,
    mut _system_theme: oneshot::Receiver<theme::Mode>,
) where
    A: program::App + 'static,
    A::Theme: theme::Base,
{
    use winit::event;
    use winit::event_loop::ControlFlow;

    let mut window_manager = window::Manager::new();
    let mut frame_gate = crate::frame_gate::FrameGate::new();
    let mut is_window_opening = !is_daemon;

    let proxy_waker = proxy.clone();
    runtime.scheduler_mut().set_notifier(move || {
        proxy_waker.send_action(Action::Tick);
    });

    let mut compositor = None;
    let mut events = PendingEvents::default();
    let mut messages = Vec::new();
    let mut update_actions = Vec::new();
    let mut update_outputs = Vec::new();
    let mut actions = 0;

    let mut user_interfaces = ManuallyDrop::new(FxHashMap::default());
    let mut clipboard = Clipboard::new();

    #[cfg(all(feature = "linux-theme-detection", target_os = "linux"))]
    let mut system_theme = {
        let to_mode = |color_scheme| match color_scheme {
            mundy::ColorScheme::NoPreference => theme::Mode::None,
            mundy::ColorScheme::Light => theme::Mode::Light,
            mundy::ColorScheme::Dark => theme::Mode::Dark,
        };

        runtime.run(
            mundy::Preferences::stream(mundy::Interest::ColorScheme)
                .map(move |preferences| {
                    Action::System(system::Action::NotifyTheme(to_mode(
                        preferences.color_scheme,
                    )))
                })
                .boxed(),
        );

        runtime
            .enter(|| {
                mundy::Preferences::once_blocking(
                    mundy::Interest::ColorScheme,
                    core::time::Duration::from_millis(200),
                )
            })
            .map(|preferences| to_mode(preferences.color_scheme))
            .unwrap_or_default()
    };

    #[cfg(not(all(feature = "linux-theme-detection", target_os = "linux")))]
    let mut system_theme = _system_theme.try_recv().ok().flatten().unwrap_or_default();

    log::info!("System theme: {system_theme:?}");

    'next_event: loop {
        let _ = runtime.poll_tasks();

        // Empty the queue if possible
        let event = if let Ok(event) = event_receiver.try_recv() {
            Some(event)
        } else {
            event_receiver.next().await
        };

        let Some(event) = event else {
            break;
        };

        match event {
            Event::WindowCreated {
                id,
                window,
                exit_on_close_request,
                make_visible,
                on_open,
            } => {
                if compositor.is_none() {
                    let (compositor_sender, compositor_receiver) = oneshot::channel();

                    let create_compositor = {
                        let window = window.clone();
                        let backend_settings = backend_settings.clone();
                        let display_handle = display_handle.clone();
                        let proxy = proxy.clone();
                        let default_fonts = default_fonts.clone();

                        async move {
                            let shell = Shell::new(proxy.clone());

                            let mut compositor =
                                <A::Renderer as compositor::Default>::Compositor::new(
                                    backend_settings,
                                    display_handle,
                                    window,
                                    shell,
                                )
                                .await;

                            if let Ok(compositor) = &mut compositor {
                                for font in default_fonts {
                                    compositor.load_font(font.clone());
                                }
                            }

                            compositor_sender
                                .send(compositor)
                                .ok()
                                .expect("Send compositor");

                            // HACK! Send a proxy event on completion to trigger
                            // a runtime re-poll
                            // TODO: Send compositor through proxy (?)
                            {
                                let (sender, _receiver) = oneshot::channel();

                                proxy.send_action(Action::Window(
                                    runtime::window::Action::GetLatest(sender),
                                ));
                            }
                        }
                    };

                    #[cfg(target_arch = "wasm32")]
                    wasm_bindgen_futures::spawn_local(create_compositor);

                    #[cfg(not(target_arch = "wasm32"))]
                    runtime.block_on(create_compositor);

                    match compositor_receiver.await.expect("Wait for compositor") {
                        Ok(new_compositor) => {
                            compositor = Some(new_compositor);
                        }
                        Err(error) => {
                            let _ = control_sender.start_send(Control::Crash(error.into()));
                            continue;
                        }
                    }
                }

                let window_theme = window
                    .theme()
                    .map(conversion::theme_mode)
                    .unwrap_or_default();

                if system_theme != window_theme {
                    system_theme = window_theme;

                    runtime.broadcast(subscription::Event::SystemThemeChanged(window_theme));
                }

                let is_first = window_manager.is_empty();
                let window = window_manager.insert(
                    id,
                    window,
                    &runner,
                    compositor.as_mut().expect("Compositor must be initialized"),
                    proxy.clone(),
                    renderer_settings,
                    exit_on_close_request,
                    system_theme,
                );
                frame_gate.track_window(id, window.raw.scale_factor() as f32);
                if window.state.physical_size().width == 0
                    || window.state.physical_size().height == 0
                {
                    let _ = frame_gate.handle_event(
                        id,
                        crate::frame_gate::PlatformEvent::SurfaceSizeChanged { zero_sized: true },
                    );
                }

                window
                    .raw
                    .set_theme(conversion::window_theme(window.state.theme_mode()));

                debug::theme_changed(|| {
                    if is_first {
                        theme::Base::seed(window.state.theme())
                    } else {
                        None
                    }
                });

                let logical_size = window.state.logical_size();

                #[cfg(feature = "hinting")]
                window.renderer.hint(window.state.scale_factor());

                // One light for every surface in the window, owned by the
                // theme rather than by a widget. Ungated: a material
                // responds to it whether or not metrics are hinted.
                window
                    .renderer
                    .set_light(theme::Base::light(window.state.theme()));

                let mut ui = build_user_interface(
                    &runner,
                    user_interface::Cache::default(),
                    &mut window.renderer,
                    logical_size,
                    id,
                    window.state.scale_factor(),
                );
                window.hit_test_index = Some(ui.hit_test_index());
                let _ = user_interfaces.insert(id, ui);

                if make_visible {
                    window.raw.set_visible(true);
                }

                // A freshly inserted window's scheduler starts in
                // `State::Idle` (`Scheduler::new`), under which
                // `should_produce_frame` is `false` for every subsequent
                // `RedrawRequested`. Nothing else transitions a new
                // window's scheduler out of `Idle` before the first
                // user input arrives, so without this the window would
                // sit fully rendered-never: winit keeps delivering
                // `RedrawRequested` (e.g. after its own initial
                // `Resized`/`ScaleFactorChanged`), and every one of them
                // is silently dropped by the scheduler gate. Requesting
                // the first frame through the wrapper (not
                // `window.raw.request_redraw()`) puts the scheduler in
                // `State::Requested` so that first `RedrawRequested`
                // actually reaches `present`.
                window.request_redraw(core::window::RedrawRequest::NextFrame);

                events.push(
                    id,
                    core::Event::Window(window::Event::Opened {
                        position: window.position(),
                        size: window.state.logical_size(),
                        scale_factor: window.raw.scale_factor() as f32,
                    }),
                );

                let _ = on_open.send(id);
                is_window_opening = false;
            }
            Event::EventLoopAwakened(event) => {
                match event {
                    event::Event::NewEvents(event::StartCause::Init) => {
                        for (_id, window) in window_manager.iter_mut() {
                            window.request_redraw(core::window::RedrawRequest::NextFrame);
                        }
                    }
                    event::Event::NewEvents(event::StartCause::ResumeTimeReached { .. }) => {
                        let now = Instant::now();

                        for (_id, window) in window_manager.iter_mut() {
                            window.scheduler.tick(now);

                            if window.scheduler.should_produce_frame(now) {
                                window.raw.request_redraw();
                            }
                        }

                        if let Some(redraw_at) = window_manager.redraw_at() {
                            let _ = control_sender
                                .start_send(Control::ChangeFlow(ControlFlow::WaitUntil(redraw_at)));
                        } else {
                            let _ =
                                control_sender.start_send(Control::ChangeFlow(ControlFlow::Wait));
                        }
                    }
                    event::Event::UserEvent(action) => {
                        run_action(
                            action,
                            &runner,
                            &proxy,
                            &mut runtime,
                            &mut compositor,
                            &mut events,
                            &mut messages,
                            &mut clipboard,
                            &mut control_sender,
                            &mut user_interfaces,
                            &mut window_manager,
                            &mut frame_gate,
                            &mut is_window_opening,
                            &mut system_theme,
                            &mut renderer_settings,
                        );
                        actions += 1;
                    }
                    event::Event::WindowEvent {
                        window_id: id,
                        event: event::WindowEvent::RedrawRequested,
                        ..
                    } => {
                        // Diagnostic (see `future.md`, Ceiling 8 / H0):
                        // confirms whether the platform ever delivers the
                        // redraw that leads to the first `present()` call.
                        log::debug!("RedrawRequested for id `{id:?}`");

                        let Some(mut current_compositor) = compositor.as_mut() else {
                            continue;
                        };

                        let Some((id, mut window)) = window_manager.get_mut_alias(id) else {
                            continue;
                        };

                        let physical_size = window.state.physical_size();
                        let mut logical_size = window.state.logical_size();

                        if physical_size.width == 0 || physical_size.height == 0 {
                            window.set_zero_sized(true);
                            let _ = frame_gate.handle_event(
                                id,
                                crate::frame_gate::PlatformEvent::SurfaceSizeChanged {
                                    zero_sized: true,
                                },
                            );
                            continue;
                        }

                        // A `RedrawRequested` event may still be in the
                        // OS's queue from before an `Occluded(true)` was
                        // processed; honoring it anyway would submit
                        // work for a surface the scheduler has already
                        // been told to stop producing frames for.
                        if !window.scheduler.begin_frame(Instant::now()) {
                            continue;
                        }

                        let _ = frame_gate
                            .handle_event(id, crate::frame_gate::PlatformEvent::RedrawRequested);

                        // Window was resized between redraws
                        if window.surface_version != window.state.surface_version() {
                            #[cfg(feature = "hinting")]
                            window.renderer.hint(window.state.scale_factor());

                            let ui = user_interfaces.remove(&id).expect("Remove user interface");

                            let layout_span = debug::layout(id);
                            let mut ui = ui.relayout(logical_size, &mut window.renderer);
                            window.hit_test_index = Some(ui.hit_test_index());
                            let _ = user_interfaces.insert(id, ui);
                            layout_span.finish();

                            current_compositor.configure_surface(
                                &mut window.surface,
                                physical_size.width,
                                physical_size.height,
                            );

                            window.surface_version = window.state.surface_version();
                        }

                        let redraw_event =
                            core::Event::Window(window::Event::RedrawRequested(Instant::now()));

                        let cursor = window.state.cursor();

                        let mut interface =
                            user_interfaces.get_mut(&id).expect("Get user interface");

                        let interact_span = debug::interact(id);
                        let mut redraw_count = 0;

                        let state = loop {
                            let message_count = messages.len();
                            let (state, _) = interface.update(
                                &window.raw,
                                &window.waker,
                                slice::from_ref(&redraw_event),
                                cursor,
                                &mut window.renderer,
                                &mut messages,
                            );

                            if message_count == messages.len() && !state.has_layout_changed() {
                                break state;
                            }

                            if redraw_count >= 2 {
                                log::warn!(
                                    "More than 3 consecutive RedrawRequested events \
                                    produced layout invalidation"
                                );

                                break state;
                            }

                            redraw_count += 1;

                            if !messages.is_empty() {
                                let caches: FxHashMap<_, _> =
                                    ManuallyDrop::into_inner(user_interfaces)
                                        .into_iter()
                                        .map(|(id, interface)| (id, interface.into_cache()))
                                        .collect();

                                update(
                                    &mut runner,
                                    &mut runtime,
                                    &mut messages,
                                    &mut update_actions,
                                    &mut update_outputs,
                                );

                                user_interfaces = ManuallyDrop::new(build_user_interfaces(
                                    &runner,
                                    &mut window_manager,
                                    caches,
                                    &mut proxy,
                                ));
                                for (ui_id, ui) in user_interfaces.iter_mut() {
                                    if let Some(w) = window_manager.get_mut(*ui_id) {
                                        w.hit_test_index = Some(ui.hit_test_index());
                                    }
                                }

                                for action in update_actions.drain(..) {
                                    // Defer all window actions to avoid compositor
                                    // race conditions while redrawing
                                    if let Action::Window(_) = action {
                                        proxy.send_action(action);
                                        continue;
                                    }

                                    run_action(
                                        action,
                                        &runner,
                                        &proxy,
                                        &mut runtime,
                                        &mut compositor,
                                        &mut events,
                                        &mut messages,
                                        &mut clipboard,
                                        &mut control_sender,
                                        &mut user_interfaces,
                                        &mut window_manager,
                                        &mut frame_gate,
                                        &mut is_window_opening,
                                        &mut system_theme,
                                        &mut renderer_settings,
                                    );
                                }

                                for (window_id, window) in window_manager.iter_mut() {
                                    // We are already redrawing this window
                                    if window_id == id {
                                        continue;
                                    }

                                    frame_gate.invalidate(
                                        window_id,
                                        crate::frame_gate::DirtyFlags::PAINT_DIRTY,
                                    );
                                    window.request_redraw(core::window::RedrawRequest::NextFrame);
                                }

                                let Some(next_compositor) = compositor.as_mut() else {
                                    continue 'next_event;
                                };

                                current_compositor = next_compositor;
                                window = window_manager.get_mut(id).unwrap();

                                // Window scale factor changed during a redraw request
                                if logical_size != window.state.logical_size() {
                                    logical_size = window.state.logical_size();

                                    log::debug!(
                                        "Window scale factor changed during a redraw request"
                                    );

                                    let ui =
                                        user_interfaces.remove(&id).expect("Remove user interface");

                                    let layout_span = debug::layout(id);
                                    let mut ui = ui.relayout(logical_size, &mut window.renderer);
                                    window.hit_test_index = Some(ui.hit_test_index());
                                    let _ = user_interfaces.insert(id, ui);
                                    layout_span.finish();
                                }

                                interface = user_interfaces.get_mut(&id).unwrap();
                            }
                        };
                        interact_span.finish();

                        let draw_span = debug::draw(id);
                        interface.draw(
                            &mut window.renderer,
                            window.state.theme(),
                            &renderer::Style {
                                text_color: window.state.text_color(),
                            },
                            cursor,
                        );

                        // Accessibility is part of the live frame path. The
                        // interface retains the previous semantic tree and
                        // returns only incremental changes, so unchanged
                        // frames are cheap while assistive technology never
                        // depends on an application-specific callback being
                        // remembered by the event loop.
                        let semantic_changes = interface.semantics_changes(&window.renderer);
                        window.accessibility.publish(&semantic_changes);

                        let focused = interface
                            .semantic_tree()
                            .find(|entry| entry.state.contains(core::semantics::State::FOCUSED))
                            .map(|entry| &entry.id);
                        window.accessibility.focus_changed(focused);
                        draw_span.finish();

                        let frame_context = {
                            let scale_factor = window.state.viewport().scale_factor();
                            match state.damage_region() {
                                Some(core_region) => {
                                    let damage = graphics::damage::DamageRegion::from_core_region(
                                        core_region,
                                    );
                                    graphics::FrameContext::with_damage(damage, scale_factor)
                                }
                                None => graphics::FrameContext::full_present(scale_factor),
                            }
                        };

                        if let user_interface::State::Updated {
                            redraw_request,
                            input_method,
                            mouse_interaction,
                            clipboard: clipboard_requests,
                            is_animating,
                            ..
                        } = state
                        {
                            window.request_redraw(redraw_request);
                            window.request_input_method(input_method);
                            window.update_mouse(mouse_interaction);

                            if is_animating {
                                window.scheduler.request_animation();
                            } else {
                                window.scheduler.stop_animating();
                            }

                            run_clipboard(&mut proxy, &mut clipboard, clipboard_requests, id);
                        }

                        runtime.broadcast(subscription::Event::Interaction {
                            window: id,
                            event: redraw_event,
                            status: core::event::Status::Ignored,
                        });

                        window.draw_preedit();

                        window.scheduler.submit(Instant::now());

                        let present_span = debug::present(id);
                        // Diagnostic (see `future.md`, Ceiling 8 / H0):
                        // brackets the actual GPU present call, the last
                        // unlogged step between `RedrawRequested` and the
                        // measurement marker.
                        log::debug!("Calling compositor present for id `{id:?}`");
                        match current_compositor.present_with_damage(
                            &mut window.renderer,
                            &mut window.surface,
                            window.state.viewport(),
                            window.state.background_color(),
                            &frame_context,
                            || window.raw.pre_present_notify(),
                        ) {
                            Ok(()) => {
                                present_span.finish();
                                log::debug!("compositor present Ok for id `{id:?}`");
                                #[cfg(all(feature = "measurement", not(target_arch = "wasm32")))]
                                measurement::first_present();

                                // No real async presentation-feedback
                                // source is wired yet (see
                                // `docs/architecture/frame-scheduling.md`),
                                // so a successful synchronous `present`
                                // is treated as its own feedback,
                                // recovering the scheduler immediately
                                // rather than waiting out
                                // `FEEDBACK_TIMEOUT` for no reason.
                                window.scheduler.feedback_received();
                                frame_gate.on_frame_presented(id);
                                if window.scheduler.should_produce_frame(Instant::now()) {
                                    window.raw.request_redraw();
                                }
                            }
                            Err(error) => match error {
                                compositor::SurfaceError::OutOfMemory => {
                                    // This is an unrecoverable error.
                                    panic!("{error:?}");
                                }
                                compositor::SurfaceError::Outdated
                                | compositor::SurfaceError::Lost => {
                                    present_span.finish();
                                    window.scheduler.feedback_received();

                                    // Reconfigure surface and try redrawing
                                    let physical_size = window.state.physical_size();

                                    if error == compositor::SurfaceError::Lost {
                                        window.surface = current_compositor.create_surface(
                                            window.raw.clone(),
                                            physical_size.width,
                                            physical_size.height,
                                        );
                                    } else {
                                        current_compositor.configure_surface(
                                            &mut window.surface,
                                            physical_size.width,
                                            physical_size.height,
                                        );
                                    }

                                    window.request_redraw(core::window::RedrawRequest::NextFrame);
                                }
                                compositor::SurfaceError::Occluded => {
                                    present_span.finish();

                                    // The compositor backend itself
                                    // observed occlusion (distinct from
                                    // winit's own `WindowEvent::Occluded`,
                                    // which may not fire on every
                                    // platform): stop scheduling here
                                    // too, so both signals lead to the
                                    // same suspended state.
                                    window.set_occluded(true);
                                    let _ = frame_gate.handle_event(
                                        id,
                                        crate::frame_gate::PlatformEvent::OcclusionChanged(true),
                                    );
                                }
                                _ => {
                                    present_span.finish();
                                    window.scheduler.feedback_received();

                                    log::warn!("Error {error:?} when presenting surface.");

                                    // Try rendering all windows again next frame.
                                    for (_id, window) in window_manager.iter_mut() {
                                        window
                                            .request_redraw(core::window::RedrawRequest::NextFrame);
                                    }
                                }
                            },
                        }
                    }
                    event::Event::WindowEvent {
                        event: window_event,
                        window_id,
                    } => {
                        // Diagnostic (see `future.md`, Ceiling 8 / H0): the
                        // only way to tell "no window events are arriving
                        // at all" apart from "an event arrived but was
                        // ignored" without a debugger attached.
                        log::trace!("WindowEvent for id `{window_id:?}`: {window_event:?}");

                        if !is_daemon
                            && matches!(window_event, winit::event::WindowEvent::Destroyed)
                            && !is_window_opening
                            && window_manager.is_empty()
                        {
                            control_sender
                                .start_send(Control::Exit)
                                .expect("Send control action");

                            continue;
                        }

                        let Some((id, window)) = window_manager.get_mut_alias(window_id) else {
                            continue;
                        };

                        match &window_event {
                            winit::event::WindowEvent::Resized(size) => {
                                let zero_sized = size.width == 0 || size.height == 0;
                                window.set_zero_sized(zero_sized);
                                let _ = frame_gate.handle_event(
                                    id,
                                    crate::frame_gate::PlatformEvent::SurfaceSizeChanged {
                                        zero_sized,
                                    },
                                );
                                frame_gate.invalidate(
                                    id,
                                    crate::frame_gate::DirtyFlags::LAYOUT_DIRTY
                                        | crate::frame_gate::DirtyFlags::PAINT_DIRTY,
                                );
                                window.request_redraw(core::window::RedrawRequest::NextFrame);
                            }
                            winit::event::WindowEvent::Focused(focused) => {
                                let _ = frame_gate.handle_event(
                                    id,
                                    crate::frame_gate::PlatformEvent::FocusChanged(*focused),
                                );
                                window.request_redraw(core::window::RedrawRequest::NextFrame);
                            }
                            winit::event::WindowEvent::Occluded(true) => {
                                window.set_occluded(true);
                                let _ = frame_gate.handle_event(
                                    id,
                                    crate::frame_gate::PlatformEvent::OcclusionChanged(true),
                                );
                            }
                            winit::event::WindowEvent::Occluded(false) => {
                                window.set_occluded(false);
                                let _ = frame_gate.handle_event(
                                    id,
                                    crate::frame_gate::PlatformEvent::OcclusionChanged(false),
                                );
                                window.request_redraw(core::window::RedrawRequest::NextFrame);
                            }
                            winit::event::WindowEvent::ScaleFactorChanged {
                                scale_factor, ..
                            } => {
                                let _ = frame_gate.handle_event(
                                    id,
                                    crate::frame_gate::PlatformEvent::ScaleFactorChanged(
                                        *scale_factor as f32,
                                    ),
                                );
                                window.request_redraw(core::window::RedrawRequest::NextFrame);
                            }
                            winit::event::WindowEvent::CursorMoved { position, .. } => {
                                let _ = frame_gate.handle_event(
                                    id,
                                    crate::frame_gate::PlatformEvent::CursorMoved {
                                        x: position.x as f32,
                                        y: position.y as f32,
                                    },
                                );
                            }
                            winit::event::WindowEvent::ThemeChanged(theme) => {
                                let _ = frame_gate.handle_event(
                                    id,
                                    crate::frame_gate::PlatformEvent::ThemeChanged,
                                );
                                window.request_redraw(core::window::RedrawRequest::NextFrame);
                                let mode = conversion::theme_mode(*theme);

                                if mode != system_theme {
                                    system_theme = mode;

                                    runtime
                                        .broadcast(subscription::Event::SystemThemeChanged(mode));
                                }
                            }
                            _ => {}
                        }

                        if matches!(window_event, winit::event::WindowEvent::CloseRequested)
                            && window.exit_on_close_request
                        {
                            run_action(
                                Action::Window(runtime::window::Action::Close(id)),
                                &runner,
                                &proxy,
                                &mut runtime,
                                &mut compositor,
                                &mut events,
                                &mut messages,
                                &mut clipboard,
                                &mut control_sender,
                                &mut user_interfaces,
                                &mut window_manager,
                                &mut frame_gate,
                                &mut is_window_opening,
                                &mut system_theme,
                                &mut renderer_settings,
                            );
                        } else {
                            window.state.update(&runner, &window.raw, &window_event);

                            if let Some(event) = conversion::window_event(
                                window_event,
                                window.state.scale_factor(),
                                window.state.modifiers(),
                            ) {
                                events.push(id, event);
                            }
                        }
                    }
                    event::Event::AboutToWait => {
                        if actions > 0 {
                            proxy.free_slots(actions);
                            actions = 0;
                        }

                        if events.is_empty()
                            && messages.is_empty()
                            && window_manager.is_idle()
                            && frame_gate.is_idle()
                        {
                            continue;
                        }

                        let mut uis_stale = false;

                        for (id, window) in window_manager.iter_mut() {
                            let interact_span = debug::interact(id);
                            let Some(window_events) = events
                                .windows
                                .get_mut(&id)
                                .filter(|window_events| !window_events.is_empty())
                            else {
                                continue;
                            };

                            let interface =
                                user_interfaces.get_mut(&id).expect("Get user interface");
                            let cursor = window.state.cursor();
                            let now = Instant::now();

                            for event in window_events.iter() {
                                let path =
                                    event
                                        .to_pointer(cursor.position(), now)
                                        .and_then(|pointer| {
                                            if pointer.phase == crate::core::pointer::Phase::Down {
                                                window.hit_test_index.as_mut()?.hit_test(
                                                    &mut interface.visit(),
                                                    pointer.position,
                                                )
                                            } else {
                                                None
                                            }
                                        });

                                let _ = interface.route_pointer(event, cursor, now, || {
                                    path.map(|p| (p, taim_gesture::arena::Arena::new()))
                                });
                            }

                            let (ui_state, statuses) = interface.update(
                                &window.raw,
                                &window.waker,
                                window_events,
                                cursor,
                                &mut window.renderer,
                                &mut messages,
                            );

                            #[cfg(feature = "unconditional-rendering")]
                            window.request_redraw(window::RedrawRequest::NextFrame);

                            match ui_state {
                                user_interface::State::Updated {
                                    redraw_request: _redraw_request,
                                    mouse_interaction,
                                    clipboard: clipboard_requests,
                                    is_animating,
                                    ..
                                } => {
                                    window.update_mouse(mouse_interaction);

                                    #[cfg(not(feature = "unconditional-rendering"))]
                                    window.request_redraw(_redraw_request);

                                    if is_animating {
                                        window.scheduler.request_animation();
                                        window.raw.request_redraw();
                                    } else {
                                        window.scheduler.stop_animating();
                                    }

                                    run_clipboard(
                                        &mut proxy,
                                        &mut clipboard,
                                        clipboard_requests,
                                        id,
                                    );
                                }
                                user_interface::State::Outdated => {
                                    uis_stale = true;
                                }
                            }

                            for (event, status) in window_events.drain(..).zip(statuses) {
                                runtime.broadcast(subscription::Event::Interaction {
                                    window: id,
                                    event,
                                    status,
                                });
                            }

                            interact_span.finish();
                        }

                        for (id, window_events) in &mut events.windows {
                            for event in window_events.drain(..) {
                                runtime.broadcast(subscription::Event::Interaction {
                                    window: *id,
                                    event,
                                    status: core::event::Status::Ignored,
                                });
                            }
                        }
                        events.finish_batch();
                        events.retain_windows(|id| window_manager.get(id).is_some());

                        if !messages.is_empty() || uis_stale {
                            let cached_interfaces: FxHashMap<_, _> =
                                ManuallyDrop::into_inner(user_interfaces)
                                    .into_iter()
                                    .map(|(id, ui)| (id, ui.into_cache()))
                                    .collect();

                            update(
                                &mut runner,
                                &mut runtime,
                                &mut messages,
                                &mut update_actions,
                                &mut update_outputs,
                            );

                            user_interfaces = ManuallyDrop::new(build_user_interfaces(
                                &runner,
                                &mut window_manager,
                                cached_interfaces,
                                &mut proxy,
                            ));
                            for (ui_id, ui) in user_interfaces.iter_mut() {
                                if let Some(w) = window_manager.get_mut(*ui_id) {
                                    w.hit_test_index = Some(ui.hit_test_index());
                                }
                            }

                            for action in update_actions.drain(..) {
                                run_action(
                                    action,
                                    &runner,
                                    &proxy,
                                    &mut runtime,
                                    &mut compositor,
                                    &mut events,
                                    &mut messages,
                                    &mut clipboard,
                                    &mut control_sender,
                                    &mut user_interfaces,
                                    &mut window_manager,
                                    &mut frame_gate,
                                    &mut is_window_opening,
                                    &mut system_theme,
                                    &mut renderer_settings,
                                );
                            }

                            for (window_id, window) in window_manager.iter_mut() {
                                frame_gate.invalidate(
                                    window_id,
                                    crate::frame_gate::DirtyFlags::PAINT_DIRTY,
                                );
                                window.request_redraw(core::window::RedrawRequest::NextFrame);
                            }
                        }

                        if frame_gate.has_immediate_work() {
                            let _ =
                                control_sender.start_send(Control::ChangeFlow(ControlFlow::Poll));
                        } else if let Some(redraw_at) = window_manager.redraw_at() {
                            let _ = control_sender
                                .start_send(Control::ChangeFlow(ControlFlow::WaitUntil(redraw_at)));
                        } else {
                            let _ =
                                control_sender.start_send(Control::ChangeFlow(ControlFlow::Wait));
                        }
                    }
                    _ => {}
                }
            }
            Event::Exit => break,
        }
    }

    let _ = ManuallyDrop::into_inner(user_interfaces);
}

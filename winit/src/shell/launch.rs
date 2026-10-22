//! Starting an application: event loop, runtime, engine, host.
use super::engine::{self, Setup};
use super::host::Host;
use crate::core::{backend, renderer, theme};
use crate::debug;
use crate::futures::futures::channel::{mpsc, oneshot};
use crate::futures::{Executor, Runtime, subscription};
use crate::program::{self, App, Program};
use crate::runtime::{self, Action, Task};
use crate::{Error, Proxy};

/// Runs a compatibility [`Program`] on the TAIM engine.
///
/// The program is adapted into an [`App`] and driven exactly like a native
/// one; there is no separate legacy event loop.
pub fn run<P>(program: P) -> Result<(), Error>
where
    P: Program + 'static,
    P::Theme: theme::Base,
{
    run_app(program::compat::ProgramAdapter::new(program))
}

/// Runs a native TAIM [`App`] until it exits.
pub fn run_app<A>(app: A) -> Result<(), Error>
where
    A: App + 'static,
    A::Theme: theme::Base,
{
    let boot_span = debug::boot();

    let config = app.config();
    let settings = config.settings;
    let first_window = config.window;
    let is_daemon = first_window.is_none();

    let event_loop = winit::event_loop::EventLoop::with_user_event()
        .build()
        .expect("the platform refused to create an event loop");

    let (proxy, forwarder) = Proxy::new(event_loop.create_proxy());

    #[cfg(feature = "debug")]
    {
        let proxy = proxy.clone();
        debug::on_hotpatch(move || proxy.send_action(Action::Reload));
    }

    let mut runtime = {
        let executor = A::Executor::new().map_err(Error::ExecutorCreationFailed)?;
        executor.spawn(forwarder);

        Runtime::new(executor, proxy.clone())
    };

    let (runner, boot_task) = runtime.enter(|| program::AppRunner::new(app));

    // An application (not a daemon) opens its first window before anything
    // its boot task asked for, so that task can rely on a window existing.
    let boot_task = match first_window {
        Some(window_settings) => {
            let (_, open) = runtime::window::open(window_settings);
            let mut boot_task = Some(boot_task);

            open.then(move |_| boot_task.take().unwrap_or_else(Task::none))
        }
        None => boot_task,
    };

    if let Some(stream) = runtime::task::into_stream(boot_task) {
        let _ = runtime.run(stream);
    }

    runtime.track(subscription::into_recipes(
        runtime.enter(|| runner.subscription().map(Action::Output)),
    ));

    let (signal_sender, signals) = mpsc::unbounded();
    let (request_sender, requests) = mpsc::unbounded();
    let (theme_sender, first_theme) = oneshot::channel();

    let engine = engine::drive(
        runner,
        runtime,
        proxy,
        signals,
        request_sender,
        first_theme,
        Setup {
            is_daemon,
            display: event_loop.owned_display_handle(),
            backend_settings: backend::Settings::from(&settings),
            renderer_settings: renderer::Settings::from(&settings),
            fonts: settings.fonts,
        },
    );

    let host = Host::new(engine, signal_sender, requests, settings.id, theme_sender);

    boot_span.finish();

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut host = host;
        let _ = event_loop.run_app(&mut host);

        host.into_failure().map_or(Ok(()), Err)
    }

    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        event_loop.spawn_app(host);

        Ok(())
    }
}

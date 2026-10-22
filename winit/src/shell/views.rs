//! Per-window user interfaces and how they are (re)built from the application.
use crate::Proxy;
use crate::core::{self, Renderer as _, Size, theme, window};
use crate::debug;
use crate::graphics::Compositor;
use crate::program::{self, App, AppRunner};
use crate::runtime::Action;
use crate::runtime::user_interface::{self, UserInterface};

use rustc_hash::FxHashMap;

type Interface<'a, A> =
    UserInterface<'a, <A as App>::Message, <A as App>::Theme, <A as App>::Renderer>;

/// The live interface of every open window.
///
/// Interfaces borrow the application, so before the application can be
/// updated they are [`recycle`](Views::recycle)d into layout caches and built
/// again afterwards.
pub(super) struct Views<'a, A: App> {
    interfaces: FxHashMap<window::Id, Interface<'a, A>>,
}

impl<'a, A: App> Views<'a, A> {
    pub(super) fn new() -> Self {
        Self {
            interfaces: FxHashMap::default(),
        }
    }

    pub(super) fn get_mut(&mut self, id: window::Id) -> Option<&mut Interface<'a, A>> {
        self.interfaces.get_mut(&id)
    }

    pub(super) fn insert(&mut self, id: window::Id, interface: Interface<'a, A>) {
        let _ = self.interfaces.insert(id, interface);
    }

    pub(super) fn remove(&mut self, id: window::Id) -> Option<Interface<'a, A>> {
        self.interfaces.remove(&id)
    }

    pub(super) fn iter_mut(&mut self) -> impl Iterator<Item = (window::Id, &mut Interface<'a, A>)> {
        self.interfaces
            .iter_mut()
            .map(|(id, interface)| (*id, interface))
    }

    /// Releases the borrow of the application, keeping what layout can reuse.
    pub(super) fn recycle(self) -> FxHashMap<window::Id, user_interface::Cache> {
        self.interfaces
            .into_iter()
            .map(|(id, interface)| (id, interface.into_cache()))
            .collect()
    }

    /// Replaces the interface of `id` with `rebuild(old)`, if it exists.
    pub(super) fn replace(
        &mut self,
        id: window::Id,
        rebuild: impl FnOnce(Interface<'a, A>) -> Interface<'a, A>,
    ) -> Option<&mut Interface<'a, A>> {
        let interface = rebuild(self.interfaces.remove(&id)?);
        let slot = self.interfaces.entry(id).insert_entry(interface);

        Some(slot.into_mut())
    }
}

/// Views and lays out the application for one window.
pub(super) fn build<'a, A: App>(
    runner: &'a AppRunner<A>,
    cache: user_interface::Cache,
    renderer: &mut A::Renderer,
    size: Size,
    id: window::Id,
    scale_factor: f32,
) -> Interface<'a, A>
where
    A::Theme: theme::Base,
{
    let view_span = debug::view(id);
    let element = runner.view(&program::WindowContext::new(id, scale_factor));
    view_span.finish();

    let layout_span = debug::layout(id);
    let interface = UserInterface::build(element, size, cache, renderer);
    layout_span.finish();

    interface
}

/// Re-synchronizes every window with the application, then rebuilds the
/// interface of each window that has a cache in `caches`.
///
/// Windows whose logical size changed while synchronizing are sent a
/// `Resized` event through `proxy`.
pub fn build_user_interfaces<'a, A: App, C>(
    runner: &'a AppRunner<A>,
    windows: &mut crate::window::Manager<A, C>,
    mut caches: FxHashMap<window::Id, user_interface::Cache>,
    proxy: &mut Proxy<A::Message>,
) -> FxHashMap<window::Id, Interface<'a, A>>
where
    C: Compositor<Renderer = A::Renderer>,
    A::Theme: theme::Base,
{
    for (id, window) in windows.iter_mut() {
        let before = window.state.logical_size();
        window.state.synchronize(runner, id, &window.raw);
        let after = window.state.logical_size();

        if before != after {
            proxy.send_action(Action::Event {
                window: id,
                event: core::Event::Window(window::Event::Resized(after)),
            });
        }

        #[cfg(feature = "hinting")]
        window.renderer.hint(window.state.scale_factor());

        // Synchronizing re-reads the theme; the light belongs to the theme,
        // so it must follow here or a scheme switch would leave it behind.
        window
            .renderer
            .set_light(theme::Base::light(window.state.theme()));
    }

    debug::theme_changed(|| {
        windows
            .first()
            .and_then(|window| theme::Base::seed(window.state.theme()))
    });

    caches
        .drain()
        .filter_map(|(id, cache)| {
            let window = windows.get_mut(id)?;
            let size = window.state.logical_size();
            let scale_factor = window.state.scale_factor();

            Some((
                id,
                build(runner, cache, &mut window.renderer, size, id, scale_factor),
            ))
        })
        .collect()
}

impl<'a, A: App> Views<'a, A>
where
    A::Theme: theme::Base,
{
    /// [`build_user_interfaces`], keeping each window's hit-test index in step.
    pub(super) fn rebuild<C>(
        runner: &'a AppRunner<A>,
        windows: &mut crate::window::Manager<A, C>,
        caches: FxHashMap<window::Id, user_interface::Cache>,
        proxy: &mut Proxy<A::Message>,
    ) -> Self
    where
        C: Compositor<Renderer = A::Renderer>,
    {
        let mut interfaces = build_user_interfaces(runner, windows, caches, proxy);

        for (id, interface) in &mut interfaces {
            if let Some(window) = windows.get_mut(*id) {
                window.hit_test_index = Some(interface.hit_test_index());
            }
        }

        Self { interfaces }
    }
}

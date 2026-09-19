//! Window ownership, platform-id routing, and compositor resources.

use super::{State, Window};
use crate::Proxy;
use crate::core;
use crate::core::mouse;
use crate::core::renderer;
use crate::core::shell;
use crate::core::theme;
use crate::core::time::Instant;
use crate::core::window::Id;
use crate::graphics::Compositor;
use crate::program::{App, AppRunner};
use crate::runtime::frame_scheduler::Scheduler;

use rustc_hash::FxHashMap;
use std::collections::BTreeMap;
use std::sync::Arc;
use winit::monitor::MonitorHandle;

/// Owns live windows and maps platform identifiers to stable TAIM identifiers.
pub struct Manager<A, C>
where
    A: App,
    C: Compositor<Renderer = A::Renderer>,
    A::Theme: theme::Base,
{
    aliases: FxHashMap<winit::window::WindowId, Id>,
    entries: BTreeMap<Id, Window<A, C>>,
}

impl<A, C> Manager<A, C>
where
    A: App,
    C: Compositor<Renderer = A::Renderer>,
    A::Theme: theme::Base,
{
    pub fn new() -> Self {
        Self {
            aliases: FxHashMap::default(),
            entries: BTreeMap::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert(
        &mut self,
        id: Id,
        window: Arc<winit::window::Window>,
        runner: &AppRunner<A>,
        compositor: &mut C,
        proxy: Proxy<A::Message>,
        renderer_settings: renderer::Settings,
        exit_on_close_request: bool,
        system_theme: theme::Mode,
    ) -> &mut Window<A, C> {
        // Maintain a strict bijection when callers reuse either identifier.
        if let Some(previous) = self.entries.remove(&id) {
            let _ = self.aliases.remove(&previous.raw.id());
        }
        if let Some(previous_id) = self.aliases.remove(&window.id())
            && previous_id != id
        {
            let _ = self.entries.remove(&previous_id);
        }

        let state = State::new(runner, id, &window, system_theme);
        let surface_size = state.physical_size();
        let surface_version = state.surface_version();
        let surface =
            compositor.create_surface(window.clone(), surface_size.width, surface_size.height);
        let renderer = compositor.create_renderer(renderer_settings);
        let is_zero_sized = surface_size.width == 0 || surface_size.height == 0;
        let mut scheduler = Scheduler::new();
        if is_zero_sized {
            scheduler.suspend();
        }

        let waker = shell::Waker::new_tagged(move |reactive| {
            proxy.send_action(taim_runtime::Action::Event {
                window: id,
                event: reactive.map_or(core::Event::Waken, core::Event::Reactive),
            });
        });

        let accessibility =
            taim_accessibility::Session::new(taim_accessibility::atspi::adapter::Adapter::new());

        #[cfg(feature = "accessibility")]
        let accessibility_bridge = taim_accessibility::atspi::bridge::Bridge::spawn(
            accessibility.adapter().store().clone(),
            accessibility.adapter().events().clone(),
            window.title(),
        );

        let platform_id = window.id();
        let _ = self.aliases.insert(platform_id, id);
        let _ = self.entries.insert(
            id,
            Window {
                raw: window,
                waker,
                state,
                exit_on_close_request,
                surface,
                surface_version,
                renderer,
                mouse_interaction: mouse::Interaction::None,
                hit_test_index: None,
                scheduler,
                is_occluded: false,
                is_zero_sized,
                accessibility,
                #[cfg(feature = "accessibility")]
                _accessibility_bridge: accessibility_bridge,
                preedit: None,
                ime_state: None,
            },
        );

        self.entries
            .get_mut(&id)
            .expect("the window inserted immediately above must exist")
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_idle(&self) -> bool {
        self.entries
            .values()
            .all(|window| window.scheduler.is_idle())
    }

    pub fn redraw_at(&self) -> Option<Instant> {
        self.entries
            .values()
            .filter_map(|window| window.scheduler.deadline())
            .min()
    }

    pub fn first(&self) -> Option<&Window<A, C>> {
        self.entries.first_key_value().map(|(_, window)| window)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Id, &mut Window<A, C>)> {
        self.entries.iter_mut().map(|(id, window)| (*id, window))
    }

    pub fn get(&self, id: Id) -> Option<&Window<A, C>> {
        self.entries.get(&id)
    }

    pub fn get_mut(&mut self, id: Id) -> Option<&mut Window<A, C>> {
        self.entries.get_mut(&id)
    }

    pub fn get_mut_alias(
        &mut self,
        platform_id: winit::window::WindowId,
    ) -> Option<(Id, &mut Window<A, C>)> {
        let id = self.aliases.get(&platform_id).copied()?;
        Some((id, self.entries.get_mut(&id)?))
    }

    pub fn get_alias(&self, id: winit::window::WindowId) -> Option<Id> {
        self.aliases.get(&id).copied()
    }

    pub fn last_monitor(&self) -> Option<MonitorHandle> {
        self.entries.values().last()?.raw.current_monitor()
    }

    pub fn remove(&mut self, id: Id) -> Option<Window<A, C>> {
        let window = self.entries.remove(&id)?;
        let _ = self.aliases.remove(&window.raw.id());
        Some(window)
    }

    pub fn replace_with(&mut self, mut f: impl FnMut(Window<A, C>) -> Window<A, C>) {
        self.entries = std::mem::take(&mut self.entries)
            .into_iter()
            .map(|(id, window)| (id, f(window)))
            .collect();
    }
}

impl<A, C> Default for Manager<A, C>
where
    A: App,
    C: Compositor<Renderer = A::Renderer>,
    A::Theme: theme::Base,
{
    fn default() -> Self {
        Self::new()
    }
}

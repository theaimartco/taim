//! The set of open windows, addressable by TAIM id or by platform id.
use super::Window;
use crate::core::theme;
use crate::core::time::Instant;
use crate::core::window::Id;
use crate::graphics::Compositor;
use crate::program::App;

use rustc_hash::FxHashMap;
use std::collections::BTreeMap;
use winit::monitor::MonitorHandle;
use winit::window::WindowId as PlatformId;

/// Open windows in creation order (TAIM ids are monotonic), plus the
/// platform-id index the event loop routes by.
///
/// The two maps are kept a bijection: re-inserting either id evicts whatever
/// it pointed to before.
pub struct Manager<A, C>
where
    A: App,
    C: Compositor<Renderer = A::Renderer>,
    A::Theme: theme::Base,
{
    windows: BTreeMap<Id, Window<A, C>>,
    by_platform: FxHashMap<PlatformId, Id>,
}

impl<A, C> Manager<A, C>
where
    A: App,
    C: Compositor<Renderer = A::Renderer>,
    A::Theme: theme::Base,
{
    pub fn new() -> Self {
        Self {
            windows: BTreeMap::new(),
            by_platform: FxHashMap::default(),
        }
    }

    /// Adds `window` under `id` and returns it.
    pub fn insert(&mut self, id: Id, window: Window<A, C>) -> &mut Window<A, C> {
        let platform_id = window.raw.id();

        if let Some(evicted) = self.windows.remove(&id) {
            let _ = self.by_platform.remove(&evicted.raw.id());
        }

        if let Some(stale) = self.by_platform.insert(platform_id, id)
            && stale != id
        {
            let _ = self.windows.remove(&stale);
        }

        self.windows.entry(id).insert_entry(window).into_mut()
    }

    pub fn remove(&mut self, id: Id) -> Option<Window<A, C>> {
        let window = self.windows.remove(&id)?;
        let _ = self.by_platform.remove(&window.raw.id());

        Some(window)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    /// Whether no window's frame clock has anything scheduled.
    pub fn is_idle(&self) -> bool {
        self.windows
            .values()
            .all(|window| window.scheduler.is_idle())
    }

    /// The earliest frame deadline across all windows.
    pub fn redraw_at(&self) -> Option<Instant> {
        self.windows
            .values()
            .filter_map(|window| window.scheduler.deadline())
            .min()
    }

    /// The oldest window.
    pub fn first(&self) -> Option<&Window<A, C>> {
        self.windows.values().next()
    }

    /// The monitor of the newest window, where new windows should open.
    pub fn last_monitor(&self) -> Option<MonitorHandle> {
        self.windows.values().next_back()?.raw.current_monitor()
    }

    pub fn get(&self, id: Id) -> Option<&Window<A, C>> {
        self.windows.get(&id)
    }

    pub fn get_mut(&mut self, id: Id) -> Option<&mut Window<A, C>> {
        self.windows.get_mut(&id)
    }

    /// The TAIM id of a platform window.
    pub fn get_alias(&self, platform_id: PlatformId) -> Option<Id> {
        self.by_platform.get(&platform_id).copied()
    }

    /// Looks a window up by its platform id.
    pub fn get_mut_alias(&mut self, platform_id: PlatformId) -> Option<(Id, &mut Window<A, C>)> {
        let id = self.get_alias(platform_id)?;

        Some((id, self.windows.get_mut(&id)?))
    }

    /// Windows oldest first.
    pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = (Id, &mut Window<A, C>)> {
        self.windows.iter_mut().map(|(id, window)| (*id, window))
    }

    /// Rebuilds every window by value (for example onto a new GPU device).
    pub fn replace_with(&mut self, mut rebuild: impl FnMut(Window<A, C>) -> Window<A, C>) {
        self.windows = std::mem::take(&mut self.windows)
            .into_iter()
            .map(|(id, window)| (id, rebuild(window)))
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

//! Retained scene mounting for signal-driven applications.
use crate::{Settings, window};
use std::marker::PhantomData;

/// Creates a retained reactive application definition.
pub fn reactive_app<State, Boot, Mount>(
    boot: Boot,
    mount: Mount,
) -> ReactiveApplication<State, Boot, Mount>
where
    State: 'static,
    Boot: FnOnce() -> State,
    Mount: Fn(&State, &mut crate::scene::SceneArena) -> crate::scene::NodeId + 'static,
{
    ReactiveApplication {
        boot,
        mount,
        settings: Settings::default(),
        window: window::Settings::default(),
        state: PhantomData,
    }
}

/// A configured retained application ready to be mounted.
#[derive(Debug)]
pub struct ReactiveApplication<State, Boot, Mount> {
    boot: Boot,
    mount: Mount,
    settings: Settings,
    window: window::Settings,
    state: PhantomData<fn() -> State>,
}

impl<State, Boot, Mount> ReactiveApplication<State, Boot, Mount>
where
    State: 'static,
    Boot: FnOnce() -> State,
    Mount: Fn(&State, &mut crate::scene::SceneArena) -> crate::scene::NodeId + 'static,
{
    /// Replaces initial window configuration.
    pub fn window(mut self, window: window::Settings) -> Self {
        self.window = window;
        self
    }

    /// Replaces renderer/runtime configuration.
    pub fn settings(mut self, settings: Settings) -> Self {
        self.settings = settings;
        self
    }

    /// Consumes this definition and mounts its retained scene once.
    pub fn build_ui(self) -> (State, crate::runtime::ReactiveUi) {
        let state = (self.boot)();
        let mut arena = crate::scene::SceneArena::new();
        let root = (self.mount)(&state, &mut arena);
        arena.set_root(root);
        (state, crate::runtime::ReactiveUi::new(arena))
    }

    /// Returns configured framework settings.
    pub fn framework_settings(&self) -> &Settings {
        &self.settings
    }

    /// Returns configured initial window settings.
    pub fn window_settings(&self) -> &window::Settings {
        &self.window
    }
}

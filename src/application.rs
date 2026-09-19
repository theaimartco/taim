//! Functional and retained application builders.
mod builder;
mod logic;
mod reactive;

pub mod timed;

pub use logic::{BootFn, IntoBoot, ThemeFn, TitleFn, UpdateFn, ViewFn};
pub use reactive::{ReactiveApplication, reactive_app};
pub use timed::timed;

use crate::program::{self, Program};
use crate::{Element, Preset, Settings, Task, theme, window};
use std::marker::PhantomData;

/// Creates a functional application from boot, update, and view logic.
pub fn application<State, Message, Theme, Renderer>(
    boot: impl BootFn<State, Message>,
    update: impl UpdateFn<State, Message>,
    view: impl for<'a> ViewFn<'a, State, Message, Theme, Renderer>,
) -> Application<impl Program<State = State, Message = Message, Theme = Theme>>
where
    State: 'static,
    Message: Send + 'static,
    Theme: theme::Base,
    Renderer: program::Renderer,
{
    Application {
        raw: Functional {
            boot,
            update,
            view,
            types: PhantomData,
        },
        settings: Settings::default(),
        window: window::Settings::default(),
        presets: Vec::new(),
    }
}

/// A functional application plus its startup configuration.
#[derive(Debug)]
pub struct Application<P: Program> {
    pub(super) raw: P,
    pub(super) settings: Settings,
    pub(super) window: window::Settings,
    pub(super) presets: Vec<Preset<P::State, P::Message>>,
}

struct Functional<State, Message, Theme, Renderer, Boot, Update, View> {
    boot: Boot,
    update: Update,
    view: View,
    types: PhantomData<fn() -> (State, Message, Theme, Renderer)>,
}

impl<State, Message, Theme, Renderer, Boot, Update, View> Program
    for Functional<State, Message, Theme, Renderer, Boot, Update, View>
where
    Message: Send + 'static,
    Theme: theme::Base,
    Renderer: program::Renderer,
    Boot: BootFn<State, Message>,
    Update: UpdateFn<State, Message>,
    View: for<'a> ViewFn<'a, State, Message, Theme, Renderer>,
{
    type State = State;
    type Message = Message;
    type Theme = Theme;
    type Renderer = Renderer;
    type Executor = taim_futures::backend::default::Executor;

    fn name() -> &'static str {
        std::any::type_name::<State>()
            .rsplit("::")
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or("TAIM Application")
    }

    fn boot(&self) -> (State, Task<Message>) {
        self.boot.boot()
    }

    fn update(&self, state: &mut State, message: Message) -> Task<Message> {
        self.update.update(state, message)
    }

    fn view<'a>(
        &self,
        state: &'a State,
        _window: window::Id,
    ) -> Element<'a, Message, Theme, Renderer> {
        self.view.view(state)
    }

    fn settings(&self) -> Settings {
        Settings::default()
    }

    fn window(&self) -> Option<window::Settings> {
        Some(window::Settings::default())
    }
}

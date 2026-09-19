//! Background application runner with on-demand windows.
mod builder;
mod logic;

pub use logic::{ThemeFn, TitleFn, ViewFn};

use crate::application::{BootFn, UpdateFn};
use crate::program::{self, Program};
use crate::{Element, Preset, Settings, Task, theme, window};
use std::marker::PhantomData;

/// Creates a daemon that stays alive independently of its open windows.
pub fn daemon<State, Message, Theme, Renderer>(
    boot: impl BootFn<State, Message>,
    update: impl UpdateFn<State, Message>,
    view: impl for<'a> ViewFn<'a, State, Message, Theme, Renderer>,
) -> Daemon<impl Program<State = State, Message = Message, Theme = Theme>>
where
    State: 'static,
    Message: Send + 'static,
    Theme: theme::Base,
    Renderer: program::Renderer,
{
    Daemon {
        raw: Functional {
            boot,
            update,
            view,
            types: PhantomData,
        },
        settings: Settings::default(),
        presets: Vec::new(),
    }
}

/// A background program plus renderer and boot configuration.
#[derive(Debug)]
pub struct Daemon<P: Program> {
    pub(super) raw: P,
    pub(super) settings: Settings,
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
            .unwrap_or("TAIM Daemon")
    }

    fn settings(&self) -> Settings {
        Settings::default()
    }

    fn window(&self) -> Option<window::Settings> {
        None
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
        window: window::Id,
    ) -> Element<'a, Message, Theme, Renderer> {
        self.view.view(state, window)
    }
}

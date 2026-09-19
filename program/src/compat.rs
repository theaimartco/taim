//! Legacy compatibility layer for `Program`-based applications.
//!
//! Adapts legacy [`Program`] instances into the native TAIM [`App`] architecture.
//! New applications should implement [`App`] directly.

use crate::Renderer;
use crate::app::{App, AppConfig, AppContext, AppRunner, RuntimeContext, ViewRoot, WindowContext};
use crate::core::theme;
use crate::core::window;
use crate::core::{Element, Settings};
use crate::futures::{Executor, Subscription};
use crate::preset::Preset;
use crate::runtime::Task;

use std::borrow::Cow;

mod decorators;

pub use decorators::{
    with_executor, with_scale_factor, with_style, with_subscription, with_theme, with_title,
};

/// An interactive, native, cross-platform, multi-windowed application (legacy contract).
///
/// New applications should implement [`App`] instead.
#[allow(missing_docs)]
pub trait Program: Sized {
    type State;
    type Message: Send + 'static;
    type Theme: theme::Base;
    type Renderer: Renderer;
    type Executor: Executor;

    fn name() -> &'static str;
    fn settings(&self) -> Settings;
    fn window(&self) -> Option<window::Settings>;
    fn boot(&self) -> (Self::State, Task<Self::Message>);
    fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message>;
    fn view<'a>(
        &self,
        state: &'a Self::State,
        window: window::Id,
    ) -> Element<'a, Self::Message, Self::Theme, Self::Renderer>;

    fn title(&self, _state: &Self::State, _window: window::Id) -> String {
        let mut title = String::with_capacity(Self::name().len() + " - Taim".len());

        for (i, part) in Self::name().split('_').enumerate() {
            if i > 0 {
                title.push(' ');
            }

            if matches!(part, "a" | "an" | "of" | "in" | "and") {
                title.push_str(part);
            } else {
                let mut characters = part.chars();
                if let Some(first) = characters.next() {
                    title.extend(first.to_uppercase());
                    title.push_str(characters.as_str());
                }
            }
        }

        title.push_str(" - Taim");
        title
    }

    fn subscription(&self, _state: &Self::State) -> Subscription<Self::Message> {
        Subscription::none()
    }

    fn theme(&self, _state: &Self::State, _window: window::Id) -> Option<Self::Theme> {
        None
    }

    fn style(&self, _state: &Self::State, theme: &Self::Theme) -> theme::Style {
        theme::Base::base(theme)
    }

    fn scale_factor(&self, _state: &Self::State, _window: window::Id) -> f32 {
        1.0
    }

    fn presets(&self) -> &[Preset<Self::State, Self::Message>] {
        &[]
    }
}

/// Compatibility adapter converting any legacy [`Program`] into a native TAIM [`App`].
#[derive(Debug, Clone, Copy)]
pub struct ProgramAdapter<P> {
    /// The underlying legacy program.
    pub program: P,
}

impl<P> ProgramAdapter<P> {
    /// Creates a new [`ProgramAdapter`].
    pub fn new(program: P) -> Self {
        Self { program }
    }
}

impl<P: Program> App for ProgramAdapter<P> {
    type State = P::State;
    type Message = P::Message;
    type Theme = P::Theme;
    type Renderer = P::Renderer;
    type Executor = P::Executor;

    fn name() -> &'static str {
        P::name()
    }

    fn config(&self) -> AppConfig {
        AppConfig {
            settings: self.program.settings(),
            window: self.program.window(),
            title: Cow::Borrowed(P::name()),
        }
    }

    fn boot(&self, _context: &mut AppContext) -> (Self::State, Task<Self::Message>) {
        self.program.boot()
    }

    fn update(
        &self,
        state: &mut Self::State,
        message: Self::Message,
        _context: &mut AppContext,
    ) -> Task<Self::Message> {
        self.program.update(state, message)
    }

    fn view<'a>(
        &self,
        state: &'a Self::State,
        window: &WindowContext,
    ) -> ViewRoot<'a, Self::Message, Self::Theme, Self::Renderer> {
        self.program.view(state, window.id)
    }

    fn title(&self, state: &Self::State, window: &WindowContext) -> String {
        self.program.title(state, window.id)
    }

    fn subscription(
        &self,
        state: &Self::State,
        _context: &RuntimeContext,
    ) -> Subscription<Self::Message> {
        self.program.subscription(state)
    }

    fn theme(&self, state: &Self::State, window: &WindowContext) -> Option<Self::Theme> {
        self.program.theme(state, window.id)
    }

    fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
        self.program.style(state, theme)
    }

    fn scale_factor(&self, state: &Self::State, window: &WindowContext) -> f32 {
        self.program.scale_factor(state, window.id)
    }

    fn is_legacy_compat() -> bool {
        true
    }
}

/// Adapter allowing a native [`App`] to be executed anywhere a legacy [`Program`] is expected.
#[derive(Debug, Clone, Copy)]
pub struct AppToProgram<A>(pub A);

impl<A: App> Program for AppToProgram<A> {
    type State = A::State;
    type Message = A::Message;
    type Theme = A::Theme;
    type Renderer = A::Renderer;
    type Executor = A::Executor;

    fn name() -> &'static str {
        A::name()
    }

    fn settings(&self) -> Settings {
        self.0.config().settings
    }

    fn window(&self) -> Option<window::Settings> {
        self.0.config().window
    }

    fn boot(&self) -> (Self::State, Task<Self::Message>) {
        let mut ctx = AppContext::new();
        self.0.boot(&mut ctx)
    }

    fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message> {
        let mut ctx = AppContext::new();
        self.0.update(state, message, &mut ctx)
    }

    fn view<'a>(
        &self,
        state: &'a Self::State,
        window: window::Id,
    ) -> Element<'a, Self::Message, Self::Theme, Self::Renderer> {
        let ctx = WindowContext::new(window, 1.0);
        self.0.view(state, &ctx)
    }

    fn title(&self, state: &Self::State, window: window::Id) -> String {
        let ctx = WindowContext::new(window, 1.0);
        self.0.title(state, &ctx)
    }

    fn subscription(&self, state: &Self::State) -> Subscription<Self::Message> {
        self.0.subscription(state, &RuntimeContext::default())
    }

    fn theme(&self, state: &Self::State, window: window::Id) -> Option<Self::Theme> {
        let ctx = WindowContext::new(window, 1.0);
        self.0.theme(state, &ctx)
    }

    fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
        self.0.style(state, theme)
    }

    fn scale_factor(&self, state: &Self::State, window: window::Id) -> f32 {
        let ctx = WindowContext::new(window, 1.0);
        self.0.scale_factor(state, &ctx)
    }
}

/// A particular instance of a running legacy [`Program`], internally driven
/// by the native [`AppRunner`].
pub struct Instance<P: Program> {
    runner: AppRunner<ProgramAdapter<P>>,
}

impl<P: Program> Instance<P> {
    /// Creates a new [`Instance`] of the given [`Program`].
    pub fn new(program: P) -> (Self, Task<P::Message>) {
        let (runner, task) = AppRunner::new(ProgramAdapter::new(program));
        (Self { runner }, task)
    }

    /// Returns the current title of the [`Instance`].
    pub fn title(&self, window: window::Id) -> String {
        let ctx = WindowContext::new(window, 1.0);
        self.runner.title(&ctx)
    }

    /// Processes the given message and updates the [`Instance`].
    pub fn update(&mut self, message: P::Message) -> Task<P::Message> {
        self.runner.update(message)
    }

    /// Produces the current widget tree of the [`Instance`].
    pub fn view(&self, window: window::Id) -> Element<'_, P::Message, P::Theme, P::Renderer> {
        let ctx = WindowContext::new(window, 1.0);
        self.runner.view(&ctx)
    }

    /// Returns the current [`Subscription`] of the [`Instance`].
    pub fn subscription(&self) -> Subscription<P::Message> {
        self.runner.subscription()
    }

    /// Returns the current theme of the [`Instance`].
    pub fn theme(&self, window: window::Id) -> Option<P::Theme> {
        let ctx = WindowContext::new(window, 1.0);
        self.runner.theme(&ctx)
    }

    /// Returns the current [`theme::Style`] of the [`Instance`].
    pub fn style(&self, theme: &P::Theme) -> theme::Style {
        self.runner.style(theme)
    }

    /// Returns the current scale factor of the [`Instance`].
    pub fn scale_factor(&self, window: window::Id) -> f32 {
        let ctx = WindowContext::new(window, 1.0);
        self.runner.scale_factor(&ctx)
    }

    /// Returns a reference to the internal program state.
    pub fn state(&self) -> &P::State {
        self.runner.state()
    }

    /// Returns a mutable reference to the internal program state.
    pub fn state_mut(&mut self) -> &mut P::State {
        self.runner.state_mut()
    }
}

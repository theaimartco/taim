//! Daemon configuration and runtime delegation.
use super::{Daemon, ThemeFn, TitleFn};
use crate::message;
use crate::program::{self, Program};
use crate::theme;
use crate::window;
use crate::{Element, Executor, Font, Preset, Result, Settings, Subscription, Task};
use std::borrow::Cow;

impl<P: Program> Daemon<P> {
    /// Starts the background-capable native runner.
    pub fn run(self) -> Result
    where
        Self: 'static,
        P::Message: message::MaybeDebug + message::MaybeClone,
    {
        Ok(crate::shell::run(self)?)
    }

    /// Replaces framework settings.
    pub fn settings(mut self, settings: Settings) -> Self {
        self.settings = settings;
        self
    }

    /// Toggles primitive antialiasing.
    pub fn antialiasing(mut self, enabled: bool) -> Self {
        self.settings.antialiasing = enabled;
        self
    }

    /// Sets the default font.
    pub fn default_font(mut self, font: Font) -> Self {
        self.settings.default_font = font;
        self
    }

    /// Adds a boot font, ignoring byte-identical duplicates.
    pub fn font(mut self, font: impl Into<Cow<'static, [u8]>>) -> Self {
        let font = font.into();
        if !self
            .settings
            .fonts
            .iter()
            .any(|loaded| loaded.as_ref() == font.as_ref())
        {
            self.settings.fonts.push(font);
        }
        self
    }

    /// Installs window-aware title logic.
    pub fn title(
        self,
        title: impl TitleFn<P::State>,
    ) -> Daemon<impl Program<State = P::State, Message = P::Message, Theme = P::Theme>> {
        Daemon {
            raw: program::with_title(self.raw, move |state, window| title.title(state, window)),
            settings: self.settings,
            presets: self.presets,
        }
    }

    /// Installs passive subscription logic.
    pub fn subscription(
        self,
        subscription: impl Fn(&P::State) -> Subscription<P::Message>,
    ) -> Daemon<impl Program<State = P::State, Message = P::Message, Theme = P::Theme>> {
        Daemon {
            raw: program::with_subscription(self.raw, subscription),
            settings: self.settings,
            presets: self.presets,
        }
    }

    /// Installs window-aware theme selection.
    pub fn theme(
        self,
        theme: impl ThemeFn<P::State, P::Theme>,
    ) -> Daemon<impl Program<State = P::State, Message = P::Message, Theme = P::Theme>> {
        Daemon {
            raw: program::with_theme(self.raw, move |state, window| theme.theme(state, window)),
            settings: self.settings,
            presets: self.presets,
        }
    }

    /// Installs dynamic application styling.
    pub fn style(
        self,
        style: impl Fn(&P::State, &P::Theme) -> theme::Style,
    ) -> Daemon<impl Program<State = P::State, Message = P::Message, Theme = P::Theme>> {
        Daemon {
            raw: program::with_style(self.raw, style),
            settings: self.settings,
            presets: self.presets,
        }
    }

    /// Installs a guarded per-window scale function.
    pub fn scale_factor(
        self,
        scale: impl Fn(&P::State, window::Id) -> f32,
    ) -> Daemon<impl Program<State = P::State, Message = P::Message, Theme = P::Theme>> {
        Daemon {
            raw: program::with_scale_factor(self.raw, move |state, window| {
                positive_or_one(scale(state, window))
            }),
            settings: self.settings,
            presets: self.presets,
        }
    }

    /// Selects a task executor.
    pub fn executor<E>(
        self,
    ) -> Daemon<impl Program<State = P::State, Message = P::Message, Theme = P::Theme>>
    where
        E: Executor,
    {
        Daemon {
            raw: program::with_executor::<P, E>(self.raw),
            settings: self.settings,
            presets: self.presets,
        }
    }

    /// Replaces deterministic boot presets.
    pub fn presets(
        mut self,
        presets: impl IntoIterator<Item = Preset<P::State, P::Message>>,
    ) -> Self {
        let iterator = presets.into_iter();
        self.presets = Vec::with_capacity(iterator.size_hint().0);
        self.presets.extend(iterator);
        self
    }
}

impl<P: Program> Program for Daemon<P> {
    type State = P::State;
    type Message = P::Message;
    type Theme = P::Theme;
    type Renderer = P::Renderer;
    type Executor = P::Executor;

    fn name() -> &'static str {
        P::name()
    }

    fn settings(&self) -> Settings {
        self.settings.clone()
    }

    fn window(&self) -> Option<window::Settings> {
        None
    }

    fn boot(&self) -> (Self::State, Task<Self::Message>) {
        self.raw.boot()
    }

    fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message> {
        taim_debug::hot(|| self.raw.update(state, message))
    }

    fn view<'a>(
        &self,
        state: &'a Self::State,
        window: window::Id,
    ) -> Element<'a, Self::Message, Self::Theme, Self::Renderer> {
        taim_debug::hot(|| self.raw.view(state, window))
    }

    fn title(&self, state: &Self::State, window: window::Id) -> String {
        taim_debug::hot(|| self.raw.title(state, window))
    }

    fn subscription(&self, state: &Self::State) -> Subscription<Self::Message> {
        taim_debug::hot(|| self.raw.subscription(state))
    }

    fn theme(&self, state: &Self::State, window: window::Id) -> Option<Self::Theme> {
        taim_debug::hot(|| self.raw.theme(state, window))
    }

    fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
        taim_debug::hot(|| self.raw.style(state, theme))
    }

    fn scale_factor(&self, state: &Self::State, window: window::Id) -> f32 {
        positive_or_one(taim_debug::hot(|| self.raw.scale_factor(state, window)))
    }

    fn presets(&self) -> &[Preset<Self::State, Self::Message>] {
        &self.presets
    }
}

#[inline]
fn positive_or_one(value: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        1.0
    }
}

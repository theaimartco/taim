//! Configuration and runtime delegation for [`Application`].
use super::{Application, ThemeFn, TitleFn};
use crate::backend;
use crate::message;
use crate::program::{self, Program};
use crate::theme;
use crate::window;
use crate::{Backend, Element, Executor, Font, Preset, Result, Settings, Size, Subscription, Task};
use std::borrow::Cow;

impl<P: Program> Application<P> {
    /// Starts the native runner.
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

    /// Replaces initial window settings.
    pub fn window(mut self, window: window::Settings) -> Self {
        self.window = window;
        self
    }

    /// Centers the initial window.
    pub fn centered(mut self) -> Self {
        self.window.position = window::Position::Centered;
        self
    }

    /// Configures automatic closing after a close request.
    pub fn exit_on_close_request(mut self, enabled: bool) -> Self {
        self.window.exit_on_close_request = enabled;
        self
    }

    /// Sets finite, non-negative initial logical dimensions.
    pub fn window_size(mut self, size: impl Into<Size>) -> Self {
        let size = size.into();
        self.window.size = Size::new(extent(size.width), extent(size.height));
        self
    }

    /// Configures window transparency.
    pub fn transparent(mut self, enabled: bool) -> Self {
        self.window.transparent = enabled;
        self
    }

    /// Configures window resizing.
    pub fn resizable(mut self, enabled: bool) -> Self {
        self.window.resizable = enabled;
        self
    }

    /// Configures platform window decorations.
    pub fn decorations(mut self, enabled: bool) -> Self {
        self.window.decorations = enabled;
        self
    }

    /// Sets the initial window position.
    pub fn position(mut self, position: window::Position) -> Self {
        self.window.position = position;
        self
    }

    /// Sets the initial window stacking level.
    pub fn level(mut self, level: window::Level) -> Self {
        self.window.level = level;
        self
    }

    /// Selects a renderer backend.
    pub fn backend(mut self, backend: Backend) -> Self {
        self.settings.backend = backend;
        self
    }

    /// Selects a GPU power preference.
    pub fn power_preference(mut self, preference: backend::PowerPreference) -> Self {
        self.settings.power_preference = preference;
        self
    }

    /// Installs dynamic title logic.
    pub fn title(
        self,
        title: impl TitleFn<P::State>,
    ) -> Application<impl Program<State = P::State, Message = P::Message, Theme = P::Theme>> {
        Application {
            raw: program::with_title(self.raw, move |state, _| title.title(state)),
            settings: self.settings,
            window: self.window,
            presets: self.presets,
        }
    }

    /// Installs passive subscription logic.
    pub fn subscription(
        self,
        subscription: impl Fn(&P::State) -> Subscription<P::Message>,
    ) -> Application<impl Program<State = P::State, Message = P::Message, Theme = P::Theme>> {
        Application {
            raw: program::with_subscription(self.raw, subscription),
            settings: self.settings,
            window: self.window,
            presets: self.presets,
        }
    }

    /// Installs dynamic theme selection.
    pub fn theme(
        self,
        theme: impl ThemeFn<P::State, P::Theme>,
    ) -> Application<impl Program<State = P::State, Message = P::Message, Theme = P::Theme>> {
        Application {
            raw: program::with_theme(self.raw, move |state, _| theme.theme(state)),
            settings: self.settings,
            window: self.window,
            presets: self.presets,
        }
    }

    /// Installs dynamic application styling.
    pub fn style(
        self,
        style: impl Fn(&P::State, &P::Theme) -> theme::Style,
    ) -> Application<impl Program<State = P::State, Message = P::Message, Theme = P::Theme>> {
        Application {
            raw: program::with_style(self.raw, style),
            settings: self.settings,
            window: self.window,
            presets: self.presets,
        }
    }

    /// Installs a guarded application scale function.
    pub fn scale_factor(
        self,
        scale: impl Fn(&P::State) -> f32,
    ) -> Application<impl Program<State = P::State, Message = P::Message, Theme = P::Theme>> {
        Application {
            raw: program::with_scale_factor(self.raw, move |state, _| {
                positive_or_one(scale(state))
            }),
            settings: self.settings,
            window: self.window,
            presets: self.presets,
        }
    }

    /// Selects a task executor.
    pub fn executor<E>(
        self,
    ) -> Application<impl Program<State = P::State, Message = P::Message, Theme = P::Theme>>
    where
        E: Executor,
    {
        Application {
            raw: program::with_executor::<P, E>(self.raw),
            settings: self.settings,
            window: self.window,
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

impl<P: Program> Program for Application<P> {
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
        Some(self.window.clone())
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
fn extent(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
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

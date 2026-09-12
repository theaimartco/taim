//! The definition of an taim program.
pub use taim_graphics as graphics;
pub use taim_runtime as runtime;
pub use taim_runtime::core;
pub use taim_runtime::futures;

pub mod message;

mod preset;

pub use preset::Preset;

use crate::core::renderer;
use crate::core::text;
use crate::core::theme;
use crate::core::window;
use crate::core::{Element, Font, Settings};
use crate::futures::{Executor, Subscription};
use crate::graphics::compositor;
use crate::runtime::Task;

/// An interactive, native, cross-platform, multi-windowed application.
///
/// A [`Program`] can execute asynchronous actions by returning a
/// [`Task`] in some of its methods.
#[allow(missing_docs)]
pub trait Program: Sized {
    /// The state of the program.
    type State;

    /// The message of the program.
    type Message: Send + 'static;

    /// The theme of the program.
    type Theme: theme::Base;

    /// The renderer of the program.
    type Renderer: Renderer;

    /// The executor of the program.
    type Executor: Executor;

    /// Returns the unique name of the [`Program`].
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
        let mut title = String::new();

        for (i, part) in Self::name().split("_").enumerate() {
            use std::borrow::Cow;

            let part = match part {
                "a" | "an" | "of" | "in" | "and" => Cow::Borrowed(part),
                _ => {
                    let mut part = part.to_owned();

                    if let Some(first_letter) = part.get_mut(0..1) {
                        first_letter.make_ascii_uppercase();
                    }

                    Cow::Owned(part)
                }
            };

            if i > 0 {
                title.push(' ');
            }

            title.push_str(&part);
        }

        format!("{title} - Taim")
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

/// Decorates a [`Program`] with the given title function.
pub fn with_title<P: Program>(
    program: P,
    title: impl Fn(&P::State, window::Id) -> String,
) -> impl Program<State = P::State, Message = P::Message, Theme = P::Theme> {
    struct WithTitle<P, Title> {
        program: P,
        title: Title,
    }

    impl<P, Title> Program for WithTitle<P, Title>
    where
        P: Program,
        Title: Fn(&P::State, window::Id) -> String,
    {
        type State = P::State;
        type Message = P::Message;
        type Theme = P::Theme;
        type Renderer = P::Renderer;
        type Executor = P::Executor;

        fn title(&self, state: &Self::State, window: window::Id) -> String {
            (self.title)(state, window)
        }

        fn name() -> &'static str {
            P::name()
        }

        fn settings(&self) -> Settings {
            self.program.settings()
        }

        fn window(&self) -> Option<window::Settings> {
            self.program.window()
        }

        fn boot(&self) -> (Self::State, Task<Self::Message>) {
            self.program.boot()
        }

        fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message> {
            self.program.update(state, message)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
        ) -> Element<'a, Self::Message, Self::Theme, Self::Renderer> {
            self.program.view(state, window)
        }

        fn theme(&self, state: &Self::State, window: window::Id) -> Option<Self::Theme> {
            self.program.theme(state, window)
        }

        fn subscription(&self, state: &Self::State) -> Subscription<Self::Message> {
            self.program.subscription(state)
        }

        fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
            self.program.style(state, theme)
        }

        fn scale_factor(&self, state: &Self::State, window: window::Id) -> f32 {
            self.program.scale_factor(state, window)
        }
    }

    WithTitle { program, title }
}

/// Decorates a [`Program`] with the given subscription function.
pub fn with_subscription<P: Program>(
    program: P,
    f: impl Fn(&P::State) -> Subscription<P::Message>,
) -> impl Program<State = P::State, Message = P::Message, Theme = P::Theme> {
    struct WithSubscription<P, F> {
        program: P,
        subscription: F,
    }

    impl<P: Program, F> Program for WithSubscription<P, F>
    where
        F: Fn(&P::State) -> Subscription<P::Message>,
    {
        type State = P::State;
        type Message = P::Message;
        type Theme = P::Theme;
        type Renderer = P::Renderer;
        type Executor = P::Executor;

        fn subscription(&self, state: &Self::State) -> Subscription<Self::Message> {
            (self.subscription)(state)
        }

        fn name() -> &'static str {
            P::name()
        }

        fn settings(&self) -> Settings {
            self.program.settings()
        }

        fn window(&self) -> Option<window::Settings> {
            self.program.window()
        }

        fn boot(&self) -> (Self::State, Task<Self::Message>) {
            self.program.boot()
        }

        fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message> {
            self.program.update(state, message)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
        ) -> Element<'a, Self::Message, Self::Theme, Self::Renderer> {
            self.program.view(state, window)
        }

        fn title(&self, state: &Self::State, window: window::Id) -> String {
            self.program.title(state, window)
        }

        fn theme(&self, state: &Self::State, window: window::Id) -> Option<Self::Theme> {
            self.program.theme(state, window)
        }

        fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
            self.program.style(state, theme)
        }

        fn scale_factor(&self, state: &Self::State, window: window::Id) -> f32 {
            self.program.scale_factor(state, window)
        }
    }

    WithSubscription {
        program,
        subscription: f,
    }
}

/// Decorates a [`Program`] with the given theme function.
pub fn with_theme<P: Program>(
    program: P,
    f: impl Fn(&P::State, window::Id) -> Option<P::Theme>,
) -> impl Program<State = P::State, Message = P::Message, Theme = P::Theme> {
    struct WithTheme<P, F> {
        program: P,
        theme: F,
    }

    impl<P: Program, F> Program for WithTheme<P, F>
    where
        F: Fn(&P::State, window::Id) -> Option<P::Theme>,
    {
        type State = P::State;
        type Message = P::Message;
        type Theme = P::Theme;
        type Renderer = P::Renderer;
        type Executor = P::Executor;

        fn theme(&self, state: &Self::State, window: window::Id) -> Option<Self::Theme> {
            (self.theme)(state, window)
        }

        fn name() -> &'static str {
            P::name()
        }

        fn settings(&self) -> Settings {
            self.program.settings()
        }

        fn window(&self) -> Option<window::Settings> {
            self.program.window()
        }

        fn boot(&self) -> (Self::State, Task<Self::Message>) {
            self.program.boot()
        }

        fn title(&self, state: &Self::State, window: window::Id) -> String {
            self.program.title(state, window)
        }

        fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message> {
            self.program.update(state, message)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
        ) -> Element<'a, Self::Message, Self::Theme, Self::Renderer> {
            self.program.view(state, window)
        }

        fn subscription(&self, state: &Self::State) -> Subscription<Self::Message> {
            self.program.subscription(state)
        }

        fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
            self.program.style(state, theme)
        }

        fn scale_factor(&self, state: &Self::State, window: window::Id) -> f32 {
            self.program.scale_factor(state, window)
        }
    }

    WithTheme { program, theme: f }
}

/// Decorates a [`Program`] with the given style function.
pub fn with_style<P: Program>(
    program: P,
    f: impl Fn(&P::State, &P::Theme) -> theme::Style,
) -> impl Program<State = P::State, Message = P::Message, Theme = P::Theme> {
    struct WithStyle<P, F> {
        program: P,
        style: F,
    }

    impl<P: Program, F> Program for WithStyle<P, F>
    where
        F: Fn(&P::State, &P::Theme) -> theme::Style,
    {
        type State = P::State;
        type Message = P::Message;
        type Theme = P::Theme;
        type Renderer = P::Renderer;
        type Executor = P::Executor;

        fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
            (self.style)(state, theme)
        }

        fn name() -> &'static str {
            P::name()
        }

        fn settings(&self) -> Settings {
            self.program.settings()
        }

        fn window(&self) -> Option<window::Settings> {
            self.program.window()
        }

        fn boot(&self) -> (Self::State, Task<Self::Message>) {
            self.program.boot()
        }

        fn title(&self, state: &Self::State, window: window::Id) -> String {
            self.program.title(state, window)
        }

        fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message> {
            self.program.update(state, message)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
        ) -> Element<'a, Self::Message, Self::Theme, Self::Renderer> {
            self.program.view(state, window)
        }

        fn subscription(&self, state: &Self::State) -> Subscription<Self::Message> {
            self.program.subscription(state)
        }

        fn theme(&self, state: &Self::State, window: window::Id) -> Option<Self::Theme> {
            self.program.theme(state, window)
        }

        fn scale_factor(&self, state: &Self::State, window: window::Id) -> f32 {
            self.program.scale_factor(state, window)
        }
    }

    WithStyle { program, style: f }
}

/// Decorates a [`Program`] with the given scale factor function.
pub fn with_scale_factor<P: Program>(
    program: P,
    f: impl Fn(&P::State, window::Id) -> f32,
) -> impl Program<State = P::State, Message = P::Message, Theme = P::Theme> {
    struct WithScaleFactor<P, F> {
        program: P,
        scale_factor: F,
    }

    impl<P: Program, F> Program for WithScaleFactor<P, F>
    where
        F: Fn(&P::State, window::Id) -> f32,
    {
        type State = P::State;
        type Message = P::Message;
        type Theme = P::Theme;
        type Renderer = P::Renderer;
        type Executor = P::Executor;

        fn title(&self, state: &Self::State, window: window::Id) -> String {
            self.program.title(state, window)
        }

        fn name() -> &'static str {
            P::name()
        }

        fn settings(&self) -> Settings {
            self.program.settings()
        }

        fn window(&self) -> Option<window::Settings> {
            self.program.window()
        }

        fn boot(&self) -> (Self::State, Task<Self::Message>) {
            self.program.boot()
        }

        fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message> {
            self.program.update(state, message)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
        ) -> Element<'a, Self::Message, Self::Theme, Self::Renderer> {
            self.program.view(state, window)
        }

        fn subscription(&self, state: &Self::State) -> Subscription<Self::Message> {
            self.program.subscription(state)
        }

        fn theme(&self, state: &Self::State, window: window::Id) -> Option<Self::Theme> {
            self.program.theme(state, window)
        }

        fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
            self.program.style(state, theme)
        }

        fn scale_factor(&self, state: &Self::State, window: window::Id) -> f32 {
            (self.scale_factor)(state, window)
        }
    }

    WithScaleFactor {
        program,
        scale_factor: f,
    }
}

/// Decorates a [`Program`] with the given executor function.
pub fn with_executor<P: Program, E: Executor>(
    program: P,
) -> impl Program<State = P::State, Message = P::Message, Theme = P::Theme> {
    use std::marker::PhantomData;

    struct WithExecutor<P, E> {
        program: P,
        executor: PhantomData<E>,
    }

    impl<P: Program, E> Program for WithExecutor<P, E>
    where
        E: Executor,
    {
        type State = P::State;
        type Message = P::Message;
        type Theme = P::Theme;
        type Renderer = P::Renderer;
        type Executor = E;

        fn title(&self, state: &Self::State, window: window::Id) -> String {
            self.program.title(state, window)
        }

        fn name() -> &'static str {
            P::name()
        }

        fn settings(&self) -> Settings {
            self.program.settings()
        }

        fn window(&self) -> Option<window::Settings> {
            self.program.window()
        }

        fn boot(&self) -> (Self::State, Task<Self::Message>) {
            self.program.boot()
        }

        fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message> {
            self.program.update(state, message)
        }

        fn view<'a>(
            &self,
            state: &'a Self::State,
            window: window::Id,
        ) -> Element<'a, Self::Message, Self::Theme, Self::Renderer> {
            self.program.view(state, window)
        }

        fn subscription(&self, state: &Self::State) -> Subscription<Self::Message> {
            self.program.subscription(state)
        }

        fn theme(&self, state: &Self::State, window: window::Id) -> Option<Self::Theme> {
            self.program.theme(state, window)
        }

        fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
            self.program.style(state, theme)
        }

        fn scale_factor(&self, state: &Self::State, window: window::Id) -> f32 {
            self.program.scale_factor(state, window)
        }
    }

    WithExecutor {
        program,
        executor: PhantomData::<E>,
    }
}

/// The renderer of some [`Program`].
pub trait Renderer: text::Renderer<Font = Font> + compositor::Default + renderer::Headless {}

impl<T> Renderer for T where
    T: text::Renderer<Font = Font> + compositor::Default + renderer::Headless
{
}

/// A particular instance of a running [`Program`].
pub struct Instance<P: Program> {
    program: P,
    state: P::State,
}

impl<P: Program> Instance<P> {
    /// Creates a new [`Instance`] of the given [`Program`].
    pub fn new(program: P) -> (Self, Task<P::Message>) {
        let (state, task) = program.boot();

        (Self { program, state }, task)
    }

    /// Returns the current title of the [`Instance`].
    pub fn title(&self, window: window::Id) -> String {
        self.program.title(&self.state, window)
    }

    /// Processes the given message and updates the [`Instance`].
    pub fn update(&mut self, message: P::Message) -> Task<P::Message> {
        self.program.update(&mut self.state, message)
    }

    /// Produces the current widget tree of the [`Instance`].
    pub fn view(&self, window: window::Id) -> Element<'_, P::Message, P::Theme, P::Renderer> {
        self.program.view(&self.state, window)
    }

    /// Returns the current [`Subscription`] of the [`Instance`].
    pub fn subscription(&self) -> Subscription<P::Message> {
        self.program.subscription(&self.state)
    }

    /// Returns the current theme of the [`Instance`].
    pub fn theme(&self, window: window::Id) -> Option<P::Theme> {
        self.program.theme(&self.state, window)
    }

    /// Returns the current [`theme::Style`] of the [`Instance`].
    pub fn style(&self, theme: &P::Theme) -> theme::Style {
        self.program.style(&self.state, theme)
    }

    /// Returns the current scale factor of the [`Instance`].
    pub fn scale_factor(&self, window: window::Id) -> f32 {
        self.program.scale_factor(&self.state, window)
    }

    /// Returns a reference to the internal program state.
    pub fn state(&self) -> &P::State {
        &self.state
    }

    /// Returns a mutable reference to the internal program state.
    pub fn state_mut(&mut self) -> &mut P::State {
        &mut self.state
    }
}

/// The modern, first-class application trait for TAIM.
///
/// An [`App`] defines the state, messages, view hierarchy, and platform
/// capabilities of a TAIM native application.
pub trait App: Sized {
    /// The state of the application.
    type State;

    /// The message type of the application.
    type Message: Send + 'static;

    /// The theme used by the application.
    type Theme: theme::Base;

    /// The renderer used by the application.
    type Renderer: Renderer;

    /// The executor used by the application.
    type Executor: Executor;

    /// Returns the unique name of the [`App`].
    fn name() -> &'static str;

    /// Boots the application, producing initial state and startup tasks.
    fn boot(&self) -> (Self::State, Task<Self::Message>);

    /// Handles an incoming message, mutating state and optionally returning a task.
    fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message>;

    /// Produces the widget tree for the specified window.
    fn view<'a>(
        &self,
        state: &'a Self::State,
        window: window::Id,
    ) -> Element<'a, Self::Message, Self::Theme, Self::Renderer>;

    /// Returns the window title.
    fn title(&self, _state: &Self::State, _window: window::Id) -> String {
        format!("{} - Taim", Self::name())
    }

    /// Returns the active event subscriptions for the current state.
    fn subscription(&self, _state: &Self::State) -> Subscription<Self::Message> {
        Subscription::none()
    }

    /// Returns the theme override for the given window.
    fn theme(&self, _state: &Self::State, _window: window::Id) -> Option<Self::Theme> {
        None
    }

    /// Returns the base style for the given theme.
    fn style(&self, _state: &Self::State, theme: &Self::Theme) -> theme::Style {
        theme::Base::base(theme)
    }

    /// Returns the scale factor for the given window.
    fn scale_factor(&self, _state: &Self::State, _window: window::Id) -> f32 {
        1.0
    }

    /// Application settings.
    fn settings(&self) -> Settings {
        Settings::default()
    }

    /// Primary window settings.
    fn window(&self) -> Option<window::Settings> {
        None
    }

    /// Presets supported by the application.
    fn presets(&self) -> &[Preset<Self::State, Self::Message>] {
        &[]
    }

    /// Adapts this [`App`] into a type implementing [`Program`].
    fn into_program(self) -> AppAdapter<Self> {
        AppAdapter(self)
    }
}

/// An adapter allowing an [`App`] to be executed anywhere a [`Program`] is accepted.
#[derive(Debug, Clone, Copy)]
pub struct AppAdapter<A>(pub A);

impl<A: App> Program for AppAdapter<A> {
    type State = A::State;
    type Message = A::Message;
    type Theme = A::Theme;
    type Renderer = A::Renderer;
    type Executor = A::Executor;

    fn name() -> &'static str {
        A::name()
    }

    fn settings(&self) -> Settings {
        self.0.settings()
    }

    fn window(&self) -> Option<window::Settings> {
        self.0.window()
    }

    fn boot(&self) -> (Self::State, Task<Self::Message>) {
        self.0.boot()
    }

    fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message> {
        self.0.update(state, message)
    }

    fn view<'a>(
        &self,
        state: &'a Self::State,
        window: window::Id,
    ) -> Element<'a, Self::Message, Self::Theme, Self::Renderer> {
        self.0.view(state, window)
    }

    fn title(&self, state: &Self::State, window: window::Id) -> String {
        self.0.title(state, window)
    }

    fn subscription(&self, state: &Self::State) -> Subscription<Self::Message> {
        self.0.subscription(state)
    }

    fn theme(&self, state: &Self::State, window: window::Id) -> Option<Self::Theme> {
        self.0.theme(state, window)
    }

    fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
        self.0.style(state, theme)
    }

    fn scale_factor(&self, state: &Self::State, window: window::Id) -> f32 {
        self.0.scale_factor(state, window)
    }

    fn presets(&self) -> &[Preset<Self::State, Self::Message>] {
        self.0.presets()
    }
}

/// Runtime context passed to applications.
#[derive(Debug, Clone, Default)]
pub struct AppContext {
    frame_count: u64,
}

impl AppContext {
    /// Creates a new default [`AppContext`].
    pub fn new() -> Self {
        Self { frame_count: 0 }
    }

    /// Increments the frame count and returns the new value.
    pub fn tick(&mut self) -> u64 {
        self.frame_count = self.frame_count.wrapping_add(1);
        self.frame_count
    }

    /// Returns the current frame count.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
}

/// Trait alias representing valid application state.
pub trait AppState: 'static {}
impl<T: 'static> AppState for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::theme::Theme;
    use crate::core::widget::Tree;
    use crate::core::{Layout, Length, Rectangle, Size, Widget};

    struct TestWidget;
    impl<Message, Theme, Renderer: crate::core::Renderer> Widget<Message, Theme, Renderer>
        for TestWidget
    {
        fn size(&self) -> Size<Length> {
            Size::new(Length::Shrink, Length::Shrink)
        }
        fn layout(
            &mut self,
            _tree: &mut Tree,
            _renderer: &Renderer,
            _limits: &crate::core::layout::Limits,
        ) -> crate::core::layout::Node {
            crate::core::layout::Node::new(Size::new(100.0, 50.0))
        }
        fn draw(
            &self,
            _tree: &Tree,
            _renderer: &mut Renderer,
            _theme: &Theme,
            _style: &renderer::Style,
            _layout: Layout<'_>,
            _cursor: crate::core::mouse::Cursor,
            _viewport: &Rectangle,
        ) {
        }
    }

    struct CounterApp;

    impl App for CounterApp {
        type State = i32;
        type Message = i32;
        type Theme = Theme;
        type Renderer = ();
        type Executor = futures::backend::default::Executor;

        fn name() -> &'static str {
            "counter_app"
        }

        fn boot(&self) -> (Self::State, Task<Self::Message>) {
            (10, Task::none())
        }

        fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message> {
            *state += message;
            Task::none()
        }

        fn view<'a>(
            &self,
            _state: &'a Self::State,
            _window: window::Id,
        ) -> Element<'a, Self::Message, Self::Theme, Self::Renderer> {
            Element::new(TestWidget)
        }
    }

    #[test]
    fn test_app_adapter_and_instance() {
        let app = CounterApp;
        let (mut instance, _task) = Instance::new(app.into_program());
        let window_id = window::Id::unique();

        assert_eq!(*instance.state(), 10);
        assert_eq!(instance.title(window_id), "counter_app - Taim");
        assert_eq!(instance.scale_factor(window_id), 1.0);

        let _task = instance.update(5);
        assert_eq!(*instance.state(), 15);

        let _task = instance.update(-3);
        assert_eq!(*instance.state(), 12);
    }

    #[test]
    fn test_app_context() {
        let mut ctx = AppContext::new();
        assert_eq!(ctx.frame_count(), 0);
        assert_eq!(ctx.tick(), 1);
        assert_eq!(ctx.tick(), 2);
        assert_eq!(ctx.frame_count(), 2);
    }
}

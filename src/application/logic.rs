//! User-supplied application lifecycle functions.
use crate::{Element, Never, Task, Theme};

/// Boot logic accepted by [`super::application`].
pub trait BootFn<State, Message> {
    /// Produces initial state and optional startup work.
    fn boot(&self) -> (State, Task<Message>);
}

impl<T, C, State, Message> BootFn<State, Message> for T
where
    T: Fn() -> C,
    C: IntoBoot<State, Message>,
{
    fn boot(&self) -> (State, Task<Message>) {
        self().into_boot()
    }
}

/// A value that can initialize an application.
pub trait IntoBoot<State, Message> {
    /// Converts into state plus startup work.
    fn into_boot(self) -> (State, Task<Message>);
}

impl<State, Message> IntoBoot<State, Message> for State {
    fn into_boot(self) -> (State, Task<Message>) {
        (self, Task::none())
    }
}

impl<State, Message> IntoBoot<State, Message> for (State, Task<Message>) {
    fn into_boot(self) -> (State, Task<Message>) {
        self
    }
}

/// Dynamic window-title logic.
pub trait TitleFn<State> {
    /// Produces a title for current state.
    fn title(&self, state: &State) -> String;
}

impl<State> TitleFn<State> for &'static str {
    fn title(&self, _state: &State) -> String {
        String::from(*self)
    }
}

impl<T, State> TitleFn<State> for T
where
    T: Fn(&State) -> String,
{
    fn title(&self, state: &State) -> String {
        self(state)
    }
}

/// State transition logic.
pub trait UpdateFn<State, Message> {
    /// Applies one message and returns any follow-up work.
    fn update(&self, state: &mut State, message: Message) -> Task<Message>;
}

impl<State> UpdateFn<State, Never> for () {
    fn update(&self, _state: &mut State, _message: Never) -> Task<Never> {
        Task::none()
    }
}

impl<T, State, Message, Output> UpdateFn<State, Message> for T
where
    T: Fn(&mut State, Message) -> Output,
    Output: Into<Task<Message>>,
{
    fn update(&self, state: &mut State, message: Message) -> Task<Message> {
        self(state, message).into()
    }
}

/// View construction logic.
pub trait ViewFn<'a, State, Message, Theme, Renderer> {
    /// Builds the widget hierarchy for current state.
    fn view(&self, state: &'a State) -> Element<'a, Message, Theme, Renderer>;
}

impl<'a, F, State, Message, Theme, Renderer, Output> ViewFn<'a, State, Message, Theme, Renderer>
    for F
where
    F: Fn(&'a State) -> Output,
    State: 'static,
    Output: Into<Element<'a, Message, Theme, Renderer>>,
{
    fn view(&self, state: &'a State) -> Element<'a, Message, Theme, Renderer> {
        self(state).into()
    }
}

/// Theme selection logic.
pub trait ThemeFn<State, Output> {
    /// Returns an explicit theme or requests system-theme selection.
    fn theme(&self, state: &State) -> Option<Output>;
}

impl<State> ThemeFn<State, Theme> for Theme {
    fn theme(&self, _state: &State) -> Option<Theme> {
        Some(self.clone())
    }
}

impl<F, T, State, Output> ThemeFn<State, Output> for F
where
    F: Fn(&State) -> T,
    T: Into<Option<Output>>,
{
    fn theme(&self, state: &State) -> Option<Output> {
        (self)(state).into()
    }
}

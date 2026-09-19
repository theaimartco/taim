//! Window-aware daemon presentation functions.
use crate::{Element, Theme, window};

/// Dynamic title logic for daemon-owned windows.
pub trait TitleFn<State> {
    /// Produces the title of one window.
    fn title(&self, state: &State, window: window::Id) -> String;
}

impl<State> TitleFn<State> for &'static str {
    fn title(&self, _state: &State, _window: window::Id) -> String {
        String::from(*self)
    }
}

impl<F, State> TitleFn<State> for F
where
    F: Fn(&State, window::Id) -> String,
{
    fn title(&self, state: &State, window: window::Id) -> String {
        self(state, window)
    }
}

/// Window-aware daemon view logic.
pub trait ViewFn<'a, State, Message, Theme, Renderer> {
    /// Builds the hierarchy for one window.
    fn view(&self, state: &'a State, window: window::Id) -> Element<'a, Message, Theme, Renderer>;
}

impl<'a, F, State, Message, Theme, Renderer, Output> ViewFn<'a, State, Message, Theme, Renderer>
    for F
where
    F: Fn(&'a State, window::Id) -> Output,
    State: 'static,
    Output: Into<Element<'a, Message, Theme, Renderer>>,
{
    fn view(&self, state: &'a State, window: window::Id) -> Element<'a, Message, Theme, Renderer> {
        self(state, window).into()
    }
}

/// Window-aware theme selection.
pub trait ThemeFn<State, Output> {
    /// Returns an explicit theme or requests system-theme selection.
    fn theme(&self, state: &State, window: window::Id) -> Option<Output>;
}

impl<State> ThemeFn<State, Theme> for Theme {
    fn theme(&self, _state: &State, _window: window::Id) -> Option<Theme> {
        Some(self.clone())
    }
}

impl<F, T, State, Output> ThemeFn<State, Output> for F
where
    F: Fn(&State, window::Id) -> T,
    T: Into<Option<Output>>,
{
    fn theme(&self, state: &State, window: window::Id) -> Option<Output> {
        (self)(state, window).into()
    }
}

//! Top-level runner conveniences.
use crate::{Result, application, message, program, theme};

/// Runs an application with default state and settings.
pub fn run<State, Message, Theme, Renderer>(
    update: impl crate::application::UpdateFn<State, Message> + 'static,
    view: impl for<'a> crate::application::ViewFn<'a, State, Message, Theme, Renderer> + 'static,
) -> Result
where
    State: Default + 'static,
    Message: Send + message::MaybeDebug + message::MaybeClone + 'static,
    Theme: theme::Base + 'static,
    Renderer: program::Renderer + 'static,
{
    application(State::default, update, view).run()
}

/// Runs a native TAIM [`crate::App`].
pub fn run_app<A: program::App + 'static>(app: A) -> Result
where
    A::Theme: theme::Base + 'static,
    A::Message: message::MaybeDebug + message::MaybeClone,
{
    Ok(taim_winit::run_app(app)?)
}

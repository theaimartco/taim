//! An [`Application`] that receives an [`Instant`] in update logic.
use crate::application::{Application, BootFn, ViewFn};
use crate::program;
use crate::theme;
use crate::time::Instant;
use crate::window;
use crate::{Program, Settings, Subscription, Task};

mod adapter;

use adapter::TimedAdapter;

/// Creates an [`Application`] with an `update` function that also
/// takes the [`Instant`] of each `Message`.
///
/// This constructor is useful to create animated applications that
/// are _pure_ (e.g. without relying on side-effect calls like [`Instant::now`]).
///
/// Purity is needed when you want your application to end up in the
/// same exact state given the same history of messages. This property
/// enables proper time-traveling debugging with [`Taim Lab`][`comet`].
///
/// [`comet`]: https://github.com/theaimartco/taim/tree/main/tools/taim_lab
pub fn timed<State, Message, Theme, Renderer>(
    boot: impl BootFn<State, Message>,
    update: impl UpdateFn<State, Message>,
    subscription: impl Fn(&State) -> Subscription<Message>,
    view: impl for<'a> ViewFn<'a, State, Message, Theme, Renderer>,
) -> Application<impl Program<State = State, Message = (Message, Instant), Theme = Theme>>
where
    State: 'static,
    Message: Send + 'static,
    Theme: theme::Base + 'static,
    Renderer: program::Renderer + 'static,
{
    Application {
        raw: TimedAdapter::new(boot, update, subscription, view),
        settings: Settings::default(),
        window: window::Settings::default(),
        presets: Vec::new(),
    }
}

/// The update logic of some timed [`Application`].
///
/// This is like [`application::UpdateFn`](super::UpdateFn),
/// but it also takes an [`Instant`].
pub trait UpdateFn<State, Message> {
    /// Processes the message and updates the state of the [`Application`].
    fn update(&self, state: &mut State, message: Message, now: Instant)
    -> impl Into<Task<Message>>;
}

impl<State, Message> UpdateFn<State, Message> for () {
    fn update(
        &self,
        _state: &mut State,
        _message: Message,
        _now: Instant,
    ) -> impl Into<Task<Message>> {
    }
}

impl<T, State, Message, C> UpdateFn<State, Message> for T
where
    T: Fn(&mut State, Message, Instant) -> C,
    C: Into<Task<Message>>,
{
    fn update(
        &self,
        state: &mut State,
        message: Message,
        now: Instant,
    ) -> impl Into<Task<Message>> {
        self(state, message, now)
    }
}

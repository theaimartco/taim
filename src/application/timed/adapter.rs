use super::UpdateFn;
use crate::application::{BootFn, ViewFn};
use crate::program;
use crate::theme;
use crate::time::Instant;
use crate::{Element, Program, Settings, Subscription, Task, window};

use std::marker::PhantomData;

use taim_debug as debug;

/// Internal boundary that timestamps every message entering a timed program.
pub(super) struct TimedAdapter<State, Message, Theme, Renderer, Boot, Update, SubscriptionFn, View>
{
    boot: Boot,
    update: Update,
    subscription: SubscriptionFn,
    view: View,
    marker: PhantomData<fn() -> (State, Message, Theme, Renderer)>,
}

impl<State, Message, Theme, Renderer, Boot, Update, SubscriptionFn, View>
    TimedAdapter<State, Message, Theme, Renderer, Boot, Update, SubscriptionFn, View>
{
    pub(super) fn new(
        boot: Boot,
        update: Update,
        subscription: SubscriptionFn,
        view: View,
    ) -> Self {
        Self {
            boot,
            update,
            subscription,
            view,
            marker: PhantomData,
        }
    }
}

#[inline]
fn stamp<Message>(message: Message) -> (Message, Instant) {
    (message, Instant::now())
}

impl<State, Message, Theme, Renderer, Boot, Update, SubscriptionFn, View> Program
    for TimedAdapter<State, Message, Theme, Renderer, Boot, Update, SubscriptionFn, View>
where
    Message: Send + 'static,
    Theme: theme::Base + 'static,
    Renderer: program::Renderer + 'static,
    Boot: BootFn<State, Message>,
    Update: UpdateFn<State, Message>,
    SubscriptionFn: Fn(&State) -> Subscription<Message>,
    View: for<'a> ViewFn<'a, State, Message, Theme, Renderer>,
{
    type State = State;
    type Message = (Message, Instant);
    type Theme = Theme;
    type Renderer = Renderer;
    type Executor = taim_futures::backend::default::Executor;

    fn name() -> &'static str {
        std::any::type_name::<State>()
            .rsplit("::")
            .next()
            .unwrap_or("taim_application")
    }

    fn settings(&self) -> Settings {
        Settings::default()
    }

    fn window(&self) -> Option<window::Settings> {
        Some(window::Settings::default())
    }

    fn boot(&self) -> (State, Task<Self::Message>) {
        let (state, task) = self.boot.boot();
        (state, task.map(stamp))
    }

    fn update(
        &self,
        state: &mut Self::State,
        (message, now): Self::Message,
    ) -> Task<Self::Message> {
        debug::hot(move || self.update.update(state, message, now).into().map(stamp))
    }

    fn view<'a>(
        &self,
        state: &'a Self::State,
        _window: window::Id,
    ) -> Element<'a, Self::Message, Self::Theme, Self::Renderer> {
        debug::hot(|| self.view.view(state).map(stamp))
    }

    fn subscription(&self, state: &Self::State) -> Subscription<Self::Message> {
        debug::hot(|| (self.subscription)(state).map(stamp))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct VisibleState;

    type Adapter = TimedAdapter<
        VisibleState,
        (),
        crate::Theme,
        crate::Renderer,
        fn() -> VisibleState,
        (),
        fn(&VisibleState) -> Subscription<()>,
        for<'a> fn(&'a VisibleState) -> Element<'a, (), crate::Theme, crate::Renderer>,
    >;

    #[test]
    fn default_name_uses_the_state_type_instead_of_the_crate_namespace() {
        assert_eq!(<Adapter as Program>::name(), "VisibleState");
    }
}

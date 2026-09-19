//! Source-compatible decorators for legacy programs.

use super::Program;
use crate::core::{Element, Settings, theme, window};
use crate::futures::{Executor, Subscription};
use crate::preset::Preset;
use crate::runtime::Task;

macro_rules! program_types {
    ($executor:ty) => {
        type State = P::State;
        type Message = P::Message;
        type Theme = P::Theme;
        type Renderer = P::Renderer;
        type Executor = $executor;
    };
}

macro_rules! forward_core {
    () => {
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

        fn presets(&self) -> &[Preset<Self::State, Self::Message>] {
            self.program.presets()
        }
    };
}

macro_rules! forward_title {
    () => {
        fn title(&self, state: &Self::State, window: window::Id) -> String {
            self.program.title(state, window)
        }
    };
}

macro_rules! forward_subscription {
    () => {
        fn subscription(&self, state: &Self::State) -> Subscription<Self::Message> {
            self.program.subscription(state)
        }
    };
}

macro_rules! forward_theme {
    () => {
        fn theme(&self, state: &Self::State, window: window::Id) -> Option<Self::Theme> {
            self.program.theme(state, window)
        }
    };
}

macro_rules! forward_style {
    () => {
        fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
            self.program.style(state, theme)
        }
    };
}

macro_rules! forward_scale_factor {
    () => {
        fn scale_factor(&self, state: &Self::State, window: window::Id) -> f32 {
            self.program.scale_factor(state, window)
        }
    };
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

    impl<P: Program, Title: Fn(&P::State, window::Id) -> String> Program for WithTitle<P, Title> {
        program_types!(P::Executor);
        forward_core!();

        fn title(&self, state: &Self::State, window: window::Id) -> String {
            (self.title)(state, window)
        }

        forward_subscription!();
        forward_theme!();
        forward_style!();
        forward_scale_factor!();
    }

    WithTitle { program, title }
}

/// Decorates a [`Program`] with the given subscription function.
pub fn with_subscription<P: Program>(
    program: P,
    subscription: impl Fn(&P::State) -> Subscription<P::Message>,
) -> impl Program<State = P::State, Message = P::Message, Theme = P::Theme> {
    struct WithSubscription<P, F> {
        program: P,
        subscription: F,
    }

    impl<P: Program, F: Fn(&P::State) -> Subscription<P::Message>> Program for WithSubscription<P, F> {
        program_types!(P::Executor);
        forward_core!();
        forward_title!();

        fn subscription(&self, state: &Self::State) -> Subscription<Self::Message> {
            (self.subscription)(state)
        }

        forward_theme!();
        forward_style!();
        forward_scale_factor!();
    }

    WithSubscription {
        program,
        subscription,
    }
}

/// Decorates a [`Program`] with the given theme function.
pub fn with_theme<P: Program>(
    program: P,
    theme: impl Fn(&P::State, window::Id) -> Option<P::Theme>,
) -> impl Program<State = P::State, Message = P::Message, Theme = P::Theme> {
    struct WithTheme<P, F> {
        program: P,
        theme: F,
    }

    impl<P: Program, F: Fn(&P::State, window::Id) -> Option<P::Theme>> Program for WithTheme<P, F> {
        program_types!(P::Executor);
        forward_core!();
        forward_title!();
        forward_subscription!();

        fn theme(&self, state: &Self::State, window: window::Id) -> Option<Self::Theme> {
            (self.theme)(state, window)
        }

        forward_style!();
        forward_scale_factor!();
    }

    WithTheme { program, theme }
}

/// Decorates a [`Program`] with the given style function.
pub fn with_style<P: Program>(
    program: P,
    style: impl Fn(&P::State, &P::Theme) -> theme::Style,
) -> impl Program<State = P::State, Message = P::Message, Theme = P::Theme> {
    struct WithStyle<P, F> {
        program: P,
        style: F,
    }

    impl<P: Program, F: Fn(&P::State, &P::Theme) -> theme::Style> Program for WithStyle<P, F> {
        program_types!(P::Executor);
        forward_core!();
        forward_title!();
        forward_subscription!();
        forward_theme!();

        fn style(&self, state: &Self::State, theme: &Self::Theme) -> theme::Style {
            (self.style)(state, theme)
        }

        forward_scale_factor!();
    }

    WithStyle { program, style }
}

/// Decorates a [`Program`] with the given scale factor function.
pub fn with_scale_factor<P: Program>(
    program: P,
    scale_factor: impl Fn(&P::State, window::Id) -> f32,
) -> impl Program<State = P::State, Message = P::Message, Theme = P::Theme> {
    struct WithScaleFactor<P, F> {
        program: P,
        scale_factor: F,
    }

    impl<P: Program, F: Fn(&P::State, window::Id) -> f32> Program for WithScaleFactor<P, F> {
        program_types!(P::Executor);
        forward_core!();
        forward_title!();
        forward_subscription!();
        forward_theme!();
        forward_style!();

        fn scale_factor(&self, state: &Self::State, window: window::Id) -> f32 {
            (self.scale_factor)(state, window)
        }
    }

    WithScaleFactor {
        program,
        scale_factor,
    }
}

/// Decorates a [`Program`] with the given executor.
pub fn with_executor<P: Program, E: Executor>(
    program: P,
) -> impl Program<State = P::State, Message = P::Message, Theme = P::Theme> {
    use std::marker::PhantomData;

    struct WithExecutor<P, E> {
        program: P,
        executor: PhantomData<E>,
    }

    impl<P: Program, E: Executor> Program for WithExecutor<P, E> {
        program_types!(E);
        forward_core!();
        forward_title!();
        forward_subscription!();
        forward_theme!();
        forward_style!();
        forward_scale_factor!();
    }

    WithExecutor {
        program,
        executor: PhantomData::<E>,
    }
}

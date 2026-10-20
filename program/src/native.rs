//! Native-first TAIM application construction.
//!
//! Unlike the inherited functional Program compatibility facade, this API
//! implements App directly and executes through AppRunner and FrameGate.
//! Update handlers report what actually changed, instead of assuming every
//! message needs a new frame. This is a semantic API, not a renamed alias.

use std::borrow::Cow;
use std::marker::PhantomData;

use crate::app::{App, AppConfig, AppContext, FrameRequest, StateDelta, ViewRoot, WindowContext};
use crate::core::{Settings, theme, window};
use crate::runtime::Task;

/// Explicit result of processing an application intent.
///
/// Background work and visual invalidation are independent: an input may
/// schedule work without changing the display, or repaint without scheduling
/// any new task. The renderer may skip clean frames.
pub struct Dispatch<Message> {
    /// Background tasks produced by the intent.
    pub task: Task<Message>,
    /// The scene work this intent invalidated.
    pub delta: StateDelta,
}

impl<Message> Dispatch<Message> {
    /// An intent that changes neither layout nor pixels.
    #[must_use]
    pub fn idle() -> Self {
        Self {
            task: Task::none(),
            delta: StateDelta::clean(),
        }
    }

    /// An intent that only changes paint output.
    #[must_use]
    pub fn paint() -> Self {
        Self {
            task: Task::none(),
            delta: StateDelta::redraw(),
        }
    }

    /// An intent that invalidates layout and paint.
    #[must_use]
    pub fn layout() -> Self {
        Self {
            task: Task::none(),
            delta: StateDelta::relayout(),
        }
    }

    /// An intent requesting another animation frame.
    #[must_use]
    pub fn animate() -> Self {
        Self {
            task: Task::none(),
            delta: StateDelta {
                layout_dirty: false,
                paint_dirty: true,
                frame: FrameRequest::Animate,
            },
        }
    }

    /// Attach async work without changing the invalidation contract.
    #[must_use]
    pub fn with_task(mut self, task: Task<Message>) -> Self {
        self.task = task;
        self
    }

    /// Apply the invalidation decision to the native frame scheduler.
    fn apply(&self, context: &mut AppContext) {
        context.invalidate(self.delta);
    }
}

/// Native TAIM application composed from typed boot, intent and scene handlers.
///
/// The application is driven directly by the AppRunner implementation, not
/// by the legacy ProgramAdapter path. The scene handler can borrow its state.
pub struct NativeApp<State, Message, Theme, Renderer, Boot, Reduce, Compose> {
    boot: Boot,
    reduce: Reduce,
    compose: Compose,
    config: AppConfig,
    types: PhantomData<fn() -> (State, Message, Theme, Renderer)>,
}

/// Construct an application using TAIM's native intent-driven lifecycle.
///
/// Boot initializes state, reduce returns an explicit frame invalidation
/// decision, and compose describes the window's UI. Unlike the inherited
/// functional facade, the reducer also receives native AppContext.
///
/// This constructor creates a primary window by default.
pub fn app<State, Message, Theme, Renderer, Boot, Reduce, Compose>(
    title: &'static str,
    boot: Boot,
    reduce: Reduce,
    compose: Compose,
) -> NativeApp<State, Message, Theme, Renderer, Boot, Reduce, Compose>
where
    State: 'static,
    Message: Send + 'static,
    Theme: theme::Base,
    Renderer: crate::Renderer,
    Boot: Fn(&mut AppContext) -> (State, Task<Message>),
    Reduce: Fn(&mut State, Message, &mut AppContext) -> Dispatch<Message>,
    Compose: for<'a> Fn(&'a State, &WindowContext) -> ViewRoot<'a, Message, Theme, Renderer>,
{
    NativeApp {
        boot,
        reduce,
        compose,
        config: AppConfig {
            title: Cow::Borrowed(title),
            window: Some(window::Settings::default()),
            ..AppConfig::default()
        },
        types: PhantomData,
    }
}

impl<State, Message, Theme, Renderer, Boot, Reduce, Compose>
    NativeApp<State, Message, Theme, Renderer, Boot, Reduce, Compose>
{
    /// Use a custom runtime and renderer configuration.
    #[must_use]
    pub fn settings(mut self, settings: Settings) -> Self {
        self.config.settings = settings;
        self
    }

    /// Configure the primary window, retaining native ownership.
    #[must_use]
    pub fn window(mut self, settings: window::Settings) -> Self {
        self.config.window = Some(settings);
        self
    }

    /// Set a new static application title.
    #[must_use]
    pub fn title(mut self, title: &'static str) -> Self {
        self.config.title = Cow::Borrowed(title);
        self
    }
}

impl<State, Message, Theme, Renderer, Boot, Reduce, Compose> App
    for NativeApp<State, Message, Theme, Renderer, Boot, Reduce, Compose>
where
    State: 'static,
    Message: Send + 'static,
    Theme: theme::Base,
    Renderer: crate::Renderer,
    Boot: Fn(&mut AppContext) -> (State, Task<Message>),
    Reduce: Fn(&mut State, Message, &mut AppContext) -> Dispatch<Message>,
    Compose: for<'a> Fn(&'a State, &WindowContext) -> ViewRoot<'a, Message, Theme, Renderer>,
{
    type State = State;
    type Message = Message;
    type Theme = Theme;
    type Renderer = Renderer;
    type Executor = crate::futures::backend::default::Executor;

    fn name() -> &'static str {
        "TAIM Native"
    }

    fn controls_invalidation() -> bool {
        true
    }

    fn config(&self) -> AppConfig {
        self.config.clone()
    }

    fn boot(&self, context: &mut AppContext) -> (State, Task<Message>) {
        (self.boot)(context)
    }

    fn update(
        &self,
        state: &mut State,
        message: Message,
        context: &mut AppContext,
    ) -> Task<Message> {
        let dispatch = (self.reduce)(state, message, context);
        dispatch.apply(context);
        dispatch.task
    }

    fn view<'a>(
        &self,
        state: &'a State,
        context: &WindowContext,
    ) -> ViewRoot<'a, Message, Theme, Renderer> {
        (self.compose)(state, context)
    }

    fn title(&self, _state: &State, _context: &WindowContext) -> String {
        self.config.title.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppRunner;
    use crate::core::theme::Theme;
    use crate::core::widget::Tree;
    use crate::core::{Element, Layout, Length, Rectangle, Size, Widget, renderer};

    struct TestSurface;

    impl<Message, Theme, Renderer: crate::core::Renderer> Widget<Message, Theme, Renderer>
        for TestSurface
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
            crate::core::layout::Node::new(Size::new(80.0, 32.0))
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

    #[test]
    fn direct_app_runner_skips_clean_frame_and_repaints_changed_scene() {
        let app = app(
            "Native Intent Test",
            |_| (7_i32, Task::<i32>::none()),
            |state: &mut i32, change: i32, _context| {
                if change == 0 {
                    Dispatch::idle()
                } else {
                    *state += change;
                    Dispatch::paint()
                }
            },
            |_: &i32, _| Element::<i32, Theme, ()>::new(TestSurface),
        );
        let (mut runner, _boot_task) = AppRunner::new(app);
        assert!(!runner.is_legacy_compat());
        assert!(runner.controls_invalidation());
        let window = WindowContext::new(window::Id::unique(), 1.0);
        assert_eq!(runner.title(&window), "Native Intent Test");
        assert!(runner.app().config().window.is_some());
        assert_eq!(*runner.state(), 7);

        let _ = runner.update(0);
        assert_eq!(runner.context_mut().take_request(), FrameRequest::None);
        assert_eq!(*runner.state(), 7);

        let _ = runner.update(5);
        assert_eq!(runner.context_mut().take_request(), FrameRequest::Redraw);
        assert_eq!(*runner.state(), 12);
        let _scene = runner.view(&window);
    }

    #[test]
    fn animation_and_layout_decisions_drive_native_frame_requests() {
        let mut context = AppContext::new();
        Dispatch::<()>::animate().apply(&mut context);
        assert_eq!(context.take_request(), FrameRequest::Animate);

        Dispatch::<()>::layout().apply(&mut context);
        assert_eq!(context.take_request(), FrameRequest::Redraw);

        let zero_frame = Dispatch::<()> {
            task: Task::none(),
            delta: StateDelta {
                layout_dirty: true,
                paint_dirty: false,
                frame: FrameRequest::None,
            },
        };
        zero_frame.apply(&mut context);
        let pending = context.take_invalidation();
        assert_eq!(pending.frame, FrameRequest::None);
        assert!(pending.layout_dirty);
        assert!(!pending.paint_dirty);

        // An explicit animation has higher priority than a subsequent repaint.
        Dispatch::<()>::animate().apply(&mut context);
        Dispatch::<()>::paint().apply(&mut context);
        assert_eq!(context.take_request(), FrameRequest::Animate);
    }
}

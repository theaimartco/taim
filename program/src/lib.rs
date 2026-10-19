//! The definition of a TAIM application.
//!
//! TAIM applications are built around the native [`App`] contract.
//! The legacy [`Program`] contract is preserved in [`compat`] for backwards compatibility.

pub use taim_graphics as graphics;
pub use taim_runtime as runtime;
pub use taim_runtime::core;
pub use taim_runtime::futures;

pub mod app;
pub mod compat;
pub mod message;
pub mod native;
mod preset;

pub use app::*;
pub use compat::{
    Instance, Program, ProgramAdapter, with_executor, with_scale_factor, with_style,
    with_subscription, with_theme, with_title,
};
pub use preset::Preset;

use crate::core::Font;
use crate::core::renderer;
use crate::core::text;
use crate::graphics::compositor;

/// The renderer required by a TAIM [`App`] or legacy [`Program`].
pub trait Renderer: text::Renderer<Font = Font> + compositor::Default + renderer::Headless {}

impl<T> Renderer for T where
    T: text::Renderer<Font = Font> + compositor::Default + renderer::Headless
{
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::theme::Theme;
    use crate::core::widget::Tree;
    use crate::core::window;
    use crate::core::{Element, Layout, Length, Rectangle, Size, Widget};
    use crate::runtime::Task;

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

        fn boot(&self, _context: &mut AppContext) -> (Self::State, Task<Self::Message>) {
            (10, Task::none())
        }

        fn update(
            &self,
            state: &mut Self::State,
            message: Self::Message,
            context: &mut AppContext,
        ) -> Task<Self::Message> {
            *state += message;
            context.request_redraw();
            Task::none()
        }

        fn view<'a>(
            &self,
            _state: &'a Self::State,
            _window: &WindowContext,
        ) -> ViewRoot<'a, Self::Message, Self::Theme, Self::Renderer> {
            Element::new(TestWidget)
        }
    }

    struct LegacyCounterProgram;

    impl Program for LegacyCounterProgram {
        type State = i32;
        type Message = i32;
        type Theme = Theme;
        type Renderer = ();
        type Executor = futures::backend::default::Executor;

        fn name() -> &'static str {
            "legacy_counter"
        }

        fn settings(&self) -> crate::core::Settings {
            crate::core::Settings::default()
        }

        fn window(&self) -> Option<window::Settings> {
            None
        }

        fn boot(&self) -> (Self::State, Task<Self::Message>) {
            (50, Task::none())
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
    fn test_native_app_runner() {
        let app = CounterApp;
        let (mut runner, _task) = AppRunner::new(app);
        let win_ctx = WindowContext::new(window::Id::unique(), 1.5);

        assert_eq!(*runner.state(), 10);
        assert_eq!(runner.title(&win_ctx), "counter_app - Taim");
        assert_eq!(runner.scale_factor(&win_ctx), 1.0);

        let _task = runner.update(5);
        assert_eq!(*runner.state(), 15);
        assert_eq!(runner.context_mut().take_request(), FrameRequest::Redraw);

        let _task = runner.update(-3);
        assert_eq!(*runner.state(), 12);
    }

    #[test]
    fn test_program_adapter_and_instance() {
        let program = LegacyCounterProgram;
        // Legacy Program adapts into App via ProgramAdapter, driven by Instance
        let (mut instance, _task) = Instance::new(program);
        let win_id = window::Id::unique();

        assert_eq!(*instance.state(), 50);
        assert_eq!(instance.title(win_id), "Legacy Counter - Taim");

        let _task = instance.update(25);
        assert_eq!(*instance.state(), 75);
    }

    #[test]
    fn test_native_app_does_not_use_program_runtime() {
        let app = CounterApp;
        assert!(
            !CounterApp::is_legacy_compat(),
            "Native App must not be marked as legacy compat"
        );

        let (runner, _task) = AppRunner::new(app);
        assert!(
            !runner.is_legacy_compat(),
            "Native AppRunner must not be marked as legacy compat"
        );
    }

    #[test]
    fn test_legacy_program_executes_through_native_app_runtime() {
        let program = LegacyCounterProgram;
        let adapter = compat::ProgramAdapter::new(program);
        assert!(compat::ProgramAdapter::<LegacyCounterProgram>::is_legacy_compat());

        // Driven by native AppRunner directly!
        let (mut runner, _task) = AppRunner::new(adapter);
        assert!(runner.is_legacy_compat());

        let win_ctx = WindowContext::new(window::Id::unique(), 1.0);
        assert_eq!(runner.title(&win_ctx), "Legacy Counter - Taim");
        assert_eq!(*runner.state(), 50);

        let _ = runner.update(30);
        assert_eq!(*runner.state(), 80);
    }
}

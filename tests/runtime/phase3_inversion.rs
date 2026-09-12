//! Phase 3 Architecture and Runtime Inversion Verification Tests.
//!
//! Validates the end-to-end execution path:
//! Native App -> AppRunner -> Event Loop -> FrameGate -> TaskScheduler.
//! Validates that legacy Program execution adapts seamlessly through ProgramAdapter into the native path.

use taim::window;
use taim::{
    App, AppContext, AppRunner, Element, FrameRequest, Program, ProgramAdapter,
    Settings, Task, Theme, WindowContext,
};
use taim_winit::frame_gate::{DirtyFlags, FrameGate, PlatformEvent};

use taim::advanced::widget::{Tree, Widget};
use taim::advanced::{renderer, Layout};
use taim::{Length, Rectangle, Size};

struct TestWidget;

impl<Message, Theme, Renderer: taim::advanced::Renderer> Widget<Message, Theme, Renderer>
    for TestWidget
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        _limits: &taim::advanced::layout::Limits,
    ) -> taim::advanced::layout::Node {
        taim::advanced::layout::Node::new(Size::new(100.0, 50.0))
    }

    fn draw(
        &self,
        _tree: &Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: taim::advanced::mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }
}

struct MockApp;

impl App for MockApp {
    type State = u32;
    type Message = u32;
    type Theme = Theme;
    type Renderer = ();
    type Executor = taim::executor::Default;

    fn name() -> &'static str {
        "mock_app"
    }

    fn boot(&self, _context: &mut AppContext) -> (Self::State, Task<Self::Message>) {
        (100, Task::none())
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
    ) -> Element<'a, Self::Message, Self::Theme, Self::Renderer> {
        Element::new(TestWidget)
    }
}

struct MockLegacyProgram;

impl Program for MockLegacyProgram {
    type State = u32;
    type Message = u32;
    type Theme = Theme;
    type Renderer = ();
    type Executor = taim::executor::Default;

    fn name() -> &'static str {
        "mock_legacy_program"
    }

    fn settings(&self) -> Settings {
        Settings::default()
    }

    fn window(&self) -> Option<window::Settings> {
        None
    }

    fn boot(&self) -> (Self::State, Task<Self::Message>) {
        (200, Task::none())
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
fn test_native_app_does_not_use_program_runtime() {
    let app = MockApp;
    assert!(!MockApp::is_legacy_compat());

    let (mut runner, _task) = AppRunner::new(app);
    assert!(!runner.is_legacy_compat());
    assert_eq!(*runner.state(), 100);

    let win_ctx = WindowContext::new(window::Id::unique(), 1.0);
    assert_eq!(runner.title(&win_ctx), "mock_app - Taim");

    let _ = runner.update(50);
    assert_eq!(*runner.state(), 150);
    assert_eq!(runner.context_mut().take_request(), FrameRequest::Redraw);
}

#[test]
fn test_legacy_program_executes_through_native_app_runtime() {
    let program = MockLegacyProgram;
    let adapter = ProgramAdapter::new(program);
    assert!(ProgramAdapter::<MockLegacyProgram>::is_legacy_compat());

    let (mut runner, _task) = AppRunner::new(adapter);
    assert!(runner.is_legacy_compat());
    assert_eq!(*runner.state(), 200);

    let win_ctx = WindowContext::new(window::Id::unique(), 1.0);
    assert_eq!(runner.title(&win_ctx), "Mock Legacy Program - Taim");

    let _ = runner.update(75);
    assert_eq!(*runner.state(), 275);
}

#[test]
fn test_frame_gate_controls_real_redraw_path() {
    let mut gate = FrameGate::new();
    let id = window::Id::unique();
    gate.track_window(id, 1.0);
    gate.on_frame_presented(id);

    assert!(!gate.tracker(id).unwrap().should_render());

    gate.invalidate(id, DirtyFlags::PAINT_DIRTY);
    assert!(gate.tracker(id).unwrap().should_render());

    gate.on_frame_presented(id);
    assert!(!gate.tracker(id).unwrap().should_render());
}

#[test]
fn test_duplicate_redraws_result_in_one_render() {
    let mut gate = FrameGate::new();
    let id = window::Id::unique();
    gate.track_window(id, 1.0);
    gate.on_frame_presented(id);

    for _ in 0..10 {
        gate.invalidate(id, DirtyFlags::PAINT_DIRTY);
    }

    assert_eq!(gate.collapsed_redraws(), 9);
    assert!(gate.tracker(id).unwrap().should_render());

    gate.on_frame_presented(id);
    assert_eq!(gate.tracker(id).unwrap().dirty_flags, DirtyFlags::CLEAN);
    assert!(!gate.tracker(id).unwrap().should_render());
}

#[test]
fn test_clean_window_performs_no_render_work() {
    let mut gate = FrameGate::new();
    let id = window::Id::unique();
    gate.track_window(id, 1.0);
    gate.on_frame_presented(id);

    assert_eq!(gate.tracker(id).unwrap().dirty_flags, DirtyFlags::CLEAN);
    assert!(!gate.tracker(id).unwrap().should_render());
    assert_eq!(gate.dirty_windows().count(), 0);
}

#[test]
fn test_occluded_window_skips_render() {
    let mut gate = FrameGate::new();
    let id = window::Id::unique();
    gate.track_window(id, 1.0);
    gate.on_frame_presented(id);

    let _ = gate.handle_event(id, PlatformEvent::OcclusionChanged(true));
    gate.invalidate(id, DirtyFlags::PAINT_DIRTY);

    assert!(!gate.tracker(id).unwrap().should_render());
    assert_eq!(gate.dirty_windows().count(), 0);

    let _ = gate.handle_event(id, PlatformEvent::OcclusionChanged(false));
    assert!(gate.tracker(id).unwrap().should_render());
    assert_eq!(gate.dirty_windows().count(), 1);
}

#[test]
fn test_idle_runtime_has_zero_internal_frame_generation() {
    let mut gate = FrameGate::new();
    let id = window::Id::unique();
    gate.track_window(id, 1.0);
    gate.on_frame_presented(id);

    let initial_gen = gate.tracker(id).unwrap().frame_generation;
    assert!(gate.is_idle());
    assert!(gate.is_idle_now());
    assert!(!gate.has_immediate_work());
    assert_eq!(gate.next_wake_deadline(), None);
    assert_eq!(gate.tracker(id).unwrap().frame_generation, initial_gen);
}

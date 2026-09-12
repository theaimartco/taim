//! The native TAIM Application Architecture.
//!
//! An [`App`] is the primary execution contract for TAIM applications.
//! It is designed around direct configuration, fine-grained state management,
//! and native runtime contexts.

use crate::Renderer;
use crate::core::theme;
use crate::core::window;
use crate::core::{Element, Settings};
use crate::futures::{Executor, Subscription};
use crate::runtime::Task;

use std::borrow::Cow;

/// Marker trait representing valid application state.
pub trait AppState {}
impl<T> AppState for T {}

/// A frame pacing or presentation request emitted by an update cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrameRequest {
    /// No immediate redraw is needed.
    #[default]
    None,
    /// Request a redraw on the next frame presentation.
    Redraw,
    /// Request an immediate continuous animation frame.
    Animate,
}

/// A change delta or dirty mask emitted during state mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StateDelta {
    /// Whether layout requires re-computation.
    pub layout_dirty: bool,
    /// Whether paint/display list requires re-recording.
    pub paint_dirty: bool,
    /// Frame presentation request.
    pub frame: FrameRequest,
}

impl StateDelta {
    /// Creates a delta with no dirty flags set.
    pub const fn clean() -> Self {
        Self {
            layout_dirty: false,
            paint_dirty: false,
            frame: FrameRequest::None,
        }
    }

    /// Creates a delta indicating a visual update is needed.
    pub const fn redraw() -> Self {
        Self {
            layout_dirty: false,
            paint_dirty: true,
            frame: FrameRequest::Redraw,
        }
    }

    /// Creates a delta indicating both layout and visual updates are needed.
    pub const fn relayout() -> Self {
        Self {
            layout_dirty: true,
            paint_dirty: true,
            frame: FrameRequest::Redraw,
        }
    }
}

/// Context provided to application lifecycle hooks and state updates.
#[derive(Debug)]
pub struct AppContext {
    frame_count: u64,
    request: FrameRequest,
}

impl AppContext {
    /// Creates a new default [`AppContext`].
    pub fn new() -> Self {
        Self {
            frame_count: 0,
            request: FrameRequest::None,
        }
    }

    /// Returns the current monotonic frame counter.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Increments the frame count and returns the new value.
    pub fn tick(&mut self) -> u64 {
        self.frame_count = self.frame_count.wrapping_add(1);
        self.frame_count
    }

    /// Requests a redraw on the next frame.
    pub fn request_redraw(&mut self) {
        self.request = FrameRequest::Redraw;
    }

    /// Requests a continuous animation loop.
    pub fn request_animation(&mut self) {
        self.request = FrameRequest::Animate;
    }

    /// Takes the currently pending frame request.
    pub fn take_request(&mut self) -> FrameRequest {
        std::mem::take(&mut self.request)
    }
}

impl Default for AppContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Window-specific context passed to view and styling hooks.
#[derive(Debug, Clone)]
pub struct WindowContext {
    /// The unique window ID.
    pub id: window::Id,
    /// The current scale factor of the window.
    pub scale_factor: f32,
    /// Whether the window currently holds input focus.
    pub is_focused: bool,
    /// Whether the window is completely occluded or minimized.
    pub is_occluded: bool,
}

impl WindowContext {
    /// Creates a new [`WindowContext`].
    pub fn new(id: window::Id, scale_factor: f32) -> Self {
        Self {
            id,
            scale_factor,
            is_focused: true,
            is_occluded: false,
        }
    }
}

/// Global runtime context passed to subscriptions and environmental queries.
#[derive(Debug, Clone, Default)]
pub struct RuntimeContext {
    /// The operating system or desktop environment theme mode.
    pub system_theme: theme::Mode,
}

/// Direct, compact application configuration.
///
/// Replaces cascading decorator wrapper structs (`WithTitle`, `WithTheme`, etc.)
/// with a single flat configuration record.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Global application settings.
    pub settings: Settings,
    /// Initial primary window settings.
    pub window: Option<window::Settings>,
    /// Default window title prefix.
    pub title: Cow<'static, str>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            window: None,
            title: Cow::Borrowed("Taim App"),
        }
    }
}

/// Root view tree returned by an [`App`].
pub type ViewRoot<'a, Message, Theme, Renderer> = Element<'a, Message, Theme, Renderer>;

/// The primary application contract for TAIM.
///
/// Implement this trait to define your application's state, messages,
/// and reactive UI tree.
pub trait App: Sized {
    /// The state of the application.
    type State: AppState;

    /// The messages emitted by the application and widgets.
    type Message: Send + 'static;

    /// The theme used by the application.
    type Theme: theme::Base;

    /// The renderer used by the application.
    type Renderer: Renderer;

    /// The async executor used by the application.
    type Executor: Executor;

    /// Returns the unique name of the [`App`].
    fn name() -> &'static str;

    /// Returns the direct configuration for the application.
    fn config(&self) -> AppConfig {
        AppConfig {
            title: Cow::Borrowed(Self::name()),
            ..AppConfig::default()
        }
    }

    /// Boots the application, producing initial state and startup tasks.
    fn boot(&self, context: &mut AppContext) -> (Self::State, Task<Self::Message>);

    /// Handles an incoming message, mutating state and optionally returning background tasks.
    fn update(
        &self,
        state: &mut Self::State,
        message: Self::Message,
        context: &mut AppContext,
    ) -> Task<Self::Message>;

    /// Produces the root widget tree for the specified window.
    fn view<'a>(
        &self,
        state: &'a Self::State,
        window: &WindowContext,
    ) -> ViewRoot<'a, Self::Message, Self::Theme, Self::Renderer>;

    /// Produces the window title for the specified window.
    fn title(&self, _state: &Self::State, _window: &WindowContext) -> String {
        format!("{} - Taim", Self::name())
    }

    /// Declares external subscriptions active in the current state.
    fn subscription(
        &self,
        _state: &Self::State,
        _context: &RuntimeContext,
    ) -> Subscription<Self::Message> {
        Subscription::none()
    }

    /// Declares the theme override for the specified window.
    fn theme(&self, _state: &Self::State, _window: &WindowContext) -> Option<Self::Theme> {
        None
    }

    /// Declares the base style for the given theme.
    fn style(&self, _state: &Self::State, theme: &Self::Theme) -> theme::Style {
        theme::Base::base(theme)
    }

    /// Declares the scale factor for the specified window.
    fn scale_factor(&self, _state: &Self::State, _window: &WindowContext) -> f32 {
        1.0
    }

    /// Adapts this native [`App`] into a legacy [`Program`].
    fn into_program(self) -> crate::compat::AppToProgram<Self> {
        crate::compat::AppToProgram(self)
    }
}

/// The native execution runner for a TAIM [`App`].
///
/// Drives application state, context updates, frame requests, and rendering
/// directly without indirection through legacy structures.
pub struct AppRunner<A: App> {
    app: A,
    state: A::State,
    context: AppContext,
    runtime_context: RuntimeContext,
}

impl<A: App> AppRunner<A> {
    /// Boots a new [`AppRunner`] for the given [`App`].
    pub fn new(app: A) -> (Self, Task<A::Message>) {
        let mut context = AppContext::new();
        let (state, task) = app.boot(&mut context);

        (
            Self {
                app,
                state,
                context,
                runtime_context: RuntimeContext::default(),
            },
            task,
        )
    }

    /// Returns a reference to the inner [`App`].
    pub fn app(&self) -> &A {
        &self.app
    }

    /// Returns a reference to the application state.
    pub fn state(&self) -> &A::State {
        &self.state
    }

    /// Returns a mutable reference to the application state.
    pub fn state_mut(&mut self) -> &mut A::State {
        &mut self.state
    }

    /// Returns a reference to the application context.
    pub fn context(&self) -> &AppContext {
        &self.context
    }

    /// Returns a mutable reference to the application context.
    pub fn context_mut(&mut self) -> &mut AppContext {
        &mut self.context
    }

    /// Updates the application state with an incoming message.
    pub fn update(&mut self, message: A::Message) -> Task<A::Message> {
        self.app.update(&mut self.state, message, &mut self.context)
    }

    /// Produces the root view tree for the given window.
    pub fn view(&self, window: &WindowContext) -> ViewRoot<'_, A::Message, A::Theme, A::Renderer> {
        self.app.view(&self.state, window)
    }

    /// Produces the title for the given window.
    pub fn title(&self, window: &WindowContext) -> String {
        self.app.title(&self.state, window)
    }

    /// Produces the active event subscriptions.
    pub fn subscription(&self) -> Subscription<A::Message> {
        self.app.subscription(&self.state, &self.runtime_context)
    }

    /// Produces the theme for the given window.
    pub fn theme(&self, window: &WindowContext) -> Option<A::Theme> {
        self.app.theme(&self.state, window)
    }

    /// Produces the base style for the theme.
    pub fn style(&self, theme: &A::Theme) -> theme::Style {
        self.app.style(&self.state, theme)
    }

    /// Produces the scale factor for the given window.
    pub fn scale_factor(&self, window: &WindowContext) -> f32 {
        self.app.scale_factor(&self.state, window)
    }
}

use crate::core::window::{Direction, Icon, Id, Level, Mode, Screenshot, Settings, UserAttention};
use crate::core::{Point, Size, Window};
use crate::futures::futures::channel::oneshot;

/// An operation to be performed on some window.
pub enum Action {
    /// Open a new window.
    Open(Id, Settings, oneshot::Sender<Id>),
    /// Close a window.
    Close(Id),
    /// Get the oldest window identifier.
    GetOldest(oneshot::Sender<Option<Id>>),
    /// Get the newest window identifier.
    GetLatest(oneshot::Sender<Option<Id>>),
    /// Start an interactive window drag.
    Drag(Id),
    /// Start an interactive window resize.
    DragResize(Id, Direction),
    /// Resize a window.
    Resize(Id, Size),
    /// Get a window's logical size.
    GetSize(Id, oneshot::Sender<Size>),
    /// Get whether a window is maximized.
    GetMaximized(Id, oneshot::Sender<bool>),
    /// Change a window's maximized state.
    Maximize(Id, bool),
    /// Get whether a window is minimized.
    GetMinimized(Id, oneshot::Sender<Option<bool>>),
    /// Change a window's minimized state.
    Minimize(Id, bool),
    /// Get a window's logical position.
    GetPosition(Id, oneshot::Sender<Option<Point>>),
    /// Get a window's scale factor.
    GetScaleFactor(Id, oneshot::Sender<f32>),
    /// Move a window.
    Move(Id, Point),
    /// Change a window's mode.
    SetMode(Id, Mode),
    /// Get a window's mode.
    GetMode(Id, oneshot::Sender<Mode>),
    /// Toggle maximization.
    ToggleMaximize(Id),
    /// Toggle decorations.
    ToggleDecorations(Id),
    /// Request or clear user attention.
    RequestUserAttention(Id, Option<UserAttention>),
    /// Bring a window to the front and focus it.
    GainFocus(Id),
    /// Change the window level.
    SetLevel(Id, Level),
    /// Show the native system menu.
    ShowSystemMenu(Id),
    /// Get the platform window identifier.
    GetRawId(Id, oneshot::Sender<u64>),
    /// Change the window icon.
    SetIcon(Id, Icon),
    /// Run a callback against the platform window abstraction.
    Run(Id, Box<dyn FnOnce(&dyn Window) + Send>),
    /// Capture the window viewport.
    Screenshot(Id, oneshot::Sender<Screenshot>),
    /// Allow pointer input to pass through a window.
    EnableMousePassthrough(Id),
    /// Stop pointer input from passing through a window.
    DisableMousePassthrough(Id),
    /// Set the minimum inner size.
    SetMinSize(Id, Option<Size>),
    /// Set the maximum inner size.
    SetMaxSize(Id, Option<Size>),
    /// Change whether a window is resizable.
    SetResizable(Id, bool),
    /// Set logical resize increments.
    SetResizeIncrements(Id, Option<Size>),
    /// Get the size of the monitor containing a window.
    GetMonitorSize(Id, oneshot::Sender<Option<Size>>),
    /// Change macOS automatic tabbing behavior.
    SetAllowAutomaticTabbing(bool),
    /// Redraw every window.
    RedrawAll,
    /// Recompute every window layout.
    RelayoutAll,
}

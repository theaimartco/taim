use super::Action;
use crate::core::window::{Direction, Icon, Id, Level, Mode, Screenshot, Settings, UserAttention};
use crate::core::{Point, Size, Window};
use crate::task::{self, Task};

/// Opens a new window and returns its identifier with the completion task.
pub fn open(settings: Settings) -> (Id, Task<Id>) {
    let id = Id::unique();

    (
        id,
        task::oneshot(|channel| crate::Action::Window(Action::Open(id, settings, channel))),
    )
}

/// Closes a window.
pub fn close<T>(id: Id) -> Task<T> {
    task::effect(crate::Action::Window(Action::Close(id)))
}

/// Gets the oldest window identifier.
pub fn oldest() -> Task<Option<Id>> {
    task::oneshot(|channel| crate::Action::Window(Action::GetOldest(channel)))
}

/// Gets the newest window identifier.
pub fn latest() -> Task<Option<Id>> {
    task::oneshot(|channel| crate::Action::Window(Action::GetLatest(channel)))
}

/// Begins dragging a window.
pub fn drag<T>(id: Id) -> Task<T> {
    task::effect(crate::Action::Window(Action::Drag(id)))
}

/// Begins resizing a window.
pub fn drag_resize<T>(id: Id, direction: Direction) -> Task<T> {
    task::effect(crate::Action::Window(Action::DragResize(id, direction)))
}

/// Resizes a window to logical dimensions.
pub fn resize<T>(id: Id, new_size: Size) -> Task<T> {
    task::effect(crate::Action::Window(Action::Resize(id, new_size)))
}

/// Changes whether a window is resizable.
pub fn set_resizable<T>(id: Id, resizable: bool) -> Task<T> {
    task::effect(crate::Action::Window(Action::SetResizable(id, resizable)))
}

/// Sets a window's maximum inner size.
pub fn set_max_size<T>(id: Id, size: Option<Size>) -> Task<T> {
    task::effect(crate::Action::Window(Action::SetMaxSize(id, size)))
}

/// Sets a window's minimum inner size.
pub fn set_min_size<T>(id: Id, size: Option<Size>) -> Task<T> {
    task::effect(crate::Action::Window(Action::SetMinSize(id, size)))
}

/// Sets logical resize increments.
pub fn set_resize_increments<T>(id: Id, increments: Option<Size>) -> Task<T> {
    task::effect(crate::Action::Window(Action::SetResizeIncrements(
        id, increments,
    )))
}

/// Gets a window's logical size.
pub fn size(id: Id) -> Task<Size> {
    task::oneshot(move |channel| crate::Action::Window(Action::GetSize(id, channel)))
}

/// Gets whether a window is maximized.
pub fn is_maximized(id: Id) -> Task<bool> {
    task::oneshot(move |channel| crate::Action::Window(Action::GetMaximized(id, channel)))
}

/// Changes a window's maximized state.
pub fn maximize<T>(id: Id, maximized: bool) -> Task<T> {
    task::effect(crate::Action::Window(Action::Maximize(id, maximized)))
}

/// Gets whether a window is minimized.
pub fn is_minimized(id: Id) -> Task<Option<bool>> {
    task::oneshot(move |channel| crate::Action::Window(Action::GetMinimized(id, channel)))
}

/// Changes a window's minimized state.
pub fn minimize<T>(id: Id, minimized: bool) -> Task<T> {
    task::effect(crate::Action::Window(Action::Minimize(id, minimized)))
}

/// Gets a window's logical position.
pub fn position(id: Id) -> Task<Option<Point>> {
    task::oneshot(move |channel| crate::Action::Window(Action::GetPosition(id, channel)))
}

/// Gets a window's scale factor.
pub fn scale_factor(id: Id) -> Task<f32> {
    task::oneshot(move |channel| crate::Action::Window(Action::GetScaleFactor(id, channel)))
}

/// Moves a window to logical coordinates.
pub fn move_to<T>(id: Id, position: Point) -> Task<T> {
    task::effect(crate::Action::Window(Action::Move(id, position)))
}

/// Gets a window's mode.
pub fn mode(id: Id) -> Task<Mode> {
    task::oneshot(move |channel| crate::Action::Window(Action::GetMode(id, channel)))
}

/// Changes a window's mode.
pub fn set_mode<T>(id: Id, mode: Mode) -> Task<T> {
    task::effect(crate::Action::Window(Action::SetMode(id, mode)))
}

/// Toggles window maximization.
pub fn toggle_maximize<T>(id: Id) -> Task<T> {
    task::effect(crate::Action::Window(Action::ToggleMaximize(id)))
}

/// Toggles window decorations.
pub fn toggle_decorations<T>(id: Id) -> Task<T> {
    task::effect(crate::Action::Window(Action::ToggleDecorations(id)))
}

/// Requests or clears user attention.
pub fn request_user_attention<T>(id: Id, user_attention: Option<UserAttention>) -> Task<T> {
    task::effect(crate::Action::Window(Action::RequestUserAttention(
        id,
        user_attention,
    )))
}

/// Brings a window to the front and focuses it.
pub fn gain_focus<T>(id: Id) -> Task<T> {
    task::effect(crate::Action::Window(Action::GainFocus(id)))
}

/// Changes a window's level.
pub fn set_level<T>(id: Id, level: Level) -> Task<T> {
    task::effect(crate::Action::Window(Action::SetLevel(id, level)))
}

/// Shows the native system menu.
pub fn show_system_menu<T>(id: Id) -> Task<T> {
    task::effect(crate::Action::Window(Action::ShowSystemMenu(id)))
}

/// Gets the identifier from the underlying window system.
pub fn raw_id<Message>(id: Id) -> Task<u64> {
    task::oneshot(move |channel| crate::Action::Window(Action::GetRawId(id, channel)))
}

/// Changes a window icon.
pub fn set_icon<T>(id: Id, icon: Icon) -> Task<T> {
    task::effect(crate::Action::Window(Action::SetIcon(id, icon)))
}

/// Runs a callback with a window abstraction.
pub fn run<T>(id: Id, f: impl FnOnce(&dyn Window) -> T + Send + 'static) -> Task<T>
where
    T: Send + 'static,
{
    task::oneshot(move |channel| {
        crate::Action::Window(Action::Run(
            id,
            Box::new(move |handle| {
                let _ = channel.send(f(handle));
            }),
        ))
    })
}

/// Captures a window screenshot.
pub fn screenshot(id: Id) -> Task<Screenshot> {
    task::oneshot(move |channel| crate::Action::Window(Action::Screenshot(id, channel)))
}

/// Enables pointer passthrough.
pub fn enable_mouse_passthrough<Message>(id: Id) -> Task<Message> {
    task::effect(crate::Action::Window(Action::EnableMousePassthrough(id)))
}

/// Disables pointer passthrough.
pub fn disable_mouse_passthrough<Message>(id: Id) -> Task<Message> {
    task::effect(crate::Action::Window(Action::DisableMousePassthrough(id)))
}

/// Gets the size of the monitor containing a window.
pub fn monitor_size(id: Id) -> Task<Option<Size>> {
    task::oneshot(move |channel| crate::Action::Window(Action::GetMonitorSize(id, channel)))
}

/// Changes whether macOS can automatically organize windows into tabs.
pub fn allow_automatic_tabbing<T>(enabled: bool) -> Task<T> {
    task::effect(crate::Action::Window(Action::SetAllowAutomaticTabbing(
        enabled,
    )))
}

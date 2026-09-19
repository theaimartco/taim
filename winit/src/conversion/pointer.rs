use super::*;

/// Converts a [`mouse::Interaction`] into a [`winit`] cursor icon.
///
/// [`winit`]: https://github.com/rust-windowing/winit
pub fn mouse_interaction(interaction: mouse::Interaction) -> Option<winit::window::CursorIcon> {
    use mouse::Interaction;

    let icon = match interaction {
        Interaction::Hidden => {
            return None;
        }
        Interaction::None | Interaction::Idle => winit::window::CursorIcon::Default,
        Interaction::ContextMenu => winit::window::CursorIcon::ContextMenu,
        Interaction::Help => winit::window::CursorIcon::Help,
        Interaction::Pointer => winit::window::CursorIcon::Pointer,
        Interaction::Progress => winit::window::CursorIcon::Progress,
        Interaction::Wait => winit::window::CursorIcon::Wait,
        Interaction::Cell => winit::window::CursorIcon::Cell,
        Interaction::Crosshair => winit::window::CursorIcon::Crosshair,
        Interaction::Text => winit::window::CursorIcon::Text,
        Interaction::Alias => winit::window::CursorIcon::Alias,
        Interaction::Copy => winit::window::CursorIcon::Copy,
        Interaction::Move => winit::window::CursorIcon::Move,
        Interaction::NoDrop => winit::window::CursorIcon::NoDrop,
        Interaction::NotAllowed => winit::window::CursorIcon::NotAllowed,
        Interaction::Grab => winit::window::CursorIcon::Grab,
        Interaction::Grabbing => winit::window::CursorIcon::Grabbing,
        Interaction::ResizingHorizontally => winit::window::CursorIcon::EwResize,
        Interaction::ResizingVertically => winit::window::CursorIcon::NsResize,
        Interaction::ResizingDiagonallyUp => winit::window::CursorIcon::NeswResize,
        Interaction::ResizingDiagonallyDown => winit::window::CursorIcon::NwseResize,
        Interaction::ResizingColumn => winit::window::CursorIcon::ColResize,
        Interaction::ResizingRow => winit::window::CursorIcon::RowResize,
        Interaction::AllScroll => winit::window::CursorIcon::AllScroll,
        Interaction::ZoomIn => winit::window::CursorIcon::ZoomIn,
        Interaction::ZoomOut => winit::window::CursorIcon::ZoomOut,
    };

    Some(icon)
}

/// Converts a `MouseButton` from [`winit`] to an [`taim`] mouse button.
///
/// [`winit`]: https://github.com/rust-windowing/winit
/// [`taim`]: https://github.com/theaimartco/taim/tree/0.12
pub fn mouse_button(mouse_button: winit::event::MouseButton) -> mouse::Button {
    match mouse_button {
        winit::event::MouseButton::Left => mouse::Button::Left,
        winit::event::MouseButton::Right => mouse::Button::Right,
        winit::event::MouseButton::Middle => mouse::Button::Middle,
        winit::event::MouseButton::Back => mouse::Button::Back,
        winit::event::MouseButton::Forward => mouse::Button::Forward,
        winit::event::MouseButton::Other(other) => mouse::Button::Other(other),
    }
}

/// Converts some `ModifiersState` from [`winit`] to an [`taim`] modifiers
/// state.
///
/// [`winit`]: https://github.com/rust-windowing/winit
/// [`taim`]: https://github.com/theaimartco/taim/tree/0.12
pub fn modifiers(modifiers: winit::keyboard::ModifiersState) -> keyboard::Modifiers {
    let mut result = keyboard::Modifiers::empty();

    result.set(keyboard::Modifiers::SHIFT, modifiers.shift_key());
    result.set(keyboard::Modifiers::CTRL, modifiers.control_key());
    result.set(keyboard::Modifiers::ALT, modifiers.alt_key());
    result.set(keyboard::Modifiers::LOGO, modifiers.super_key());

    result
}

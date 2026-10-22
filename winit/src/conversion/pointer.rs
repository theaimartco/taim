//! Pointer and modifier vocabulary between TAIM and `winit`.
use super::*;

use winit::window::CursorIcon as Icon;

/// The system cursor TAIM shows for `interaction`, or `None` to hide it.
pub fn mouse_interaction(interaction: mouse::Interaction) -> Option<Icon> {
    use mouse::Interaction as I;

    Some(match interaction {
        I::Hidden => return None,
        I::None | I::Idle => Icon::Default,

        // Status
        I::ContextMenu => Icon::ContextMenu,
        I::Help => Icon::Help,
        I::Progress => Icon::Progress,
        I::Wait => Icon::Wait,

        // Selection and activation
        I::Pointer => Icon::Pointer,
        I::Cell => Icon::Cell,
        I::Crosshair => Icon::Crosshair,
        I::Text => Icon::Text,

        // Drag and drop
        I::Alias => Icon::Alias,
        I::Copy => Icon::Copy,
        I::Move => Icon::Move,
        I::NoDrop => Icon::NoDrop,
        I::NotAllowed => Icon::NotAllowed,
        I::Grab => Icon::Grab,
        I::Grabbing => Icon::Grabbing,
        I::AllScroll => Icon::AllScroll,

        // Resizing: TAIM names the gesture, the platform names the arrow.
        I::ResizingHorizontally => Icon::EwResize,
        I::ResizingVertically => Icon::NsResize,
        I::ResizingDiagonallyUp => Icon::NeswResize,
        I::ResizingDiagonallyDown => Icon::NwseResize,
        I::ResizingColumn => Icon::ColResize,
        I::ResizingRow => Icon::RowResize,

        // Magnification
        I::ZoomIn => Icon::ZoomIn,
        I::ZoomOut => Icon::ZoomOut,
    })
}

/// Translates a platform mouse button.
pub fn mouse_button(button: winit::event::MouseButton) -> mouse::Button {
    use winit::event::MouseButton as B;

    match button {
        B::Left => mouse::Button::Left,
        B::Right => mouse::Button::Right,
        B::Middle => mouse::Button::Middle,
        B::Back => mouse::Button::Back,
        B::Forward => mouse::Button::Forward,
        B::Other(code) => mouse::Button::Other(code),
    }
}

/// Translates the platform's held-modifier state.
pub fn modifiers(state: winit::keyboard::ModifiersState) -> keyboard::Modifiers {
    use keyboard::Modifiers as M;

    [
        (M::SHIFT, state.shift_key()),
        (M::CTRL, state.control_key()),
        (M::ALT, state.alt_key()),
        (M::LOGO, state.super_key()),
    ]
    .into_iter()
    .filter(|(_, held)| *held)
    .fold(M::empty(), |held, (modifier, _)| held | modifier)
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::ModifiersState;

    #[test]
    fn hidden_cursor_has_no_icon() {
        assert_eq!(mouse_interaction(mouse::Interaction::Hidden), None);
        assert_eq!(
            mouse_interaction(mouse::Interaction::Idle),
            Some(Icon::Default)
        );
    }

    #[test]
    fn modifiers_keep_every_held_key() {
        let held = modifiers(ModifiersState::SHIFT | ModifiersState::SUPER);

        assert!(held.shift());
        assert!(held.logo());
        assert!(!held.control());
        assert!(!held.alt());
    }
}

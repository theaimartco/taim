use crate::core::mouse;
use crate::core::touch;
use crate::core::widget::Tree;
use crate::core::{Event, Layout, Point, Rectangle, Shell};

use super::{MouseArea, Trigger};

/// Retained pointer state for a [`MouseArea`].
#[derive(Default)]
pub(super) struct State {
    is_hovered: bool,
    bounds: Rectangle,
    cursor_position: Option<Point>,
    previous_click: Option<mouse::Click>,
}

/// Routes an uncaptured pointer event through a mouse area.
pub(super) fn update<Message: Clone, Theme, Renderer>(
    widget: &MouseArea<'_, Message, Theme, Renderer>,
    tree: &mut Tree,
    event: &Event,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    shell: &mut Shell<'_, Message>,
) {
    let state: &mut State = tree.state.downcast_mut();
    let cursor_position = cursor.position();
    let bounds = layout.bounds();
    let is_hovered = cursor.is_over(bounds);

    if state.cursor_position != cursor_position || state.bounds != bounds {
        let was_hovered = state.is_hovered;

        state.is_hovered = is_hovered;
        state.cursor_position = cursor_position;
        state.bounds = bounds;

        match (
            widget.handler(Trigger::Enter),
            widget.on_move.as_ref(),
            widget.handler(Trigger::Exit),
        ) {
            (Some(on_enter), _, _) if is_hovered && !was_hovered => {
                shell.publish(on_enter.clone());
            }
            (_, Some(on_move), _) if is_hovered => {
                if let Some(position) = cursor.position_in(bounds) {
                    shell.publish(on_move(position));
                }
            }
            (_, _, Some(on_exit)) if !is_hovered && was_hovered => {
                shell.publish(on_exit.clone());
            }
            _ => {}
        }
    }

    if !is_hovered {
        return;
    }

    match event {
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        | Event::Touch(touch::Event::FingerPressed { .. }) => {
            if let Some(message) = widget.handler(Trigger::Press) {
                shell.publish(message.clone());
                shell.capture_event();
            }

            if let Some(position) = cursor_position
                && let Some(message) = widget.handler(Trigger::DoubleClick)
            {
                let click = mouse::Click::new(position, mouse::Button::Left, state.previous_click);

                if click.kind() == mouse::click::Kind::Double {
                    shell.publish(message.clone());
                }

                state.previous_click = Some(click);
                shell.capture_event();
            }
        }
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
        | Event::Touch(touch::Event::FingerLifted { .. }) => {
            if let Some(message) = widget.handler(Trigger::Release) {
                shell.publish(message.clone());
                shell.capture_event();
            }
        }
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
            if let Some(message) = widget.handler(Trigger::RightPress) {
                shell.publish(message.clone());
                shell.capture_event();
            }
        }
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Right)) => {
            if let Some(message) = widget.handler(Trigger::RightRelease) {
                shell.publish(message.clone());
                shell.capture_event();
            }
        }
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)) => {
            if let Some(message) = widget.handler(Trigger::MiddlePress) {
                shell.publish(message.clone());
                shell.capture_event();
            }
        }
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)) => {
            if let Some(message) = widget.handler(Trigger::MiddleRelease) {
                shell.publish(message.clone());
                shell.capture_event();
            }
        }
        Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
            if let Some(on_scroll) = widget.on_scroll.as_ref() {
                shell.publish(on_scroll(*delta));
                shell.capture_event();
            }
        }
        _ => {}
    }
}

//! Small pieces shared by the built-in views.
use crate::cx::PaintCx;
use crate::{Border, Color, Event, Rectangle, keyboard, mouse, semantics};

/// Width of the keyboard-focus outline.
pub(crate) const FOCUS_RING: f32 = 2.0;

/// Draws the keyboard-focus outline around `bounds`.
pub(crate) fn focus_ring(cx: &mut PaintCx<'_>, bounds: Rectangle, radius: f32) {
    let color = cx.theme().palette().primary.strong.color;
    let ring = bounds.expand(FOCUS_RING + 1.0);

    cx.fill(
        ring,
        Color::TRANSPARENT,
        Border::default()
            .width(FOCUS_RING)
            .color(color)
            .rounded(radius + FOCUS_RING + 1.0),
    );
}

/// Whether `event` is a key that activates the focused control
/// (Space or Enter).
pub(crate) fn is_activation(event: &Event) -> bool {
    use keyboard::key::Named;

    matches!(
        event,
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(Named::Space | Named::Enter),
            ..
        })
    )
}

/// Whether `event` is a primary click starting over the view.
pub(crate) fn is_press(event: &Event) -> bool {
    matches!(
        event,
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
    )
}

/// Whether `event` ends a primary click.
pub(crate) fn is_release(event: &Event) -> bool {
    matches!(
        event,
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
    )
}

/// The common semantic state flags of an interactive view.
pub(crate) fn interactive_state(enabled: bool, focused: bool) -> semantics::State {
    let mut state = semantics::State::FOCUSABLE;
    state.set(semantics::State::DISABLED, !enabled);
    state.set(semantics::State::FOCUSED, focused);
    state
}

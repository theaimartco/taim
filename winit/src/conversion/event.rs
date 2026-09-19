//! TAIM-owned Winit event normalization.
//!
//! Platform events enter here once and leave in logical UI coordinates. The
//! window state already owns normalized modifiers, so keyboard bursts do not
//! rebuild the same bitset for every key event.

use super::{key, mouse_button, physical_key};
use crate::core::input_method;
use crate::core::keyboard;
use crate::core::mouse;
use crate::core::pointer;
use crate::core::touch;
use crate::core::window;
use crate::core::{Event, Point, Size};

/// Converts a Winit window event into a normalized TAIM event.
pub fn window_event(
    event: winit::event::WindowEvent,
    scale_factor: f32,
    modifiers: keyboard::Modifiers,
) -> Option<Event> {
    use winit::event::{ElementState, Ime, MouseScrollDelta, WindowEvent};

    match event {
        WindowEvent::Resized(size) => {
            let scale = logical_scale(scale_factor);
            Some(Event::Window(window::Event::Resized(Size::new(
                size.width as f32 * scale,
                size.height as f32 * scale,
            ))))
        }
        WindowEvent::CloseRequested => Some(Event::Window(window::Event::CloseRequested)),
        WindowEvent::CursorMoved { position, .. } => {
            Some(Event::Mouse(mouse::Event::CursorMoved {
                position: cursor_position(position, scale_factor),
            }))
        }
        WindowEvent::CursorEntered { .. } => Some(Event::Mouse(mouse::Event::CursorEntered)),
        WindowEvent::CursorLeft { .. } => Some(Event::Mouse(mouse::Event::CursorLeft)),
        WindowEvent::MouseInput { button, state, .. } => {
            let button = mouse_button(button);
            Some(Event::Mouse(match state {
                ElementState::Pressed => mouse::Event::ButtonPressed(button),
                ElementState::Released => mouse::Event::ButtonReleased(button),
            }))
        }
        WindowEvent::MouseWheel { delta, .. } => {
            let delta = match delta {
                MouseScrollDelta::LineDelta(x, y) => mouse::ScrollDelta::Lines { x, y },
                MouseScrollDelta::PixelDelta(position) => {
                    let scale = f64::from(logical_scale(scale_factor));
                    mouse::ScrollDelta::Pixels {
                        x: (position.x * scale) as f32,
                        y: (position.y * scale) as f32,
                    }
                }
            };

            Some(Event::Mouse(mouse::Event::WheelScrolled { delta }))
        }
        WindowEvent::KeyboardInput {
            is_synthetic: true, ..
        } => None,
        WindowEvent::KeyboardInput { event, .. } => {
            Some(Event::Keyboard(keyboard_event(event, modifiers)))
        }
        WindowEvent::ModifiersChanged(new_modifiers) => Some(Event::Keyboard(
            keyboard::Event::ModifiersChanged(super::modifiers(new_modifiers.state())),
        )),
        WindowEvent::Ime(event) => Some(Event::InputMethod(match event {
            Ime::Enabled => input_method::Event::Opened,
            Ime::Preedit(content, selection) => {
                input_method::Event::Preedit(content, selection.map(|(start, end)| start..end))
            }
            Ime::Commit(content) => input_method::Event::Commit(content),
            Ime::Disabled => input_method::Event::Closed,
        })),
        WindowEvent::Focused(true) => Some(Event::Window(window::Event::Focused)),
        WindowEvent::Focused(false) => Some(Event::Window(window::Event::Unfocused)),
        WindowEvent::HoveredFile(path) => Some(Event::Window(window::Event::FileHovered(path))),
        WindowEvent::DroppedFile(path) => Some(Event::Window(window::Event::FileDropped(path))),
        WindowEvent::HoveredFileCancelled => Some(Event::Window(window::Event::FilesHoveredLeft)),
        WindowEvent::Touch(touch) => Some(Event::Touch(touch_event(touch, scale_factor))),
        WindowEvent::Moved(position) => {
            let scale = logical_scale(scale_factor);
            Some(Event::Window(window::Event::Moved(Point::new(
                position.x as f32 * scale,
                position.y as f32 * scale,
            ))))
        }
        WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
            let scale_factor = scale_factor as f32;
            Some(Event::Window(window::Event::Rescaled(
                if scale_factor.is_finite() && scale_factor > 0.0 {
                    scale_factor
                } else {
                    1.0
                },
            )))
        }
        _ => None,
    }
}

/// Converts a physical cursor coordinate into TAIM logical points.
#[inline]
pub fn cursor_position(position: winit::dpi::PhysicalPosition<f64>, scale_factor: f32) -> Point {
    let scale = f64::from(logical_scale(scale_factor));
    Point::new((position.x * scale) as f32, (position.y * scale) as f32)
}

fn keyboard_event(
    event: winit::event::KeyEvent,
    modifiers: keyboard::Modifiers,
) -> keyboard::Event {
    let unmodified_key = {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
            event.key_without_modifiers()
        }

        #[cfg(target_arch = "wasm32")]
        {
            event.logical_key.clone()
        }
    };

    let text = {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use crate::core::SmolStr;
            use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
            event.text_with_all_modifiers().map(SmolStr::new)
        }

        #[cfg(target_arch = "wasm32")]
        {
            event.text
        }
    }
    .filter(|text| !contains_private_use(text.as_str()));

    let winit::event::KeyEvent {
        state,
        location,
        logical_key,
        physical_key: native_physical_key,
        repeat,
        ..
    } = event;

    let unmodified_key = key(unmodified_key);
    let modified_key = key(logical_key);
    let physical_key = physical_key(native_physical_key);
    let location = match location {
        winit::keyboard::KeyLocation::Standard => keyboard::Location::Standard,
        winit::keyboard::KeyLocation::Left => keyboard::Location::Left,
        winit::keyboard::KeyLocation::Right => keyboard::Location::Right,
        winit::keyboard::KeyLocation::Numpad => keyboard::Location::Numpad,
    };

    match state {
        winit::event::ElementState::Pressed => keyboard::Event::KeyPressed {
            key: unmodified_key,
            modified_key,
            physical_key,
            modifiers,
            location,
            text,
            repeat,
        },
        winit::event::ElementState::Released => keyboard::Event::KeyReleased {
            key: unmodified_key,
            modified_key,
            physical_key,
            modifiers,
            location,
        },
    }
}

fn touch_event(touch: winit::event::Touch, scale_factor: f32) -> touch::Event {
    let id = touch::Finger(touch.id);
    let position = cursor_position(touch.location, scale_factor);
    let metadata = pointer::Metadata {
        pressure: touch.force.map(|force| force.normalized() as f32),
        tilt: None,
        contact_size: None,
    };

    match touch.phase {
        winit::event::TouchPhase::Started => touch::Event::FingerPressed {
            id,
            position,
            metadata,
        },
        winit::event::TouchPhase::Moved => touch::Event::FingerMoved {
            id,
            position,
            metadata,
        },
        winit::event::TouchPhase::Ended => touch::Event::FingerLifted {
            id,
            position,
            metadata,
        },
        winit::event::TouchPhase::Cancelled => touch::Event::FingerLost {
            id,
            position,
            metadata,
        },
    }
}

#[inline]
fn logical_scale(scale_factor: f32) -> f32 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor.recip()
    } else {
        1.0
    }
}

#[inline]
fn contains_private_use(text: &str) -> bool {
    !text.is_ascii()
        && text.chars().any(|character| {
            matches!(
                character,
                '\u{E000}'..='\u{F8FF}'
                    | '\u{F0000}'..='\u{FFFFD}'
                    | '\u{100000}'..='\u{10FFFD}'
            )
        })
}

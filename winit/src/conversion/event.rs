//! TAIM-owned `winit` event normalization.
//!
//! Platform events enter here once and leave in logical UI coordinates.
//! Each input family (window lifecycle, pointer, keyboard, input method,
//! touch) has its own translator; anything none of them claims is dropped.
//! The window state already owns normalized modifiers, so keyboard bursts do
//! not rebuild the same bitset for every key.
use super::{key, mouse_button, physical_key};
use crate::core::{Event, Point, Size};
use crate::core::{input_method, keyboard, mouse, pointer, touch, window};

use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, WindowEvent};

/// Translates a platform window event into a TAIM event, if TAIM has one.
pub fn window_event(
    event: WindowEvent,
    scale_factor: f32,
    modifiers: keyboard::Modifiers,
) -> Option<Event> {
    let to_logical = logical_scale(scale_factor);

    match event {
        WindowEvent::KeyboardInput {
            is_synthetic,
            event,
            ..
        } => {
            // Synthetic presses replay keys held while the window was
            // unfocused; acting on them would trigger shortcuts twice.
            (!is_synthetic).then(|| Event::Keyboard(keyboard_event(event, modifiers)))
        }
        WindowEvent::ModifiersChanged(state) => Some(Event::Keyboard(
            keyboard::Event::ModifiersChanged(super::modifiers(state.state())),
        )),
        WindowEvent::Ime(ime) => Some(Event::InputMethod(input_method_event(ime))),
        WindowEvent::Touch(contact) => Some(Event::Touch(touch_event(contact, scale_factor))),
        event => pointer_event(&event, scale_factor)
            .map(Event::Mouse)
            .or_else(|| lifecycle_event(event, to_logical).map(Event::Window)),
    }
}

/// Converts a physical cursor coordinate into TAIM logical points.
#[inline]
pub fn cursor_position(position: PhysicalPosition<f64>, scale_factor: f32) -> Point {
    let to_logical = f64::from(logical_scale(scale_factor));

    Point::new(
        (position.x * to_logical) as f32,
        (position.y * to_logical) as f32,
    )
}

/// Geometry, focus, file drag-and-drop and close requests.
fn lifecycle_event(event: WindowEvent, to_logical: f32) -> Option<window::Event> {
    Some(match event {
        WindowEvent::Resized(physical) => window::Event::Resized(Size::new(
            physical.width as f32 * to_logical,
            physical.height as f32 * to_logical,
        )),
        WindowEvent::Moved(physical) => window::Event::Moved(Point::new(
            physical.x as f32 * to_logical,
            physical.y as f32 * to_logical,
        )),
        WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
            let factor = scale_factor as f32;
            window::Event::Rescaled(if factor.is_finite() && factor > 0.0 {
                factor
            } else {
                1.0
            })
        }
        WindowEvent::Focused(focused) => {
            if focused {
                window::Event::Focused
            } else {
                window::Event::Unfocused
            }
        }
        WindowEvent::CloseRequested => window::Event::CloseRequested,
        WindowEvent::HoveredFile(path) => window::Event::FileHovered(path),
        WindowEvent::DroppedFile(path) => window::Event::FileDropped(path),
        WindowEvent::HoveredFileCancelled => window::Event::FilesHoveredLeft,
        _ => return None,
    })
}

/// Cursor motion, presence, buttons and the wheel.
fn pointer_event(event: &WindowEvent, scale_factor: f32) -> Option<mouse::Event> {
    use winit::event::MouseScrollDelta;

    Some(match *event {
        WindowEvent::CursorMoved { position, .. } => mouse::Event::CursorMoved {
            position: cursor_position(position, scale_factor),
        },
        WindowEvent::CursorEntered { .. } => mouse::Event::CursorEntered,
        WindowEvent::CursorLeft { .. } => mouse::Event::CursorLeft,
        WindowEvent::MouseInput { button, state, .. } => {
            let button = mouse_button(button);

            if state == ElementState::Pressed {
                mouse::Event::ButtonPressed(button)
            } else {
                mouse::Event::ButtonReleased(button)
            }
        }
        WindowEvent::MouseWheel { delta, .. } => mouse::Event::WheelScrolled {
            delta: match delta {
                MouseScrollDelta::LineDelta(x, y) => mouse::ScrollDelta::Lines { x, y },
                MouseScrollDelta::PixelDelta(physical) => {
                    let logical = cursor_position(physical, scale_factor);
                    mouse::ScrollDelta::Pixels {
                        x: logical.x,
                        y: logical.y,
                    }
                }
            },
        },
        _ => return None,
    })
}

fn input_method_event(ime: winit::event::Ime) -> input_method::Event {
    use winit::event::Ime;

    match ime {
        Ime::Enabled => input_method::Event::Opened,
        Ime::Preedit(text, cursor) => {
            input_method::Event::Preedit(text, cursor.map(|(from, to)| from..to))
        }
        Ime::Commit(text) => input_method::Event::Commit(text),
        Ime::Disabled => input_method::Event::Closed,
    }
}

fn keyboard_event(
    event: winit::event::KeyEvent,
    modifiers: keyboard::Modifiers,
) -> keyboard::Event {
    // The layout-independent key ("the key labeled S", even with Shift
    // held) drives shortcuts; the modified key and text drive typing.
    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
    let (bare_key, text) = {
        use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;

        (
            event.key_without_modifiers(),
            event
                .text_with_all_modifiers()
                .map(crate::core::SmolStr::new),
        )
    };

    #[cfg(any(target_arch = "wasm32", target_os = "android"))]
    let (bare_key, text) = (event.logical_key.clone(), event.text.clone());

    // Function keys on some platforms produce private-use glyphs as "text";
    // they are not something a user typed.
    let text = text.filter(|text| !contains_private_use(text));

    let modified_key = key(event.logical_key);
    let bare_key = key(bare_key);
    let physical_key = physical_key(event.physical_key);
    let location = key_location(event.location);

    match event.state {
        ElementState::Pressed => keyboard::Event::KeyPressed {
            key: bare_key,
            modified_key,
            physical_key,
            modifiers,
            location,
            text,
            repeat: event.repeat,
        },
        ElementState::Released => keyboard::Event::KeyReleased {
            key: bare_key,
            modified_key,
            physical_key,
            modifiers,
            location,
        },
    }
}

fn key_location(location: winit::keyboard::KeyLocation) -> keyboard::Location {
    use winit::keyboard::KeyLocation as Zone;

    match location {
        Zone::Standard => keyboard::Location::Standard,
        Zone::Left => keyboard::Location::Left,
        Zone::Right => keyboard::Location::Right,
        Zone::Numpad => keyboard::Location::Numpad,
    }
}

fn touch_event(contact: winit::event::Touch, scale_factor: f32) -> touch::Event {
    use winit::event::TouchPhase;

    let id = touch::Finger(contact.id);
    let position = cursor_position(contact.location, scale_factor);
    let metadata = pointer::Metadata {
        pressure: contact.force.map(|force| force.normalized() as f32),
        tilt: None,
        contact_size: None,
    };

    match contact.phase {
        TouchPhase::Started => touch::Event::FingerPressed {
            id,
            position,
            metadata,
        },
        TouchPhase::Moved => touch::Event::FingerMoved {
            id,
            position,
            metadata,
        },
        TouchPhase::Ended => touch::Event::FingerLifted {
            id,
            position,
            metadata,
        },
        TouchPhase::Cancelled => touch::Event::FingerLost {
            id,
            position,
            metadata,
        },
    }
}

/// Multiplier from physical pixels to logical points; a broken scale factor
/// degrades to 1:1 instead of producing infinities.
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
                '\u{E000}'..='\u{F8FF}' | '\u{F0000}'..='\u{FFFFD}' | '\u{100000}'..='\u{10FFFD}'
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broken_scale_factors_fall_back_to_one_to_one() {
        for broken in [0.0, -2.0, f32::NAN, f32::INFINITY] {
            assert_eq!(logical_scale(broken), 1.0);
        }

        assert_eq!(
            cursor_position(PhysicalPosition::new(200.0, 100.0), 2.0),
            Point::new(100.0, 50.0)
        );
    }

    #[test]
    fn private_use_glyphs_are_not_text() {
        assert!(contains_private_use("\u{F704}"));
        assert!(!contains_private_use("é"));
        assert!(!contains_private_use("a"));
    }

    #[test]
    fn focus_and_close_reach_the_window() {
        assert_eq!(
            window_event(
                WindowEvent::Focused(false),
                1.0,
                keyboard::Modifiers::empty()
            ),
            Some(Event::Window(window::Event::Unfocused))
        );
        assert_eq!(
            window_event(
                WindowEvent::CloseRequested,
                1.0,
                keyboard::Modifiers::empty()
            ),
            Some(Event::Window(window::Event::CloseRequested))
        );
    }
}

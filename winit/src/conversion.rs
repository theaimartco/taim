//! Translation between `winit` types and TAIM types.
//!
//! Every submodule covers one vocabulary: `keymap` for keys, `pointer` for
//! buttons, cursors and modifiers, `event` for whole window events,
//! `platform_window` for window creation and window state, `misc` for the
//! remaining small enums.
mod event;
mod keymap;
mod misc;
mod platform_window;
mod pointer;

use crate::core::{Size, input_method, keyboard, mouse, theme, window};

pub use event::{cursor_position, window_event};
pub use keymap::{key, key_code, native_key_code, physical_key};
pub use misc::{icon, ime_purpose, resize_direction, user_attention};
pub use platform_window::{
    fullscreen, mode, position, theme_mode, visible, window_attributes, window_level, window_theme,
};
pub use pointer::{modifiers, mouse_button, mouse_interaction};

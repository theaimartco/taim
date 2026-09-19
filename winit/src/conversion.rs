//! Convert [`winit`] types into [`taim_runtime`] types, and viceversa.
//!
//! [`winit`]: https://github.com/rust-windowing/winit
//! [`taim_runtime`]: https://github.com/theaimartco/taim/tree/master/runtime
mod event;

pub use event::{cursor_position, window_event};

use crate::core::Size;
use crate::core::input_method;
use crate::core::keyboard;
use crate::core::mouse;
use crate::core::theme;
use crate::core::window;

#[path = "conversion/keyboard.rs"]
mod keymap;
mod misc;
mod platform_window;
mod pointer;

pub use keymap::{key, key_code, native_key_code, physical_key};
pub use misc::{icon, ime_purpose, resize_direction, user_attention};
pub use platform_window::{
    fullscreen, mode, position, theme_mode, visible, window_attributes, window_level, window_theme,
};
pub use pointer::{modifiers, mouse_button, mouse_interaction};

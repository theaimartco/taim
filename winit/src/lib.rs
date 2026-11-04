//! The TAIM desktop shell, on top of [`winit`].
//!
//! [`run`] opens a window, builds its views once, and from then on turns
//! platform events into view events and frame-clock deadlines into frames.
//! There is no application loop to write: views change their own state, and
//! state changes wake the shell.
//!
//! The [`conversion`] module translates `winit` input and window types into
//! TAIM's vocabulary for anyone writing their own event loop, and
//! [`frame_gate`] exposes the per-window frame-production policy.
//!
//! [`winit`]: https://github.com/rust-windowing/winit
#![doc(html_logo_url = "https://raw.githubusercontent.com/theaimartco/taim/main/docs/logo.svg")]
#![cfg_attr(docsrs, feature(doc_cfg))]
pub use taim_core as core;
pub use taim_debug as debug;
pub use taim_graphics as graphics;
pub use winit;

pub mod clipboard;
pub mod conversion;
pub mod frame_clock;
pub mod frame_gate;

#[cfg(any(target_os = "android", test))]
pub mod android;
mod error;
#[cfg(all(feature = "measurement", not(target_arch = "wasm32")))]
mod measurement;
mod shell;
mod surface;
#[cfg(all(feature = "linux-theme-detection", target_os = "linux"))]
mod system_theme;
mod wake;

pub use clipboard::Clipboard;
pub use error::Error;
pub use frame_gate::{DirtyFlags, FrameGate, PlatformEvent, TaimEvent, WindowStateTracker};
#[cfg(target_os = "android")]
pub use shell::run_android;
pub use shell::{App, run};

type Renderer = taim_renderer::Renderer;
type Compositor = taim_renderer::Compositor;

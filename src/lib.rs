//! # TAIM
//!
//! TAIM is a Rust GUI framework built around views that own their state.
//! There is no message type and no update function: a view reads state
//! handles, and changing a handle repaints exactly the views that read it.
//!
//! ```no_run
//! use taim::prelude::*;
//!
//! fn main() -> taim::Result {
//!     taim::launch("Counter", |cx| {
//!         let count = cx.signal(0);
//!
//!         column(views![
//!             text(move || format!("Count: {}", count.get())).size(24),
//!             row(views![
//!                 button("-1").on_press(move || count.update(|n| *n -= 1)),
//!                 button("+1").on_press(move || count.update(|n| *n += 1)),
//!             ])
//!             .gap(8.0),
//!         ])
//!         .gap(12.0)
//!         .padding(24.0)
//!     })
//! }
//! ```
//!
//! The [`view`] module holds the view API and every built-in view; the
//! [`prelude`] imports what an application needs. The runtime owns frame
//! scheduling, accessibility and renderer recovery.
//!
//! TAIM is under active development. Consult `docs/status.md` before
//! depending on a platform or performance property.
#![doc(html_logo_url = "https://raw.githubusercontent.com/theaimartco/taim/main/docs/logo.svg")]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(all(
    target_family = "unix",
    not(target_os = "macos"),
    not(feature = "wayland"),
    not(feature = "x11"),
))]
compile_error!("No Unix display backend is enabled. Enable x11 or wayland.");

use taim_core as core;

mod error;
mod launch;

pub mod touch;
pub mod window;

pub use error::Error;
pub use launch::{App, app, launch, prelude};

/// The TAIM view API: views, state handles and their contracts.
pub use taim_view as view;

pub use crate::core::{
    Background, Border, Color, Degrees, Font, Gradient, Padding, Pixels, Point, Radians, Rectangle,
    Settings, Shadow, Size, Theme, Transformation, Vector, alignment, border, color, font,
    gradient, keyboard, mouse, padding, semantics, theme, time,
};
pub use taim_signal::{Memo, Signal};

/// The renderer views draw with, chosen when TAIM is compiled.
pub type Renderer = taim_renderer::Renderer;

/// Renderer backend selection and preferences.
pub mod backend {
    pub use taim_core::backend::*;
}

pub use backend::{Backend, PowerPreference};

/// Fine-grained reactive state primitives.
pub mod signal {
    pub use taim_signal::*;
}

/// The VYOM backend for TAIM applications.
#[cfg(feature = "vyom")]
pub mod platform_vyom {
    pub use taim_platform_vyom::*;
}

#[cfg(feature = "highlighter")]
pub use taim_highlighter as highlighter;

#[cfg(feature = "wgpu-bare")]
pub use taim_renderer::wgpu::wgpu;

/// What running an application returns.
pub type Result = std::result::Result<(), Error>;

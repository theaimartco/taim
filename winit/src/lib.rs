//! A windowing shell for Taim, on top of [`winit`].
//!
//! ![The native path of the Taim ecosystem](https://github.com/theaimartco/taim/blob/main/docs/graphs/native.svg?raw=true)
//!
//! `taim_winit` offers some convenient abstractions on top of [`taim_runtime`]
//! to quickstart development when using [`winit`].
//!
//! Applications are driven through [`run_app`] (native TAIM [`App`]s) or
//! [`run`] (the compatibility [`Program`] path, adapted onto the same engine).
//!
//! Additionally, a [`conversion`] module is available for users that decide to
//! implement a custom event loop.
//!
//! [`taim_runtime`]: https://github.com/theaimartco/taim/tree/master/runtime
//! [`winit`]: https://github.com/rust-windowing/winit
//! [`conversion`]: crate::conversion
//! [`App`]: program::App
//! [`Program`]: program::Program
#![doc(html_logo_url = "https://raw.githubusercontent.com/theaimartco/taim/main/docs/logo.svg")]
#![cfg_attr(docsrs, feature(doc_cfg))]
pub use program::core;
pub use program::graphics;
pub use runtime::futures;
pub use taim_debug as debug;
pub use taim_program as program;
pub use taim_runtime as runtime;
pub use winit;

pub mod clipboard;
pub mod conversion;
pub mod frame_gate;

mod error;
#[cfg(all(feature = "measurement", not(target_arch = "wasm32")))]
mod measurement;
mod proxy;
mod window;

pub use clipboard::Clipboard;
pub use error::Error;
pub use frame_gate::{DirtyFlags, FrameGate, PlatformEvent, TaimEvent, WindowStateTracker};
pub use proxy::Proxy;

mod shell;

pub use shell::{build_user_interfaces, run, run_app, user_force_quit};

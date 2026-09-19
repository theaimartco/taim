//! A windowing shell for Taim, on top of [`winit`].
//!
//! ![The native path of the Taim ecosystem](https://github.com/theaimartco/taim/blob/main/docs/graphs/native.svg?raw=true)
//!
//! `taim_winit` offers some convenient abstractions on top of [`taim_runtime`]
//! to quickstart development when using [`winit`].
//!
//! It exposes a renderer-agnostic [`Program`] trait that can be implemented
//! and then run with a simple call. The use of this trait is optional.
//!
//! Additionally, a [`conversion`] module is available for users that decide to
//! implement a custom event loop.
//!
//! [`taim_runtime`]: https://github.com/theaimartco/taim/tree/master/runtime
//! [`winit`]: https://github.com/rust-windowing/winit
//! [`conversion`]: crate::conversion
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

use crate::core::Renderer;
use crate::core::backend;
use crate::core::mouse;
use crate::core::renderer;
use crate::core::theme;
use crate::core::time::Instant;
use crate::core::widget::operation;
use crate::core::{Point, Size};
use crate::futures::futures::channel::mpsc;
use crate::futures::futures::channel::oneshot;
use crate::futures::futures::task;
use crate::futures::futures::{Future, StreamExt};
use crate::futures::subscription;
use crate::futures::{Executor, Runtime};
use crate::graphics::{Compositor, Shell, compositor};
use crate::runtime::font;
use crate::runtime::image;
use crate::runtime::system;
use crate::runtime::user_interface::{self, UserInterface};
use crate::runtime::{Action, Task};

use program::Program;

use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::mem::ManuallyDrop;
use std::slice;
use std::sync::Arc;

#[path = "shell/actions.rs"]
mod actions;
#[path = "shell/instance.rs"]
mod instance;
#[path = "shell/runner.rs"]
mod runner;

pub use actions::{build_user_interfaces, user_force_quit};
pub use runner::{run, run_app};

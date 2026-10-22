//! # TAIM
//!
//! TAIM is an experimental Rust GUI framework focused on predictable frame
//! scheduling, retained rendering work, bounded caches, and composable state.
//! The runtime owns invalidation, window lifecycle, accessibility, and renderer
//! recovery while applications remain ordinary state, update, and view code.
//!
//! Native applications have a dedicated intent-first API. Updates explicitly
//! declare whether to schedule a frame, avoiding a redraw on idle actions.
//!
//! ```no_run
//! use taim::{Element, Task};
//! use taim::native::{app, Dispatch};
//! use taim::AppContext;
//! use taim::WindowContext;
//!
//! fn scene<'a>(value: &'a u64, _: &WindowContext) -> Element<'a, u64> {
//!     taim::widget::text(format!("Count: {value}")).into()
//! }
//!
//! fn main() -> taim::Result {
//!     let native = app(
//!         "TAIM Native Example",
//!         |_context: &mut AppContext| (0_u64, Task::none()),
//!         |state: &mut u64, increment: u64, _context: &mut AppContext| {
//!             if increment == 0 {
//!                 Dispatch::idle()
//!             } else {
//!                 *state += increment;
//!                 Dispatch::paint()
//!             }
//!         },
//!         scene,
//!     );
//!     taim::run_app(native)
//! }
//! ```
//!
//! Use [`application`] for initialization, subscriptions, theming, and window
//! configuration. Use [`reactive_app`] with [`Signal`] for fine-grained state,
//! or implement [`App`] when native runner lifecycle control is required.
//!
//! Widgets compose through containers such as [`widget::Row`],
//! [`widget::Column`], [`widget::Grid`], and [`widget::Stack`]. Asynchronous
//! finite work uses [`Task`]; passive sources use [`Subscription`].
//!
//! TAIM is under active development. Consult `docs/status.md` and current
//! benchmark reports before depending on a platform or performance property.
#![doc(html_logo_url = "https://raw.githubusercontent.com/theaimartco/taim/main/docs/logo.svg")]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(not(any(
    target_arch = "wasm32",
    feature = "thread-pool",
    feature = "tokio",
    feature = "smol"
)))]
compile_error!("No futures executor is enabled. Enable one of: thread-pool, tokio, smol.");

#[cfg(all(
    target_family = "unix",
    not(target_os = "macos"),
    not(feature = "wayland"),
    not(feature = "x11"),
))]
compile_error!("No Unix display backend is enabled. Enable x11 or wayland.");

#[cfg(feature = "advanced")]
use taim_widget::renderer;
use taim_winit as shell;
use taim_winit::{core, program, runtime};

mod entry;
mod error;
mod facade;
mod launch;

pub mod application;

/// Native intent-based TAIM application API (no legacy ProgramAdapter).
pub mod native {
    pub use crate::program::native::{Dispatch, NativeApp, app};
}
pub mod daemon;
pub mod time;
pub mod window;

#[cfg(feature = "advanced")]
pub mod advanced;

pub use entry::{run, run_app};
pub use launch::{launch, prelude};

pub use facade::*;
/// The TAIM view API: views, state handles and their contracts.
pub use taim_view as view;

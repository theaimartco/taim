//! # TAIM
//!
//! TAIM is an experimental Rust GUI framework focused on predictable frame
//! scheduling, retained rendering work, bounded caches, and composable state.
//! The runtime owns invalidation, window lifecycle, accessibility, and renderer
//! recovery while applications remain ordinary state, update, and view code.
//!
//! ```no_run,standalone_crate
//! use taim::widget::{button, column, text};
//! use taim::Element;
//!
//! #[derive(Debug, Clone)]
//! enum Message { Increment }
//!
//! fn update(value: &mut u64, message: Message) {
//!     match message {
//!         Message::Increment => *value += 1,
//!     }
//! }
//!
//! fn view(value: &u64) -> Element<'_, Message> {
//!     column![
//!         text(value),
//!         button("Increment").on_press(Message::Increment),
//!     ]
//!     .spacing(8)
//!     .into()
//! }
//!
//! fn main() -> taim::Result { taim::run(update, view) }
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

pub mod application;
pub mod daemon;
pub mod time;
pub mod window;

#[cfg(feature = "advanced")]
pub mod advanced;

pub use entry::{run, run_app};
pub use facade::*;

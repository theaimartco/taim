//! The TAIM desktop shell.
//!
//! The shell is split along one line: who owns the platform, and who owns the
//! application.
//!
//! - The [`host`] is the `winit` [`ApplicationHandler`]. It is the only code
//!   that touches the event loop: it creates native windows, sets the control
//!   flow and exits. It never sees application state.
//! - The [`engine`] owns the application, its windows, renderers and the
//!   async runtime. It runs as a single cooperative task that the host polls
//!   whenever the platform has something to say, so the per-window
//!   [`UserInterface`]s can borrow the application across event-loop turns
//!   without any shared mutability.
//!
//! The two halves talk only through the vocabulary in [`protocol`]: the host
//! sends [`protocol::Signal`]s, the engine answers with
//! [`protocol::Request`]s.
//!
//! [`ApplicationHandler`]: winit::application::ApplicationHandler
//! [`UserInterface`]: crate::runtime::user_interface::UserInterface
mod commands;
mod engine;
mod frame;
mod host;
mod inbox;
mod launch;
mod protocol;
mod views;

pub use host::user_force_quit;
pub use launch::{run, run_app};
pub use views::build_user_interfaces;

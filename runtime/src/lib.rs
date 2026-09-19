//! A renderer-agnostic native GUI runtime.
//!
//! `taim_runtime` turns an [`taim_core`] widget tree into a running
//! application frame: event dispatch, layout, semantic updates, paint
//! preparation, input-method coordination, and background tasks.
//!
//! [`taim_core`]: https://github.com/theaimartco/taim/tree/main/core

#![doc(html_logo_url = "https://raw.githubusercontent.com/theaimartco/taim/main/docs/logo.svg")]
#![cfg_attr(docsrs, feature(doc_cfg))]
pub mod backend;
pub mod clipboard;
pub mod diagnostics;
pub mod font;
pub mod frame_scheduler;
pub mod hit_test;
pub mod image;
pub mod invalidation;
pub mod keyboard;
pub mod memory_pressure;
pub mod pointer_router;
pub mod power_policy;
pub mod quality_controller;
pub mod reactive_ui;
pub mod reconcile;
pub mod recovery;
pub mod semantics;
pub mod system;
pub mod task;
pub mod user_interface;
pub mod widget;
pub mod window;

pub use taim_core as core;
pub use taim_futures as futures;

pub use reactive_ui::ReactiveUi;
pub use task::Task;
pub use user_interface::UserInterface;

mod action;

pub use action::Action;

/// Creates a [`Task`] that exits the taim runtime.
///
/// This will normally close any application windows and
/// terminate the runtime loop.
pub fn exit<T>() -> Task<T> {
    task::effect(Action::Exit)
}

//! Backend-neutral rendering entry point for TAIM applications.
#![cfg_attr(docsrs, feature(doc_cfg))]

pub use taim_graphics::{self as graphics, core};

#[cfg(feature = "geometry")]
pub use taim_graphics::geometry;

#[cfg(feature = "wgpu-bare")]
pub use taim_wgpu as wgpu;

pub mod route;
mod selection;

pub use route::{Route, RouteKind};
pub use selection::{Compositor, Renderer};

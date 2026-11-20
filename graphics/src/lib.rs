//! A bunch of backend-agnostic types that can be leveraged to build a renderer
//! for [`taim`].
//!
//! ![The native path of the Taim ecosystem](https://github.com/theaimartco/taim/blob/main/docs/graphs/native.svg?raw=true)
//!
//! [`taim`]: https://github.com/theaimartco/taim
#![doc(html_logo_url = "https://raw.githubusercontent.com/theaimartco/taim/main/docs/logo-256.png")]
#![cfg_attr(docsrs, feature(doc_cfg))]
mod antialiasing;
#[path = "error.rs"]
pub mod validation;
mod viewport;

pub mod cache;
pub mod color;
pub mod compositor;
pub mod damage;
pub mod display_list;
pub mod gradient;
pub mod image;
pub mod layer;
pub mod mesh;
pub mod quality;
pub mod retained_layer;
pub mod shell;
pub mod text;

#[cfg(feature = "geometry")]
pub mod geometry;

pub use antialiasing::Antialiasing;
pub use cache::Cache;
pub use compositor::Compositor;
pub use compositor::FrameContext;
pub use gradient::Gradient;
pub use image::Image;
pub use layer::Layer;
pub use mesh::Mesh;
pub use shell::Shell;
pub use text::Text;
pub use viewport::Viewport;

#[cfg(feature = "raster")]
pub use taim_raster as raster;

pub use taim_core as core;
pub use taim_futures as futures;

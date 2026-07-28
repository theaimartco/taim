//! A bunch of backend-agnostic types that can be leveraged to build a renderer
//! for [`aura`].
//!
//! ![The native path of the Aura ecosystem](https://github.com/aura-rs/aura/blob/0525d76ff94e828b7b21634fa94a747022001c83/docs/graphs/native.png?raw=true)
//!
//! [`aura`]: https://github.com/aura-rs/aura
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/aura-rs/aura/9ab6923e943f784985e9ef9ca28b10278297225d/docs/logo.svg"
)]
#![cfg_attr(docsrs, feature(doc_cfg))]
mod antialiasing;
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

pub use aura_core as core;
pub use aura_futures as futures;

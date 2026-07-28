//! Display rendering results on windows.
pub mod compositor;
pub mod frame_feedback;

pub use compositor::Compositor;
pub use wgpu::Surface;

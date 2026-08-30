//! The official renderer for taim.
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "wgpu-bare")]
pub use taim_wgpu as wgpu;

pub mod fallback;

pub use taim_graphics as graphics;
pub use taim_graphics::core;

#[cfg(feature = "geometry")]
pub use taim_graphics::geometry;

/// The default graphics renderer for [`taim`].
///
/// [`taim`]: https://github.com/theaimartco/taim
pub type Renderer = renderer::Renderer;

/// The default graphics compositor for [`taim`].
///
/// [`taim`]: https://github.com/theaimartco/taim
pub type Compositor = renderer::Compositor;

#[cfg(all(feature = "wgpu-bare", feature = "tiny-skia"))]
mod renderer {
    pub type Renderer = crate::fallback::Renderer<taim_wgpu::Renderer, taim_tiny_skia::Renderer>;

    pub type Compositor = crate::fallback::Compositor<
        taim_wgpu::window::Compositor,
        taim_tiny_skia::window::Compositor,
    >;
}

#[cfg(all(feature = "wgpu-bare", not(feature = "tiny-skia")))]
mod renderer {
    pub type Renderer = taim_wgpu::Renderer;
    pub type Compositor = taim_wgpu::window::Compositor;
}

#[cfg(all(not(feature = "wgpu-bare"), feature = "tiny-skia"))]
mod renderer {
    pub type Renderer = taim_tiny_skia::Renderer;
    pub type Compositor = taim_tiny_skia::window::Compositor;
}

#[cfg(not(any(feature = "wgpu-bare", feature = "tiny-skia")))]
mod renderer {
    #[cfg(not(debug_assertions))]
    compile_error!(
        "Cannot compile `taim_renderer` in release mode \
        without a renderer feature enabled. \
        Enable either the `wgpu` or `tiny-skia` feature, or both."
    );

    pub type Renderer = ();
    pub type Compositor = ();
}

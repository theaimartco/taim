//! The official renderer for aura.
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "wgpu-bare")]
pub use aura_wgpu as wgpu;

pub mod fallback;

pub use aura_graphics as graphics;
pub use aura_graphics::core;

#[cfg(feature = "geometry")]
pub use aura_graphics::geometry;

/// The default graphics renderer for [`aura`].
///
/// [`aura`]: https://github.com/aura-rs/aura
pub type Renderer = renderer::Renderer;

/// The default graphics compositor for [`aura`].
///
/// [`aura`]: https://github.com/aura-rs/aura
pub type Compositor = renderer::Compositor;

#[cfg(all(feature = "wgpu-bare", feature = "tiny-skia"))]
mod renderer {
    pub type Renderer = crate::fallback::Renderer<aura_wgpu::Renderer, aura_tiny_skia::Renderer>;

    pub type Compositor = crate::fallback::Compositor<
        aura_wgpu::window::Compositor,
        aura_tiny_skia::window::Compositor,
    >;
}

#[cfg(all(feature = "wgpu-bare", not(feature = "tiny-skia")))]
mod renderer {
    pub type Renderer = aura_wgpu::Renderer;
    pub type Compositor = aura_wgpu::window::Compositor;
}

#[cfg(all(not(feature = "wgpu-bare"), feature = "tiny-skia"))]
mod renderer {
    pub type Renderer = aura_tiny_skia::Renderer;
    pub type Compositor = aura_tiny_skia::window::Compositor;
}

#[cfg(not(any(feature = "wgpu-bare", feature = "tiny-skia")))]
mod renderer {
    #[cfg(not(debug_assertions))]
    compile_error!(
        "Cannot compile `aura_renderer` in release mode \
        without a renderer feature enabled. \
        Enable either the `wgpu` or `tiny-skia` feature, or both."
    );

    pub type Renderer = ();
    pub type Compositor = ();
}

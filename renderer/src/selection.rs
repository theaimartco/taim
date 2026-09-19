//! Compile-time backend selection.
//!
//! The public renderer types contain only the backends enabled by the build.
//! Single-backend builds therefore pay no recovery-route branch in their draw
//! path. Dual-backend builds retain the software recovery route without trait
//! objects or heap allocation.

#[cfg(all(feature = "wgpu-bare", feature = "tiny-skia"))]
/// Hardware-first renderer with a software recovery route.
pub type Renderer = crate::Route<taim_wgpu::Renderer, taim_tiny_skia::Renderer>;

#[cfg(all(feature = "wgpu-bare", feature = "tiny-skia"))]
/// Hardware-first compositor with a software recovery route.
pub type Compositor =
    crate::Route<taim_wgpu::window::Compositor, taim_tiny_skia::window::Compositor>;

#[cfg(all(feature = "wgpu-bare", not(feature = "tiny-skia")))]
/// Direct hardware renderer used by GPU-only builds.
pub type Renderer = taim_wgpu::Renderer;

#[cfg(all(feature = "wgpu-bare", not(feature = "tiny-skia")))]
/// Direct hardware compositor used by GPU-only builds.
pub type Compositor = taim_wgpu::window::Compositor;

#[cfg(all(not(feature = "wgpu-bare"), feature = "tiny-skia"))]
/// Direct software renderer used by software-only builds.
pub type Renderer = taim_tiny_skia::Renderer;

#[cfg(all(not(feature = "wgpu-bare"), feature = "tiny-skia"))]
/// Direct software compositor used by software-only builds.
pub type Compositor = taim_tiny_skia::window::Compositor;

#[cfg(not(any(feature = "wgpu-bare", feature = "tiny-skia")))]
/// Debug-only placeholder when a build intentionally enables no backend.
pub type Renderer = ();

#[cfg(not(any(feature = "wgpu-bare", feature = "tiny-skia")))]
/// Debug-only placeholder when a build intentionally enables no backend.
pub type Compositor = ();

#[cfg(all(
    not(debug_assertions),
    not(any(feature = "wgpu-bare", feature = "tiny-skia"))
))]
compile_error!("a release build of `taim_renderer` needs `wgpu` or `tiny-skia`");

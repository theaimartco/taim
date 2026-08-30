//! Baseline headless `taim_wgpu` renderer smoke test.
//!
//! `Compositor::is_device_lost` (the device-loss detection this file
//! was originally meant to test) lives on `window::Compositor`, which
//! requires a live window/surface to construct -- not reachable from a
//! headless integration test. `wgpu/src/window/compositor.rs`'s own
//! `device_lost_tests` module covers the actual flag/callback mechanism
//! directly instead.
//!
//! What *is* reachable headlessly is `taim_wgpu::Renderer` via
//! `renderer::Headless`, which does construct a real `wgpu::Device`
//! (skipping only the windowed `Compositor`/surface layer). This test
//! is deliberately scoped as partial coverage: a baseline "the real
//! `wgpu` backend still initializes and can draw a frame without
//! panicking" smoke test, not a device-loss test per se -- there is no
//! way to force a real device loss from safe, portable Rust without
//! hardware/driver cooperation this environment cannot provide.
use taim_core::Size;
use taim_wgpu::Renderer;

fn headless_wgpu_renderer() -> Option<Renderer> {
    taim_futures::futures::executor::block_on(<Renderer as taim_core::renderer::Headless>::new(
        taim_core::renderer::Settings::default(),
        Some("wgpu"),
    ))
}

#[test]
fn a_headless_wgpu_renderer_can_be_constructed_and_screenshotted() {
    // If no real `wgpu`-compatible adapter exists in this environment
    // (common on a headless CI/dev machine with no GPU driver), this is
    // reported as skipped rather than failed: the absence of hardware
    // is not a defect in this crate's device-loss handling.
    let Some(mut renderer) = headless_wgpu_renderer() else {
        eprintln!(
            "skipping: no wgpu-compatible adapter available in this environment \
             (this is expected on a headless machine with no GPU driver)"
        );
        return;
    };

    let viewport = taim_core::Rectangle::new(taim_core::Point::ORIGIN, Size::new(64.0, 64.0));
    let viewport = taim_wgpu::graphics::Viewport::with_physical_size(
        Size::new(viewport.width as u32, viewport.height as u32),
        1.0,
    );

    let screenshot = renderer.screenshot(&viewport, taim_core::Color::BLACK);

    assert_eq!(
        screenshot.len(),
        64 * 64 * 4,
        "a 64x64 RGBA screenshot must be exactly width * height * 4 bytes"
    );
}

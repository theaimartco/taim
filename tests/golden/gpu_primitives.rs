//! Exact/tolerance pixel tests for `taim_wgpu`'s real GPU primitives,
//! rendered through the actual `wgpu` backend (not a synthetic oracle).
//!
//! Every test here skips (rather than fails) if no `wgpu`-compatible
//! adapter is available, matching `tests/gpu/device_loss.rs`'s
//! precedent: the absence of GPU hardware in a given environment is not
//! a defect in this crate to report as a test failure.
//!
//! A tolerance, not exact equality, is used throughout: gamma-correct
//! color blending, MSAA resolution, and driver-specific rounding all
//! introduce small, expected per-channel differences from a naively
//! computed expected color -- this is the same reasoning
//! `docs/performance/methodology.md` already documents for timing
//! measurements, applied to pixel values instead.

use taim_core::{Color, Point, Rectangle, Size};
use taim_wgpu::Renderer;
use taim_wgpu::core::renderer::Quad;
use taim_wgpu::graphics::Viewport;

/// The maximum allowed per-channel difference (out of 255) between an
/// expected and actual pixel value.
const TOLERANCE: i16 = 12;

fn headless_wgpu_renderer() -> Option<Renderer> {
    taim_futures::futures::executor::block_on(<Renderer as taim_core::renderer::Headless>::new(
        taim_core::renderer::Settings::default(),
        Some("wgpu"),
    ))
}

fn render_solid_quad(size: u32, scale_factor: f32, color: Color) -> Vec<u8> {
    let mut renderer =
        headless_wgpu_renderer().expect("a wgpu-compatible adapter must be available");

    let logical_size = Size::new(size as f32 / scale_factor, size as f32 / scale_factor);

    <Renderer as taim_core::Renderer>::fill_quad(
        &mut renderer,
        Quad {
            bounds: Rectangle::new(Point::ORIGIN, logical_size),
            ..Quad::default()
        },
        color,
    );

    let viewport = Viewport::with_physical_size(Size::new(size, size), scale_factor);

    renderer.screenshot(&viewport, Color::BLACK)
}

fn pixel_at(rgba: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
    let offset = ((y * size + x) * 4) as usize;
    [
        rgba[offset],
        rgba[offset + 1],
        rgba[offset + 2],
        rgba[offset + 3],
    ]
}

fn assert_pixel_close(actual: [u8; 4], expected: [u8; 4], context: &str) {
    for channel in 0..4 {
        let diff = i16::from(actual[channel]) - i16::from(expected[channel]);

        assert!(
            diff.abs() <= TOLERANCE,
            "{context}: channel {channel} differs by {diff} (actual {actual:?}, expected {expected:?}, tolerance {TOLERANCE})"
        );
    }
}

#[test]
fn a_solid_red_quad_fills_the_viewport_with_red_at_scale_1() {
    let Some(_) = headless_wgpu_renderer() else {
        eprintln!("skipping: no wgpu-compatible adapter available in this environment");
        return;
    };

    let size = 32;
    let rgba = render_solid_quad(size, 1.0, Color::from_rgb8(220, 40, 40));

    assert_pixel_close(
        pixel_at(&rgba, size, size / 2, size / 2),
        [220, 40, 40, 255],
        "center pixel at scale 1.0",
    );
}

#[test]
fn a_solid_quad_fills_the_viewport_consistently_across_scale_factors() {
    let Some(_) = headless_wgpu_renderer() else {
        eprintln!("skipping: no wgpu-compatible adapter available in this environment");
        return;
    };

    let color = Color::from_rgb8(30, 140, 220);

    let at_scale_1 = render_solid_quad(32, 1.0, color);
    let at_scale_2 = render_solid_quad(64, 2.0, color);

    let center_1 = pixel_at(&at_scale_1, 32, 16, 16);
    let center_2 = pixel_at(&at_scale_2, 64, 32, 32);

    assert_pixel_close(
        center_2,
        center_1,
        "the same logical color at scale 1.0 and scale 2.0 must render the same pixel value",
    );
}

#[test]
fn a_fully_transparent_background_leaves_the_clear_color_visible() {
    let Some(mut renderer) = headless_wgpu_renderer() else {
        eprintln!("skipping: no wgpu-compatible adapter available in this environment");
        return;
    };

    let size = 32;
    let viewport = Viewport::with_physical_size(Size::new(size, size), 1.0);

    // No `fill_quad` call at all: the whole surface should show the
    // clear color passed to `screenshot`, unaffected by any leftover
    // state from a previous test (a fresh `Renderer` starts empty).
    let clear_color = Color::from_rgb8(10, 200, 10);
    let rgba = renderer.screenshot(&viewport, clear_color);

    assert_pixel_close(
        pixel_at(&rgba, size, size / 2, size / 2),
        [10, 200, 10, 255],
        "an empty frame must show exactly the requested clear color",
    );
}

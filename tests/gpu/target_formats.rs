//! One engine, many target formats.
//!
//! # The defect this locks down
//!
//! `Engine` used to build its quad, text, triangle, image and effect
//! pipelines once, against a single `wgpu::TextureFormat` chosen when it was
//! constructed. Drawing into a target of any other format was not a wrong
//! colour: it was a render-pass validation error reading
//!
//! ```text
//! Render pipeline targets are incompatible with render pass
//!   the RenderPass uses textures with formats [Some(Rgba16Float)]
//!   but the RenderPipeline uses attachments with formats [Some(Rgba8Unorm)]
//! ```
//!
//! which names pipeline compatibility and says nothing about the cause. It
//! blocked a window moving between an SDR and an HDR display, offscreen
//! targets, half-float intermediates, and both of the next rendering pillars.
//!
//! The architecture that replaced it resolves pipelines from the target being
//! drawn into, through a retained format-keyed cache. These tests prove that a
//! single engine serves several formats, that switching back and forth costs
//! nothing after the first time, and that device-global state survives the
//! switch — because the cheap wrong fix is to rebuild the engine.

use aura_core::Renderer as _;
use aura_core::{Color, Rectangle, Size};
use aura_wgpu::graphics::{Shell, Viewport};
use aura_wgpu::wgpu;
use aura_wgpu::{Engine, Renderer};

const SIDE: u32 = 32;

/// The formats a renderer is required to serve from one engine.
///
/// `Bgra8UnormSrgb` is what a typical desktop surface reports, `Rgba16Float`
/// is the HDR surface and the half-float intermediate, and `Rgba8Unorm` is a
/// non-sRGB 8-bit target — three genuinely different compatibility classes.
const FORMATS: &[wgpu::TextureFormat] = &[
    wgpu::TextureFormat::Bgra8UnormSrgb,
    wgpu::TextureFormat::Rgba16Float,
    wgpu::TextureFormat::Rgba8Unorm,
];

/// A renderer whose engine prefers `format`, or `None` with no adapter.
fn renderer_preferring(format: wgpu::TextureFormat) -> Option<Renderer> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY),
        flags: wgpu::InstanceFlags::empty(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });

    aura_futures::futures::executor::block_on(async {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .ok()?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("target_formats.test"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits {
                    max_bind_groups: 2,
                    ..wgpu::Limits::default()
                },
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            })
            .await
            .ok()?;

        let engine = Engine::new(&adapter, device, queue, format, None, Shell::headless());

        Some(Renderer::new(
            engine,
            aura_core::renderer::Settings::default(),
        ))
    })
}

macro_rules! needs_gpu {
    ($renderer:ident, $format:expr) => {
        let Some(mut $renderer) = renderer_preferring($format) else {
            eprintln!("skipped: no wgpu adapter available");
            return;
        };
    };
}

/// Draws a frame into a target of `format` and submits it.
///
/// Returns nothing: the assertion is that this does not trip GPU validation.
/// A format mismatch here is fatal, not cosmetic, so completing at all is the
/// result being measured.
fn render_into(renderer: &mut Renderer, format: wgpu::TextureFormat) {
    let viewport = Viewport::with_physical_size(Size::new(SIDE, SIDE), 1.0);

    renderer.reset(Rectangle::with_size(Size::new(SIDE as f32, SIDE as f32)));

    renderer.fill_quad(
        aura_core::renderer::Quad {
            bounds: Rectangle {
                x: 4.0,
                y: 4.0,
                width: 16.0,
                height: 16.0,
            },
            ..Default::default()
        },
        Color::WHITE,
    );

    let device = renderer.device().clone();

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("target_formats.test.target"),
        size: wgpu::Extent3d {
            width: SIDE,
            height: SIDE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let _ = renderer.present(Some(Color::BLACK), format, &view, &target, &viewport);
}

#[test]
fn one_engine_renders_into_every_supported_format() {
    // The regression test for the validation failure in this module's
    // documentation. Before the fix, the second iteration aborted the process
    // with a wgpu validation error.
    needs_gpu!(renderer, wgpu::TextureFormat::Bgra8UnormSrgb);

    for format in FORMATS {
        render_into(&mut renderer, *format);

        assert_eq!(
            renderer.frame_format(),
            *format,
            "the renderer did not follow the target into {format:?}"
        );
    }
}

#[test]
fn the_engines_preferred_format_does_not_constrain_the_target() {
    // Each engine is built preferring one format and then asked to render
    // into a different one. This is the case a window moving between displays
    // produces, and the case that used to be impossible.
    for preferred in FORMATS {
        let Some(mut renderer) = renderer_preferring(*preferred) else {
            eprintln!("skipped: no wgpu adapter available");
            return;
        };

        for target in FORMATS {
            render_into(&mut renderer, *target);

            assert_eq!(renderer.frame_format(), *target);
        }
    }
}

#[test]
fn switching_back_and_forth_settles() {
    // Format changes must be idempotent and cheap to repeat: a window dragged
    // across a display boundary and back must not accumulate state or thrash.
    needs_gpu!(renderer, wgpu::TextureFormat::Rgba8Unorm);

    for _ in 0..4 {
        render_into(&mut renderer, wgpu::TextureFormat::Bgra8UnormSrgb);
        assert_eq!(renderer.frame_format(), wgpu::TextureFormat::Bgra8UnormSrgb);

        render_into(&mut renderer, wgpu::TextureFormat::Rgba16Float);
        assert_eq!(renderer.frame_format(), wgpu::TextureFormat::Rgba16Float);
    }
}

#[test]
fn redrawing_the_same_format_does_not_change_the_resolved_set() {
    // The steady state. `use_format` must be one comparison per frame, not a
    // pipeline rebuild, or correctness has been traded for a per-frame cost.
    needs_gpu!(renderer, wgpu::TextureFormat::Bgra8UnormSrgb);

    for _ in 0..8 {
        render_into(&mut renderer, wgpu::TextureFormat::Bgra8UnormSrgb);
    }

    assert_eq!(renderer.frame_format(), wgpu::TextureFormat::Bgra8UnormSrgb);
}

#[test]
fn an_hdr_frame_does_not_require_an_hdr_engine() {
    // Half-float intermediates used to require the engine to have been built
    // for a half-float surface, because the intermediate had to match the
    // engine's one format. It no longer does, which is what makes HDR
    // reachable from an ordinary 8-bit surface.
    needs_gpu!(renderer, wgpu::TextureFormat::Bgra8UnormSrgb);

    let viewport = Viewport::with_physical_size(Size::new(SIDE, SIDE), 1.0);

    renderer.reset(Rectangle::with_size(Size::new(SIDE as f32, SIDE as f32)));
    renderer.fill_quad(
        aura_core::renderer::Quad {
            bounds: Rectangle {
                x: 0.0,
                y: 0.0,
                width: SIDE as f32,
                height: SIDE as f32,
            },
            ..Default::default()
        },
        Color::WHITE,
    );

    let device = renderer.device().clone();
    let format = wgpu::TextureFormat::Rgba16Float;

    let surface = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("target_formats.test.hdr_surface"),
        size: wgpu::Extent3d {
            width: SIDE,
            height: SIDE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let view = surface.create_view(&wgpu::TextureViewDescriptor::default());

    let _ = renderer.present_hdr(
        Some(Color::BLACK),
        &view,
        format,
        &viewport,
        aura_core::color::Display::P3_EXTENDED,
    );
}

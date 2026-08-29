//! CPU/GPU conformance for the display transform.
//!
//! `core/src/color/display.rs` and `wgpu/src/shader/display.wgsl` implement
//! the same transform twice: once for screenshots, goldens, and any caller
//! reasoning about color in Rust, and once for the pixels a person actually
//! looks at. Two implementations of the same arithmetic drift, and a drift
//! here is invisible -- the frame still renders, the goldens still pass
//! (they go through `tiny-skia`), and the only symptom is that the panel is
//! wrong in a way nobody can measure without a camera.
//!
//! These tests run a sweep of scene-referred values through the shader on a
//! real device and require the answers to match the Rust implementation.
//!
//! Skipped rather than failed when no adapter exists: a machine with no GPU
//! cannot answer the question, and a suite that goes red on such a machine
//! teaches its readers to ignore it.

use aura_core::color::display::{Display, DisplayTransform, GamutMap, ToneMap};
use aura_core::color::{Color, Radiance};
use aura_wgpu::display::Pipeline;
use aura_wgpu::wgpu;

/// Full precision in and out: the point is to compare arithmetic, not to
/// measure how much a half-float target rounds it.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;

/// A device and queue, or `None` when no adapter exists.
fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
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

        adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("display_transform.test"),
                ..Default::default()
            })
            .await
            .ok()
    })
}

macro_rules! needs_gpu {
    ($device:ident, $queue:ident) => {
        let Some(($device, $queue)) = device() else {
            eprintln!("skipped: no wgpu adapter available");
            return;
        };
    };
}

/// Runs `inputs` through the shader, one texel each, and reads the result.
fn on_gpu(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    transform: &DisplayTransform,
    inputs: &[Radiance],
) -> Vec<[f32; 4]> {
    let width = inputs.len() as u32;

    let descriptor = |usage| wgpu::TextureDescriptor {
        label: Some("display_transform.test.texture"),
        size: wgpu::Extent3d {
            width,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage,
        view_formats: &[],
    };

    let source = device.create_texture(&descriptor(
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    ));
    let target = device.create_texture(&descriptor(
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    ));

    // `bytemuck` is not a dependency here, and a hand-rolled conversion is
    // clearer than adding one for sixteen bytes a texel.
    let texels: Vec<u8> = inputs
        .iter()
        .flat_map(|radiance| [radiance.red, radiance.green, radiance.blue, radiance.alpha])
        .flat_map(f32::to_ne_bytes)
        .collect();

    queue.write_texture(
        source.as_image_copy(),
        &texels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 16),
            rows_per_image: Some(1),
        },
        source.size(),
    );

    // 256-byte row alignment is a copy requirement, not a texture one.
    let bytes_per_row = (width * 16).div_ceil(256) * 256;

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("display_transform.test.readback"),
        size: u64::from(bytes_per_row),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let pipeline = Pipeline::new(device, FORMAT);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("display_transform.test.encoder"),
    });

    pipeline.render(
        device,
        queue,
        &mut encoder,
        &source.create_view(&wgpu::TextureViewDescriptor::default()),
        &target.create_view(&wgpu::TextureViewDescriptor::default()),
        transform,
    );

    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(1),
            },
        },
        target.size(),
    );

    let index = queue.submit([encoder.finish()]);

    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |result| {
        result.expect("the readback buffer must map");
    });

    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: Some(index),
        timeout: None,
    });

    let data = slice.get_mapped_range();

    let floats: Vec<f32> = data
        .chunks_exact(4)
        .map(|bytes| f32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        .collect();

    let out = (0..inputs.len())
        .map(|index| {
            [
                floats[index * 4],
                floats[index * 4 + 1],
                floats[index * 4 + 2],
                floats[index * 4 + 3],
            ]
        })
        .collect();

    drop(data);
    readback.unmap();

    out
}

/// The sweep both implementations must agree on.
fn sweep() -> Vec<Radiance> {
    let mut values = vec![
        Radiance::BLACK,
        Radiance::from(Color::WHITE),
        Radiance::from(Color::from_rgb8(0x00, 0xA8, 0xFF)),
        Radiance::from(Color::from_rgb8(0xFF, 0x33, 0x66)),
        Radiance::from(Color::from_rgba8(0x40, 0x80, 0xC0, 0.5)),
        // Saturated primaries, which are where gamut mapping does its work.
        Radiance::new(1.0, 0.0, 0.0, 1.0),
        Radiance::new(0.0, 1.0, 0.0, 1.0),
        Radiance::new(0.0, 0.0, 1.0, 1.0),
    ];

    // A luminance ramp straddling the knee, where tone mapping does its work.
    for nits in [1.0, 50.0, 203.0, 400.0, 1_000.0, 2_000.0, 4_000.0] {
        values.push(Radiance::from(Color::WHITE).at_nits(nits));
        values.push(Radiance::from(Color::from_rgb8(0xFF, 0x80, 0x20)).at_nits(nits));
    }

    values
}

/// Asserts the shader and the Rust implementation agree across the sweep.
#[track_caller]
fn assert_agrees(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    transform: &DisplayTransform,
    label: &str,
) {
    let inputs = sweep();
    let gpu = on_gpu(device, queue, transform, &inputs);

    for (input, actual) in inputs.iter().zip(gpu) {
        let expected = transform.apply(*input);

        for channel in 0..4 {
            let difference = (actual[channel] - expected[channel]).abs();
            let tolerance = expected[channel].abs().max(1.0) * 2e-3;

            assert!(
                difference <= tolerance,
                "{label}: channel {channel} of {input:?}\n  \
                 gpu  = {:?}\n  cpu  = {:?}",
                actual,
                expected
            );
        }
    }
}

#[test]
fn the_shader_agrees_with_the_reference_on_an_sdr_panel() {
    needs_gpu!(device, queue);

    assert_agrees(
        &device,
        &queue,
        &DisplayTransform::new(Display::SDR).with_source_peak_nits(4_000.0),
        "sdr",
    );
}

#[test]
fn the_shader_agrees_with_the_reference_on_an_hdr_panel() {
    needs_gpu!(device, queue);

    assert_agrees(
        &device,
        &queue,
        &DisplayTransform::new(Display::HDR10_1000).with_source_peak_nits(4_000.0),
        "hdr10",
    );
}

#[test]
fn the_shader_agrees_with_the_reference_on_an_extended_surface() {
    needs_gpu!(device, queue);

    assert_agrees(
        &device,
        &queue,
        &DisplayTransform::new(Display::P3_EXTENDED).with_source_peak_nits(4_000.0),
        "p3-extended",
    );
}

#[test]
fn the_shader_agrees_with_the_reference_for_every_operator() {
    needs_gpu!(device, queue);

    for tone_map in [ToneMap::Bt2390, ToneMap::Clip] {
        for gamut_map in [GamutMap::Desaturate, GamutMap::Clip] {
            assert_agrees(
                &device,
                &queue,
                &DisplayTransform::new(Display::HDR10_1000)
                    .with_source_peak_nits(4_000.0)
                    .with_tone_map(tone_map)
                    .with_gamut_map(gamut_map),
                &format!("{tone_map:?}/{gamut_map:?}"),
            );
        }
    }
}

#[test]
fn a_highlight_reaches_an_hdr_panel_brighter_than_white() {
    // The gate, measured on a real device rather than in arithmetic: the
    // shader must put a 1,000-nit highlight above diffuse white in the signal
    // it hands the panel.
    needs_gpu!(device, queue);

    let transform = DisplayTransform::new(Display::HDR10_1000).with_source_peak_nits(1_000.0);

    let inputs = vec![
        Radiance::from(Color::WHITE),
        Radiance::from(Color::WHITE).at_nits(1_000.0),
    ];

    let output = on_gpu(&device, &queue, &transform, &inputs);

    let white = Display::HDR10_1000.decode_nits(output[0][0]);
    let highlight = Display::HDR10_1000.decode_nits(output[1][0]);

    assert!(
        (white - 203.0).abs() < 5.0,
        "diffuse white reached the panel at {white} nits"
    );
    assert!(
        highlight > 900.0,
        "the highlight reached the panel at only {highlight} nits"
    );
}

mod present_path {
    use super::*;

    use aura_core::Renderer as _;
    use aura_core::{Rectangle, Size};
    use aura_wgpu::graphics::{Shell, Viewport};
    use aura_wgpu::{Engine, Renderer};

    const WIDTH: u32 = 64;
    const HEIGHT: u32 = 64;

    /// A renderer whose pipelines are built for `format`.
    ///
    /// The headless constructor pins itself to `Rgba8UnormSrgb`, and the HDR
    /// path needs a float frame format, so this builds the `Engine` directly.
    pub fn renderer_for(format: wgpu::TextureFormat) -> Option<Renderer> {
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
                    label: Some("display_transform.present.test"),
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

    /// The same scene every time: white, and a mid-grey patch, which is where
    /// a double-encode shows up.
    fn scene(renderer: &mut Renderer) {
        renderer.reset(Rectangle::with_size(Size::new(WIDTH as f32, HEIGHT as f32)));

        for (offset, color) in [(8.0, Color::WHITE), (32.0, Color::from_rgb(0.5, 0.5, 0.5))] {
            renderer.fill_quad(
                aura_core::renderer::Quad {
                    bounds: Rectangle {
                        x: offset,
                        y: offset,
                        width: 24.0,
                        height: 24.0,
                    },
                    ..Default::default()
                },
                color,
            );
        }
    }

    /// Presents the frame already recorded on `renderer` and reads the
    /// surface back as raw bytes. The caller owns the scene.
    pub fn present_and_read(
        renderer: &mut Renderer,
        format: wgpu::TextureFormat,
        bytes_per_texel: u32,
        display: Display,
        hdr: bool,
    ) -> Vec<u8> {
        let viewport = Viewport::with_physical_size(Size::new(WIDTH, HEIGHT), 1.0);

        let device = renderer.device().clone();
        let queue = renderer.queue().clone();

        let surface = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("display_transform.present.surface"),
            size: wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
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

        if hdr {
            let _ = renderer.present_hdr(Some(Color::BLACK), &view, format, &viewport, display);
        } else {
            let _ = renderer.present(Some(Color::BLACK), format, &view, &surface, &viewport);
        }

        let bytes_per_row = (WIDTH * bytes_per_texel).div_ceil(256) * 256;

        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("display_transform.present.readback"),
            size: u64::from(bytes_per_row * HEIGHT),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("display_transform.present.readback_encoder"),
        });

        encoder.copy_texture_to_buffer(
            surface.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(HEIGHT),
                },
            },
            surface.size(),
        );

        let index = queue.submit([encoder.finish()]);

        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            result.expect("the readback buffer must map");
        });

        let _ = device.poll(wgpu::PollType::Wait {
            submission_index: Some(index),
            timeout: None,
        });

        let data = slice.get_mapped_range();

        let row_bytes = (WIDTH * bytes_per_texel) as usize;
        let mut pixels = Vec::with_capacity(row_bytes * HEIGHT as usize);

        for row in 0..HEIGHT {
            let start = (row * bytes_per_row) as usize;
            pixels.extend_from_slice(&data[start..start + row_bytes]);
        }

        drop(data);
        readback.unmap();

        pixels
    }

    /// Decodes half-float bytes into `f32`.
    ///
    /// Hand-rolled rather than pulling in `half` for sixteen bits: IEEE 754
    /// binary16 is one sign bit, five exponent bits, ten mantissa bits.
    pub fn halves(bytes: &[u8]) -> Vec<f32> {
        bytes
            .chunks_exact(2)
            .map(|pair| {
                let bits = u16::from_le_bytes([pair[0], pair[1]]);

                let sign = if bits >> 15 == 1 { -1.0 } else { 1.0 };
                let exponent = i32::from((bits >> 10) & 0x1f);
                let mantissa = f32::from(bits & 0x3ff);

                if exponent == 0 {
                    // Subnormal: no implicit leading one.
                    sign * mantissa * 2.0f32.powi(-24)
                } else if exponent == 0x1f {
                    sign * f32::INFINITY
                } else {
                    sign * (1.0 + mantissa / 1024.0) * 2.0f32.powi(exponent - 15)
                }
            })
            .collect()
    }

    #[test]
    fn the_hdr_path_reproduces_the_standard_path_on_content_with_no_hdr_in_it() {
        // The invariant that makes this safe to ship: for a scene containing
        // nothing brighter than diffuse white -- which is every scene in the
        // framework today -- routing through a half-float intermediate and a
        // display transform must produce the image the direct path produces.
        //
        // It also catches a double-encode. `Display::SDR` carries the sRGB
        // transfer and an sRGB surface applies it in hardware too, so a
        // transform that ignored that would push mid-grey from 0.5 to 0.74
        // while leaving black and white exactly right.
        let Some(mut sdr) = renderer_for(wgpu::TextureFormat::Rgba8UnormSrgb) else {
            eprintln!("skipped: no wgpu adapter available");
            return;
        };
        let Some(mut hdr) = renderer_for(wgpu::TextureFormat::Rgba16Float) else {
            eprintln!("skipped: no wgpu adapter available");
            return;
        };

        scene(&mut sdr);

        let direct = present_and_read(
            &mut sdr,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            4,
            Display::SDR,
            false,
        );

        scene(&mut hdr);

        // Resolved onto a half-float surface, so the shader does the sRGB
        // encode itself and the two are directly comparable.
        let through = halves(&present_and_read(
            &mut hdr,
            wgpu::TextureFormat::Rgba16Float,
            8,
            Display::SDR,
            true,
        ));

        assert_eq!(direct.len(), through.len());

        let mut worst = 0.0f32;

        for (index, (byte, value)) in direct.iter().zip(&through).enumerate() {
            let expected = f32::from(*byte) / 255.0;
            let difference = (expected - value).abs();

            worst = worst.max(difference);

            assert!(
                difference < 0.02,
                "component {index}: direct {expected}, hdr path {value}"
            );
        }

        eprintln!("worst component difference between the two paths: {worst}");
    }

    #[test]
    fn the_hdr_path_leaves_headroom_above_diffuse_white() {
        // The property the intermediate exists for. On an extended surface
        // the encode is relative to SDR white, so ordinary white lands at
        // 1.0 with the panel's range still above it -- rather than at the
        // top of a buffer that clamps.
        let Some(mut renderer) = renderer_for(wgpu::TextureFormat::Rgba16Float) else {
            eprintln!("skipped: no wgpu adapter available");
            return;
        };

        scene(&mut renderer);

        let pixels = halves(&present_and_read(
            &mut renderer,
            wgpu::TextureFormat::Rgba16Float,
            8,
            Display::P3_EXTENDED,
            true,
        ));

        // The white square starts at (8, 8); sample its middle.
        let index = ((20 * WIDTH + 20) * 4) as usize;
        let white = pixels[index];

        assert!(
            (white - 1.0).abs() < 0.05,
            "diffuse white landed at {white} on an extended surface"
        );
    }

    #[test]
    fn a_renderer_that_cannot_hold_a_highlight_is_not_asked_to() {
        // The gate the compositor applies. An 8-bit engine must never be
        // routed through `present_hdr`, because its pipelines cannot draw
        // into a float intermediate at all -- the failure is a render-pass
        // validation error, not a wrong color.
        assert!(!aura_wgpu::display::Target::carries_hdr(
            wgpu::TextureFormat::Rgba8UnormSrgb
        ));
        assert!(aura_wgpu::display::Target::carries_hdr(
            wgpu::TextureFormat::Rgba16Float
        ));
    }
}

mod authoring {
    use super::present_path::*;
    use super::*;

    use aura_core::Renderer as _;
    use aura_core::{Rectangle, Size};
    use aura_design::tokens::LuminanceTokens;

    #[test]
    fn an_authored_highlight_reaches_the_surface_brighter_than_white() {
        // The end of the chain, and the first point at which any of this is
        // reachable from something a designer writes.
        //
        // A design token says 1,000 nits. That becomes a multiplier on a
        // `Quad`. The quad is drawn by the ordinary renderer into a
        // half-float frame, resolved through the display transform, and lands
        // on the surface still carrying five times the light of the white
        // square beside it -- which is a thing no `Color` in this framework
        // can express, because `Color` is bounded at 1.0 by construction.
        let Some(mut renderer) = renderer_for(wgpu::TextureFormat::Rgba16Float) else {
            eprintln!("skipped: no wgpu adapter available");
            return;
        };

        renderer.reset(Rectangle::with_size(Size::new(64.0, 64.0)));

        // Ordinary white, for comparison.
        renderer.fill_quad(
            aura_core::renderer::Quad {
                bounds: Rectangle {
                    x: 4.0,
                    y: 4.0,
                    width: 20.0,
                    height: 20.0,
                },
                ..Default::default()
            },
            Color::WHITE,
        );

        // The same white, emitting.
        renderer.fill_quad(
            aura_core::renderer::Quad {
                bounds: Rectangle {
                    x: 36.0,
                    y: 36.0,
                    width: 20.0,
                    height: 20.0,
                },
                luminance: LuminanceTokens::multiplier(LuminanceTokens::HDR.highlight),
                ..Default::default()
            },
            Color::WHITE,
        );

        let pixels = halves(&present_and_read(
            &mut renderer,
            wgpu::TextureFormat::Rgba16Float,
            8,
            Display::P3_EXTENDED,
            true,
        ));

        let sample = |x: u32, y: u32| pixels[((y * 64 + x) * 4) as usize];

        let plain = sample(14, 14);
        let emitting = sample(46, 46);

        assert!(
            (plain - 1.0).abs() < 0.05,
            "ordinary white should land at 1.0, got {plain}"
        );

        if !aura_wgpu::graphics::color::GAMMA_CORRECTION {
            // `web-colors` is a *default* feature of the `aura` crate, and
            // the root crate lists itself as a dev-dependency with defaults
            // on, so there is no way to build this suite without it today.
            // Under it the renderer blends sRGB-encoded components directly:
            // there is no linear light in the pipeline, and a luminance
            // multiplier has nothing meaningful to scale.
            //
            // The requirement is therefore that it be *ignored*, not applied
            // wrongly -- an emitting quad must look exactly like an ordinary
            // one rather than hue-shifted. Flipping that default is a
            // deliberate change that moves every blended pixel in the
            // framework, so it is not something this test should presume.
            assert!(
                (emitting - plain).abs() < 0.05,
                "without linear compositing a luminance multiplier must be \
                 ignored, not applied: plain {plain}, emitting {emitting}"
            );

            eprintln!(
                "skipped the HDR half: web-colors is on, so compositing is \
                 not linear and Quad::luminance is inert"
            );

            return;
        }

        assert!(
            emitting > 4.0,
            "a 1000-nit highlight should land near 4.93, got {emitting}"
        );

        let nits = Display::P3_EXTENDED.decode_nits(emitting);

        assert!(
            (nits - 1_000.0).abs() < 60.0,
            "the highlight reached the surface at {nits} nits"
        );
    }

    #[test]
    fn the_default_luminance_emits_nothing_extra() {
        // The safety half: a quad that says nothing about luminance must be
        // bit-identical to one that never had the field.
        assert!((aura_core::renderer::Quad::default().luminance - 1.0).abs() < f32::EPSILON);
        assert!(!LuminanceTokens::default().is_hdr());
    }
}

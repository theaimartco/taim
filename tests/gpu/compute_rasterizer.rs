//! Real GPU conformance test for the compute rasterizer (Pillars I and IX).
//!
//! Evaluates the multi-stage compute rasterizer pipeline (`path_coarse`, `coarse`, `fine`)
//! on real GPU hardware across 5,000 overlapping rounded rectangles with per-element clips.

use taim_core::Rectangle;
use taim_core::renderer::{Rasterizer, Settings};
use taim_raster::{CurveFlatteningPolicy, Encoder, MaterialId, ObjectVelocity, Rect, SceneEdit};
use taim_wgpu::Renderer;
use taim_wgpu::rasterizer::ComputeRasterizer;
use taim_wgpu::wgpu;

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY),
        flags: wgpu::InstanceFlags::empty(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });

    taim_futures::futures::executor::block_on(async {
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
                label: Some("compute_rasterizer.test"),
                ..Default::default()
            })
            .await
            .ok()
    })
}

#[test]
fn compute_rasterizer_dispatches_on_hardware_without_validation_error() {
    let Some((device, queue)) = device() else {
        eprintln!("skipped: no wgpu-compatible device available in this environment");
        return;
    };

    if !ComputeRasterizer::is_device_supported(&device) {
        eprintln!("skipped: device limits do not meet compute rasterizer requirements");
        return;
    }

    let mut rasterizer = ComputeRasterizer::new(&device);

    let mut encoder = Encoder::new();
    let policy = CurveFlatteningPolicy {
        tolerance: 0.1,
        max_depth: 6,
    };

    const COUNT: usize = 5_000;
    let mut clip_ids = Vec::with_capacity(COUNT);

    let mut seed: u64 = 0xABCD_EF01_2345_6789;
    let mut next_rand = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((seed >> 33) as f32) / ((1u32 << 31) as f32)
    };

    for i in 0..COUNT {
        let x = next_rand() * 1200.0;
        let y = next_rand() * 700.0;
        let w = 30.0 + next_rand() * 100.0;
        let h = 20.0 + next_rand() * 60.0;
        let r = 4.0 + next_rand() * 10.0;

        let rect = Rect::new([x, y], [x + w, y + h]);
        let path = encoder
            .push_rounded_rect(rect, [r; 4], Some(policy))
            .expect("path");

        let clip_rect = Rect::new([x + 1.0, y + 1.0], [x + w - 1.0, y + h - 1.0]);
        let clip_path = encoder
            .push_rounded_rect(clip_rect, [r * 0.5; 4], Some(policy))
            .expect("clip path");
        let parent = if i > 0 && i % 8 == 0 {
            Some(clip_ids[i - 1])
        } else {
            None
        };
        let clip = encoder.push_clip(clip_path, parent).expect("clip");

        encoder
            .push_draw_with_velocity(
                path,
                Some(clip),
                MaterialId((i % 16) as u32),
                0.9,
                ObjectVelocity {
                    linear: [10.0, 20.0],
                    angular: 0.0,
                    scale_rate: 0.0,
                },
            )
            .expect("draw");

        clip_ids.push(clip);
    }

    let mut scene = encoder.finish();
    assert_eq!(scene.draws().len(), COUNT);

    // Delta update check (Pillar IX)
    let edit = SceneEdit::UpdateVelocity {
        draw_index: 100,
        velocity: ObjectVelocity {
            linear: [50.0, -25.0],
            angular: 1.0,
            scale_rate: 0.0,
        },
    };
    assert_eq!(scene.delta_upload_bytes(&edit), 16);
    scene.apply_edit(&edit);

    let width = 1280;
    let height = 720;

    let target_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("compute_rasterizer output texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let target_view = target_texture.create_view(&wgpu::TextureViewDescriptor::default());

    let mut cmd_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("compute_rasterizer command encoder"),
    });

    rasterizer.dispatch(
        &device,
        &queue,
        &mut cmd_encoder,
        &scene,
        &target_view,
        width,
        height,
        Some(Rectangle {
            x: 0.0,
            y: 0.0,
            width: width as f32,
            height: height as f32,
        }),
    );

    let submission = queue.submit([cmd_encoder.finish()]);
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: Some(submission),
        timeout: None,
    });
}

#[test]
fn renderer_activates_compute_rasterizer_when_configured() {
    let Some((device, _)) = device() else {
        eprintln!("skipped: no wgpu device");
        return;
    };

    if !ComputeRasterizer::is_device_supported(&device) {
        eprintln!("skipped: device does not support compute rasterizer");
        return;
    }

    let settings = Settings {
        rasterizer: Rasterizer::Compute,
        ..Settings::default()
    };

    let Some(mut renderer) = taim_futures::futures::executor::block_on(
        <Renderer as taim_core::renderer::Headless>::new(settings, Some("wgpu")),
    ) else {
        eprintln!("skipped: failed to create headless renderer");
        return;
    };

    assert!(
        renderer.compute_rasterizer().is_some(),
        "Renderer configured with Rasterizer::Compute must initialize ComputeRasterizer"
    );
}

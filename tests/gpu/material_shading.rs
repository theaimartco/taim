//! CPU/GPU conformance for the material shading model.
//!
//! `core/src/material.rs` and `wgpu/src/shader/material.wgsl` implement the
//! same physics twice: once for anything reasoning about materials in Rust,
//! and once for the pixels a person looks at. Two implementations of the same
//! arithmetic drift, and a drift here is invisible — the frame still renders,
//! and the only symptom is that a surface responds to light slightly wrongly
//! in a way nobody can attribute.
//!
//! The shader is compiled from `material.wgsl` itself with test entry points
//! appended, so there is one source of truth for the physics rather than a
//! copy that can rot.
//!
//! Skipped rather than failed when no adapter exists.

use taim_core::material::{
    self, Material, fresnel_schlick, ggx_distribution, refraction_offset_mm,
};
use taim_wgpu::wgpu;

/// Full precision: the point is to compare arithmetic, not measure rounding.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;

/// Entry points appended to `material.wgsl`.
///
/// Each texel carries one case: `(cos_theta, roughness, ior, thickness_mm)`.
/// Dispersion is not varied here — `dispersed_ior` is a linear function of it
/// and is covered on the Rust side.
const HARNESS: &str = r#"
@group(0) @binding(0) var u_sampler: sampler;
@group(1) @binding(0) var u_cases: texture_2d<f32>;

var<private> corners: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, 0.0), vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 1.0),
);

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let corner = corners[index];
    var out: VertexOutput;
    out.uv = corner;
    out.position = vec4<f32>(corner * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let case_data = textureSample(u_cases, u_sampler, input.uv);

    var m: Material;
    m.roughness = case_data.y;
    m.thickness_mm = case_data.w;
    m.ior = case_data.z;
    m.dispersion = 0.0;
    m.transmission = 1.0;

    let cos_theta = case_data.x;

    return vec4<f32>(
        fresnel_schlick(cos_theta, normal_reflectance(m)),
        ggx_distribution(cos_theta, m.roughness),
        refraction_offset_mm(cos_theta, m),
        normal_reflectance(m),
    );
}
"#;

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
                label: Some("material_shading.test"),
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
            .ok()
    })
}

/// One case: view angle and material.
#[derive(Debug, Clone, Copy)]
struct Case {
    cos_theta: f32,
    material: Material,
}

/// Runs every case through the shader and returns its four outputs.
fn on_gpu(device: &wgpu::Device, queue: &wgpu::Queue, cases: &[Case]) -> Vec<[f32; 4]> {
    let width = cases.len() as u32;

    let descriptor = |usage| wgpu::TextureDescriptor {
        label: Some("material_shading.test.texture"),
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

    let texels: Vec<u8> = cases
        .iter()
        .flat_map(|case| {
            [
                case.cos_theta,
                case.material.roughness,
                case.material.ior,
                case.material.thickness_mm,
            ]
        })
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

    // One source of truth for the physics: the shader under test *is*
    // `material.wgsl`, with entry points appended.
    let module = format!(
        "{}\n{}",
        include_str!("../../wgpu/src/shader/material.wgsl"),
        HARNESS
    );

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("material_shading.test.shader"),
        source: wgpu::ShaderSource::Wgsl(module.into()),
    });

    let sampler_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
            count: None,
        }],
    });

    let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        }],
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(&sampler_layout), Some(&texture_layout)],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

    let sampler_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &sampler_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Sampler(&sampler),
        }],
    });

    let texture_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &texture_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(
                &source.create_view(&wgpu::TextureViewDescriptor::default()),
            ),
        }],
    });

    let bytes_per_row = (width * 16).div_ceil(256) * 256;

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(bytes_per_row),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.create_view(&wgpu::TextureViewDescriptor::default()),
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &sampler_group, &[]);
        pass.set_bind_group(1, &texture_group, &[]);
        pass.draw(0..6, 0..1);
    }

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

    let out = (0..cases.len())
        .map(|case| {
            [
                floats[case * 4],
                floats[case * 4 + 1],
                floats[case * 4 + 2],
                floats[case * 4 + 3],
            ]
        })
        .collect();

    drop(data);
    readback.unmap();

    out
}

/// Every material, across the full range of viewing angles.
fn sweep() -> Vec<Case> {
    let materials = [
        Material::OPAQUE,
        Material::GLASS,
        Material::FROSTED,
        Material::POLISHED,
    ];

    let mut cases = Vec::new();

    for material in materials {
        for step in 0..=10u8 {
            cases.push(Case {
                cos_theta: f32::from(step) / 10.0,
                material,
            });
        }
    }

    cases
}

#[test]
fn the_shader_agrees_with_the_reference() {
    let Some((device, queue)) = device() else {
        eprintln!("skipped: no wgpu adapter available");
        return;
    };

    let cases = sweep();
    let gpu = on_gpu(&device, &queue, &cases);

    for (case, actual) in cases.iter().zip(gpu) {
        let f0 = case.material.normal_reflectance();

        let expected = [
            fresnel_schlick(case.cos_theta, f0),
            ggx_distribution(case.cos_theta, case.material.roughness),
            refraction_offset_mm(case.cos_theta, case.material),
            f0,
        ];

        for channel in 0..4 {
            let difference = (actual[channel] - expected[channel]).abs();
            let tolerance = expected[channel].abs().max(1.0) * 2e-3;

            assert!(
                difference <= tolerance,
                "channel {channel} of {case:?}\n  gpu = {actual:?}\n  cpu = {expected:?}"
            );
        }
    }
}

#[test]
fn the_shader_produces_no_nan_for_any_authored_material() {
    // Materials are authored, and authored values arrive out of range. A NaN
    // reaching a lighting pass is a frame of garbage.
    let Some((device, queue)) = device() else {
        eprintln!("skipped: no wgpu adapter available");
        return;
    };

    let nonsense = [
        Material {
            roughness: -1.0,
            ..Material::GLASS
        },
        Material {
            roughness: 50.0,
            ..Material::GLASS
        },
        Material {
            ior: 0.2,
            ..Material::GLASS
        },
        Material {
            thickness_mm: -5.0,
            ..Material::GLASS
        },
        Material {
            ior: material::IOR_AIR,
            ..Material::GLASS
        },
    ];

    let cases: Vec<Case> = nonsense
        .iter()
        .flat_map(|material| {
            [0.0_f32, 0.5, 1.0].map(|cos_theta| Case {
                cos_theta,
                material: *material,
            })
        })
        .collect();

    for output in on_gpu(&device, &queue, &cases) {
        assert!(
            output.iter().all(|value| value.is_finite()),
            "an out-of-range material produced {output:?}"
        );
    }
}

#[test]
fn deferred_gbuffer_lighting_pass_executes_on_hardware() {
    let Some((device, queue)) = device() else {
        eprintln!("skipped: no wgpu adapter available");
        return;
    };

    let width = 64;
    let height = 64;
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let gbuffer = taim_wgpu::GBuffer::new(&device, format, width, height);

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("test_target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let backdrop = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("test_backdrop"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let backdrop_view = backdrop.create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("gbuffer_test_encoder"),
    });

    gbuffer.render_lighting(
        &device,
        &queue,
        &mut encoder,
        &target_view,
        &backdrop_view,
        taim_core::material::SceneLight::KEY,
        4.0,
    );

    let submission = queue.submit([encoder.finish()]);
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: Some(submission),
        timeout: None,
    });
}

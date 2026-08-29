//! The display transform, on the GPU.
//!
//! Applies [`aura_core::color::DisplayTransform`] to a rendered frame: tone
//! map into the panel's luminance range, gamut map into its primaries, encode
//! with its transfer function.
//!
//! The arithmetic lives in `shader/display.wgsl` and is a mirror of the Rust
//! implementation rather than a second design. `tests/gpu/display_transform.rs`
//! holds the two together, because a divergence between them is invisible
//! until someone photographs a panel.

use std::borrow::Cow;

use crate::core::color::display::{Display, DisplayTransform, GamutMap, ToneMap};
use crate::core::color::space::{self, ColorSpace, Transfer};

/// The uniform block `shader/display.wgsl` reads.
///
/// Every field is a `vec4` so the layout needs no padding reasoning: WGSL
/// aligns `vec4<f32>` to 16 bytes, and a struct of nothing but `vec4`s is the
/// one layout that cannot be got subtly wrong.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
struct Uniforms {
    conversion_0: [f32; 4],
    conversion_1: [f32; 4],
    conversion_2: [f32; 4],
    compositing: [f32; 4],
    display: [f32; 4],
    levels: [f32; 4],
    modes: [f32; 4],
}

impl Uniforms {
    fn new(transform: &DisplayTransform) -> Self {
        let matrix = transform.conversion_matrix();
        let display = transform.display();

        // `glam` is column-major; the shader dots each *row* against the
        // scene, so transpose exactly once, here.
        let row = |index: usize| {
            [
                matrix.x_axis[index],
                matrix.y_axis[index],
                matrix.z_axis[index],
                0.0,
            ]
        };

        let compositing = DisplayTransform::luminance_vector(ColorSpace::COMPOSITING.primaries);
        let panel = DisplayTransform::luminance_vector(display.color_space.primaries);

        Self {
            conversion_0: row(0),
            conversion_1: row(1),
            conversion_2: row(2),
            compositing: [
                compositing.x,
                compositing.y,
                compositing.z,
                transform.source_peak_nits(),
            ],
            display: [panel.x, panel.y, panel.z, display.peak_nits()],
            levels: [
                display.black_nits,
                display.reference_white_nits,
                space::DIFFUSE_WHITE_NITS,
                0.0,
            ],
            modes: [
                match transform.tone_map() {
                    ToneMap::Bt2390 => 0.0,
                    ToneMap::Clip => 1.0,
                },
                match transform.gamut_map() {
                    GamutMap::Desaturate => 0.0,
                    GamutMap::Clip => 1.0,
                },
                match display.color_space.transfer {
                    Transfer::Linear => 0.0,
                    Transfer::Srgb => 1.0,
                    Transfer::Pq => 2.0,
                    Transfer::Hlg => 3.0,
                },
                0.0,
            ],
        }
    }
}

/// A pipeline that applies a [`DisplayTransform`] to a texture.
#[derive(Debug)]
pub struct Pipeline {
    pipeline: wgpu::RenderPipeline,
    constant_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniforms: wgpu::Buffer,
}

impl Pipeline {
    /// Creates a [`Pipeline`] writing to `format`.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("aura_wgpu.display.sampler"),
            ..wgpu::SamplerDescriptor::default()
        });

        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("aura_wgpu.display.uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let constant_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("aura_wgpu.display.constant_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("aura_wgpu.display.texture_layout"),
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

        // Two bind groups, never three: the portability floor guarantees only
        // `max_bind_groups == 2`, and the mask pipeline already learned that
        // the hard way (`Capstone.md`, F-07).
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("aura_wgpu.display.pipeline_layout"),
            bind_group_layouts: &[Some(&constant_layout), Some(&texture_layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("aura_wgpu.display.shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader/display.wgsl"))),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("aura_wgpu.display.pipeline"),
            layout: Some(&pipeline_layout),
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
                    format,
                    // Replace, never blend. This is the last stage: the
                    // result *is* the frame, and blending it against whatever
                    // the target held would mix an encoded signal with a
                    // scene-referred one.
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            constant_layout,
            texture_layout,
            sampler,
            uniforms,
        }
    }

    /// Reads `source`, applies `transform`, and writes the result to `target`.
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::TextureView,
        target: &wgpu::TextureView,
        transform: &DisplayTransform,
    ) {
        queue.write_buffer(
            &self.uniforms,
            0,
            bytemuck::bytes_of(&Uniforms::new(transform)),
        );

        let constants = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("aura_wgpu.display.constant_bind_group"),
            layout: &self.constant_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.uniforms.as_entire_binding(),
                },
            ],
        });

        let texture = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("aura_wgpu.display.texture_bind_group"),
            layout: &self.texture_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(source),
            }],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("aura_wgpu.display.render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
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

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &constants, &[]);
        pass.set_bind_group(1, &texture, &[]);
        pass.draw(0..6, 0..1);
    }
}

/// Everything about a render target that colour depends on.
///
/// # Why format alone is not enough
///
/// A `wgpu::TextureFormat` says how many bits a channel has and whether the
/// hardware applies a transfer function on write. It does not say what the
/// values *mean*: `Rgba16Float` is the format of an scRGB surface, a
/// scene-referred intermediate, and a PQ-encoded HDR10 buffer, and those three
/// want completely different final transforms.
///
/// So the target's colour semantics are carried alongside its format, in one
/// place, rather than inferred at each site that needs them.
///
/// # The double-encode this prevents
///
/// An `*_Srgb` format applies the sRGB transfer **in hardware** on every
/// write. A shader that also encodes produces a frame encoded twice, which
/// leaves black and white exactly right and pushes mid-grey from 0.5 to 0.74 --
/// so it reads as a badly tuned theme rather than as a bug, and it is
/// invisible to any test that samples only the extremes.
///
/// [`TargetColorState::encodes_in_hardware`] is the single source of truth for
/// that question. Nothing else in the renderer should ask a format whether it
/// is sRGB.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TargetColorState {
    format: wgpu::TextureFormat,
    display: Display,
}

impl TargetColorState {
    /// Describes a target of `format` presenting to `display`.
    #[must_use]
    pub fn new(format: wgpu::TextureFormat, display: Display) -> Self {
        Self { format, display }
    }

    /// The target's texture format.
    #[must_use]
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// The panel this target presents to.
    #[must_use]
    pub fn display(&self) -> Display {
        self.display
    }

    /// Whether writing to this target applies a transfer function in
    /// hardware.
    ///
    /// When true, whatever the renderer writes must be **linear**: the
    /// hardware supplies the encode. When false, the renderer must encode.
    /// Exactly one of the two, always.
    #[must_use]
    pub fn encodes_in_hardware(&self) -> bool {
        self.format.is_srgb()
    }

    /// Whether a component above `1.0` survives being written here.
    ///
    /// Every `Unorm` format clamps whatever its precision, so extra bits are
    /// not extra range: only a float format can hold a highlight.
    #[must_use]
    pub fn carries_hdr(&self) -> bool {
        matches!(
            self.format,
            wgpu::TextureFormat::Rgba16Float | wgpu::TextureFormat::Rgba32Float
        )
    }

    /// The display transform this target needs.
    ///
    /// Three cases, and the middle one is the double-encode guard:
    ///
    /// - a float target carries the full range, so it gets the real panel's
    ///   transform;
    /// - an sRGB target has the hardware encode already, so the transform is
    ///   asked for *linear* output;
    /// - anything else is a plain standard-range target and encodes in the
    ///   shader.
    #[must_use]
    pub fn transform(&self) -> DisplayTransform {
        if self.carries_hdr() {
            return DisplayTransform::new(self.display);
        }

        if self.encodes_in_hardware() {
            let mut linear = Display::SDR;
            linear.color_space.transfer = Transfer::Linear;

            return DisplayTransform::new(linear);
        }

        DisplayTransform::new(Display::SDR)
    }
}

/// Returns the transform a surface of `format` needs, for `display`.
///
/// Thin wrapper over [`TargetColorState::transform`], kept because most
/// callers have a format and a display and nothing else.
#[must_use]
pub fn transform_for(format: wgpu::TextureFormat, display: Display) -> DisplayTransform {
    TargetColorState::new(format, display).transform()
}

/// An offscreen half-float frame, plus the pipeline that resolves it onto a
/// surface.
///
/// The renderer draws linear light and relies on an sRGB surface to encode it
/// in hardware, which works and clamps everything above `1.0`. To keep that
/// range, the frame is drawn here instead -- into a target that can hold it --
/// and then resolved through the display transform onto the real surface.
#[derive(Debug)]
pub struct Target {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    pipeline: Pipeline,
    width: u32,
    height: u32,
}

impl Target {
    /// The format an intermediate frame is held in when the goal is HDR.
    ///
    /// Half float, not `Rgba32Float`: it carries every value a panel can show
    /// with room to spare, at half the bandwidth of full float, and it is
    /// renderable and filterable everywhere without an opt-in feature.
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    /// Returns whether a frame drawn in `format` can carry values above
    /// diffuse white.
    ///
    /// Prefer [`TargetColorState::carries_hdr`] where a full target
    /// description is to hand; this is the format-only form.
    #[must_use]
    pub fn carries_hdr(format: wgpu::TextureFormat) -> bool {
        TargetColorState::new(format, Display::SDR).carries_hdr()
    }

    /// Creates a [`Target`] of `width` by `height` in `format`, resolving onto
    /// `surface_format`.
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("aura_wgpu.display.target"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            texture,
            view,
            pipeline: Pipeline::new(device, surface_format),
            width: width.max(1),
            height: height.max(1),
        }
    }

    /// Returns whether this target is already the right size.
    #[must_use]
    pub fn fits(&self, width: u32, height: u32) -> bool {
        self.width == width.max(1) && self.height == height.max(1)
    }

    /// The texture the frame is drawn into.
    #[must_use]
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    /// A view of the texture the frame is drawn into.
    #[must_use]
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Applies `transform` to the drawn frame and writes it to `surface`.
    pub fn resolve(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        surface: &wgpu::TextureView,
        transform: &DisplayTransform,
    ) {
        self.pipeline
            .render(device, queue, encoder, &self.view, surface, transform);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every format the renderer can select for a surface or intermediate.
    const FORMATS: &[wgpu::TextureFormat] = &[
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Bgra8UnormSrgb,
        wgpu::TextureFormat::Rgba16Float,
    ];

    #[test]
    fn a_frame_is_encoded_exactly_once() {
        // The invariant that makes double-encoding impossible to reintroduce:
        // if the hardware encodes, the transform must be asked for linear
        // output; if it does not, the transform must encode.
        for format in FORMATS {
            let state = TargetColorState::new(*format, Display::SDR);
            let transfer = state.transform().display().color_space.transfer;

            if state.encodes_in_hardware() {
                assert_eq!(
                    transfer,
                    Transfer::Linear,
                    "{format:?} encodes in hardware, so the shader must not"
                );
            } else {
                assert_ne!(
                    transfer,
                    Transfer::Linear,
                    "{format:?} does not encode in hardware, so the shader must"
                );
            }
        }
    }

    #[test]
    fn only_float_targets_carry_a_highlight() {
        for format in FORMATS {
            let state = TargetColorState::new(*format, Display::HDR10_1000);

            assert_eq!(
                state.carries_hdr(),
                *format == wgpu::TextureFormat::Rgba16Float,
                "{format:?} reported the wrong range capability"
            );
        }
    }

    #[test]
    fn a_float_target_keeps_the_panel_it_was_given() {
        // A half-float target is the only one that can present the real
        // panel's transform; everything else is standard range whatever
        // display is attached, because the buffer cannot carry more.
        let hdr = TargetColorState::new(wgpu::TextureFormat::Rgba16Float, Display::HDR10_1000);
        assert_eq!(hdr.transform().display(), Display::HDR10_1000);

        let sdr = TargetColorState::new(wgpu::TextureFormat::Bgra8Unorm, Display::HDR10_1000);
        assert_eq!(
            sdr.transform().display().peak_nits(),
            Display::SDR.peak_nits()
        );
    }

    #[test]
    fn ten_bit_precision_is_not_extra_range() {
        // A recurring misconception, and an expensive one: `Rgb10a2Unorm` has
        // more precision than 8-bit and exactly the same range, because it
        // still clamps at 1.0.
        let state = TargetColorState::new(wgpu::TextureFormat::Rgb10a2Unorm, Display::HDR10_1000);

        assert!(!state.carries_hdr());
    }
}

use super::atlas;
use super::{Batch, Cache, Image, Pipeline};

use crate::Buffer;
#[cfg(feature = "svg")]
use crate::core::Size;
use crate::core::border;
use crate::core::{Rectangle, Transformation};

use bytemuck::{Pod, Zeroable};

use std::mem;
use std::ops::Range;
use std::sync::Arc;

/// Mutable image preparation state with stable CPU/GPU buffer reuse.
#[derive(Default)]
pub struct State {
    layers: Vec<Layer>,
    active_layer: usize,
    instances: Vec<Instance>,
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        pipeline: &Pipeline,
        device: &wgpu::Device,
        belt: &mut wgpu::util::StagingBelt,
        encoder: &mut wgpu::CommandEncoder,
        cache: &mut Cache,
        images: &Batch,
        transformation: Transformation,
        scale: f32,
    ) {
        if self.layers.len() == self.active_layer {
            self.layers.push(pipeline.create_state_layer(device));
        }

        let layer = &mut self.layers[self.active_layer];
        for image in images {
            match image {
                #[cfg(feature = "image")]
                Image::Raster {
                    image,
                    bounds,
                    clip_bounds,
                } => {
                    let bounds = (*bounds * scale).round();
                    let clip_bounds = (*clip_bounds * scale).round();
                    if !drawable_paint(bounds, clip_bounds, image.opacity) {
                        // Do not upload completely invisible or invalid
                        // image data to the GPU atlas.
                        continue;
                    }

                    let Some((entry, atlas)) =
                        cache.upload_raster(device, encoder, belt, &image.handle)
                    else {
                        continue;
                    };
                    let sampling = match image.filter_method {
                        crate::core::image::FilterMethod::Nearest => Sampling::Nearest,
                        crate::core::image::FilterMethod::Linear => Sampling::Linear,
                    };
                    let start = self.instances.len();
                    add_instances(
                        bounds,
                        clip_bounds,
                        image.border_radius * scale,
                        f32::from(image.rotation),
                        image.opacity,
                        entry,
                        &mut self.instances,
                    );
                    layer.record(atlas, sampling, start..self.instances.len());
                }
                #[cfg(not(feature = "image"))]
                Image::Raster { .. } => {}

                #[cfg(feature = "svg")]
                Image::Vector {
                    svg,
                    bounds,
                    clip_bounds,
                } => {
                    let bounds = (*bounds * scale).round();
                    let clip_bounds = (*clip_bounds * scale).round();
                    if !drawable_paint(bounds, clip_bounds, svg.opacity) {
                        continue;
                    }

                    let Some((entry, atlas)) = cache.upload_vector(
                        device,
                        encoder,
                        belt,
                        &svg.handle,
                        svg.color,
                        Size::new(bounds.width as u32, bounds.height as u32),
                    ) else {
                        continue;
                    };
                    let start = self.instances.len();
                    add_instances(
                        bounds,
                        clip_bounds,
                        border::radius(0.0),
                        f32::from(svg.rotation),
                        svg.opacity,
                        entry,
                        &mut self.instances,
                    );
                    layer.record(atlas, Sampling::Nearest, start..self.instances.len());
                }
                #[cfg(not(feature = "svg"))]
                Image::Vector { .. } => {}
            }
        }

        layer.upload(device, encoder, belt, transformation, &self.instances);
        self.instances.clear();
        self.active_layer += 1;
    }

    pub fn render<'a>(
        &'a self,
        pipeline: &'a Pipeline,
        layer: usize,
        bounds: Rectangle<u32>,
        render_pass: &mut wgpu::RenderPass<'a>,
    ) {
        let Some(layer) = self.layers.get(layer) else {
            return;
        };

        render_pass.set_pipeline(&pipeline.raw);
        render_pass.set_scissor_rect(bounds.x, bounds.y, bounds.width, bounds.height);
        layer.render(render_pass);
    }

    pub fn trim(&mut self) {
        for layer in &mut self.layers[..self.active_layer] {
            layer.clear();
        }
        self.active_layer = 0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sampling {
    Nearest,
    Linear,
}

#[derive(Debug)]
struct DrawGroup {
    atlas: Arc<wgpu::BindGroup>,
    sampling: Sampling,
    instances: Range<u32>,
}

#[derive(Debug)]
pub(super) struct Layer {
    uniforms: wgpu::Buffer,
    instances: Buffer<Instance>,
    nearest_constants: wgpu::BindGroup,
    linear_constants: wgpu::BindGroup,
    groups: Vec<DrawGroup>,
}

impl Layer {
    pub fn new(
        device: &wgpu::Device,
        constant_layout: &wgpu::BindGroupLayout,
        nearest_sampler: &wgpu::Sampler,
        linear_sampler: &wgpu::Sampler,
    ) -> Self {
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taim.image.uniforms"),
            size: mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let instances = Buffer::new(
            device,
            "taim.image.instances",
            Instance::INITIAL_CAPACITY,
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        );
        let nearest_constants = constants(device, constant_layout, &uniforms, nearest_sampler);
        let linear_constants = constants(device, constant_layout, &uniforms, linear_sampler);

        Self {
            uniforms,
            instances,
            nearest_constants,
            linear_constants,
            groups: Vec::new(),
        }
    }

    fn record(
        &mut self,
        atlas: &Arc<wgpu::BindGroup>,
        sampling: Sampling,
        instances: Range<usize>,
    ) {
        if instances.is_empty() {
            return;
        }

        let Ok(start) = u32::try_from(instances.start) else {
            return;
        };
        let Ok(end) = u32::try_from(instances.end) else {
            return;
        };
        if let Some(group) = self.groups.last_mut()
            && group.sampling == sampling
            && Arc::ptr_eq(&group.atlas, atlas)
            && group.instances.end == start
        {
            group.instances.end = end;
            return;
        }

        self.groups.push(DrawGroup {
            atlas: Arc::clone(atlas),
            sampling,
            instances: start..end,
        });
    }

    fn upload(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        transformation: Transformation,
        instances: &[Instance],
    ) {
        let uniforms = Uniforms {
            transform: transformation.into(),
        };
        let uniform_bytes = bytemuck::bytes_of(&uniforms);
        belt.write_buffer(
            encoder,
            &self.uniforms,
            0,
            wgpu::BufferSize::new(uniform_bytes.len() as u64).expect("non-empty image uniforms"),
        )
        .copy_from_slice(uniform_bytes);

        let _ = self.instances.resize(device, instances.len());
        if !instances.is_empty() {
            let _ = self.instances.write(encoder, belt, 0, instances);
        }
    }

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_vertex_buffer(0, self.instances.slice(..));
        let mut current_sampling = None;
        let mut current_atlas: Option<&Arc<wgpu::BindGroup>> = None;

        for group in &self.groups {
            if current_sampling != Some(group.sampling) {
                let constants = match group.sampling {
                    Sampling::Nearest => &self.nearest_constants,
                    Sampling::Linear => &self.linear_constants,
                };
                render_pass.set_bind_group(0, constants, &[]);
                current_sampling = Some(group.sampling);
            }

            if current_atlas.is_none_or(|atlas| !Arc::ptr_eq(atlas, &group.atlas)) {
                render_pass.set_bind_group(1, group.atlas.as_ref(), &[]);
                current_atlas = Some(&group.atlas);
            }
            render_pass.draw(0..6, group.instances.clone());
        }
    }

    fn clear(&mut self) {
        self.groups.clear();
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
pub(super) struct Instance {
    center: [f32; 2],
    clip_bounds: [f32; 4],
    border_radius: [f32; 4],
    tile: [f32; 4],
    rotation_cos_sin: [f32; 2],
    opacity: f32,
    atlas_position: [f32; 2],
    atlas_scale: [f32; 2],
    layer: i32,
}

impl Instance {
    const INITIAL_CAPACITY: usize = 32;
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
pub(super) struct Uniforms {
    transform: [f32; 16],
}

/// TAIM image-atlas admission: reject invisible primitives before the
/// CPU/GPU upload, including malformed coordinates and clips.
///
/// Do not cull by rectangle overlap: image rotation can move visible pixels
/// into a clip even if the unrotated bounds appear out of view.
#[inline]
fn drawable_paint(bounds: Rectangle, clip: Rectangle, opacity: f32) -> bool {
    [
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        clip.x,
        clip.y,
        clip.width,
        clip.height,
        opacity,
    ]
    .iter()
    .all(|value| value.is_finite())
        && bounds.width >= 1.0
        && bounds.height >= 1.0
        && clip.width > 0.0
        && clip.height > 0.0
        && opacity > 0.0
}

fn add_instances(
    bounds: Rectangle,
    clip_bounds: Rectangle,
    border_radius: border::Radius,
    rotation: f32,
    opacity: f32,
    entry: &atlas::Entry,
    instances: &mut Vec<Instance>,
) {
    let center = [
        bounds.x + bounds.width * 0.5,
        bounds.y + bounds.height * 0.5,
    ];
    let clip_bounds = [
        clip_bounds.x,
        clip_bounds.y,
        clip_bounds.width,
        clip_bounds.height,
    ];
    let border_radius = border_radius.into();
    let (sin, cos) = (-rotation).sin_cos();
    let rotation_cos_sin = [cos, sin];

    match entry {
        atlas::Entry::Contiguous(allocation) => push_instance(
            center,
            clip_bounds,
            border_radius,
            [bounds.x, bounds.y, bounds.width, bounds.height],
            rotation_cos_sin,
            opacity,
            allocation,
            instances,
        ),
        atlas::Entry::Fragmented { fragments, size } => {
            let scale_x = bounds.width / size.width as f32;
            let scale_y = bounds.height / size.height as f32;
            instances.reserve(fragments.len());

            for fragment in fragments {
                let allocation = &fragment.allocation;
                let allocation_size = allocation.size();
                push_instance(
                    center,
                    clip_bounds,
                    border_radius,
                    [
                        bounds.x + fragment.position.0 as f32 * scale_x,
                        bounds.y + fragment.position.1 as f32 * scale_y,
                        allocation_size.width as f32 * scale_x,
                        allocation_size.height as f32 * scale_y,
                    ],
                    rotation_cos_sin,
                    opacity,
                    allocation,
                    instances,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn push_instance(
    center: [f32; 2],
    clip_bounds: [f32; 4],
    border_radius: [f32; 4],
    tile: [f32; 4],
    rotation_cos_sin: [f32; 2],
    opacity: f32,
    allocation: &atlas::Allocation,
    instances: &mut Vec<Instance>,
) {
    let position = allocation.position();
    let size = allocation.size();
    let atlas_size = allocation.atlas_size() as f32;

    instances.push(Instance {
        center,
        clip_bounds,
        border_radius,
        tile,
        rotation_cos_sin,
        opacity,
        atlas_position: [
            position.0 as f32 / atlas_size,
            position.1 as f32 / atlas_size,
        ],
        atlas_scale: [
            size.width as f32 / atlas_size,
            size.height as f32 / atlas_size,
        ],
        layer: allocation.layer() as i32,
    });
}

fn constants(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniforms: &wgpu::Buffer,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("taim.image.constants"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

#[cfg(test)]
mod taim_image_admission_tests {
    use super::*;
    use crate::core::Point;
    use crate::core::Size;

    #[test]
    fn fully_transparent_images_skip_atlas_upload() {
        let rect = Rectangle::new(Point::ORIGIN, Size::new(12.0, 8.0));
        assert!(drawable_paint(rect, rect, 1.0));
        assert!(!drawable_paint(rect, rect, 0.0));
        assert!(!drawable_paint(rect, rect, -1.0));
    }

    #[test]
    fn malformed_clip_and_transforms_cannot_poison_atlas() {
        let rect = Rectangle::new(Point::ORIGIN, Size::new(12.0, 8.0));
        let invalid = Rectangle::new(Point::new(f32::NAN, 0.0), Size::new(12.0, 8.0));
        let empty_clip = Rectangle::new(Point::ORIGIN, Size::new(0.0, 8.0));
        assert!(!drawable_paint(invalid, rect, 1.0));
        assert!(!drawable_paint(rect, invalid, 1.0));
        assert!(!drawable_paint(rect, empty_clip, 1.0));
        assert!(!drawable_paint(rect, rect, f32::INFINITY));
    }
}

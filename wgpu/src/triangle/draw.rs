use super::Uniforms;
use super::pipeline::{gradient, solid};

use crate::Buffer;
use crate::core::{Point, Rectangle, Transformation, Vector};
use crate::graphics::mesh::{self, Mesh};

use std::ops::Range;

pub(super) const INITIAL_INDEX_COUNT: usize = 1_000;
pub(super) const INITIAL_VERTEX_COUNT: usize = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Solid,
    Gradient,
}

#[derive(Debug)]
struct Draw {
    kind: Kind,
    vertices: Range<usize>,
    indices: Range<usize>,
    uniform: usize,
    clip_bounds: Rectangle,
}

/// GPU buffers and a compact draw plan for one mesh group.
#[derive(Debug)]
pub(super) struct Layer {
    index_buffer: Buffer<u32>,
    solid: solid::Buffers,
    gradient: gradient::Buffers,
    draws: Vec<Draw>,
    solid_uniforms: Vec<Uniforms>,
    gradient_uniforms: Vec<Uniforms>,
}

impl Layer {
    pub fn new(
        device: &wgpu::Device,
        solid: &solid::Pipeline,
        gradient: &gradient::Pipeline,
    ) -> Self {
        Self {
            index_buffer: Buffer::new(
                device,
                "taim.triangle.indices",
                INITIAL_INDEX_COUNT,
                wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            ),
            solid: solid::Buffers::new(device, &solid.constants_layout),
            gradient: gradient::Buffers::new(device, &gradient.constants_layout),
            draws: Vec::new(),
            solid_uniforms: Vec::new(),
            gradient_uniforms: Vec::new(),
        }
    }

    pub fn prepare_geometry(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        solid_pipeline: &solid::Pipeline,
        gradient_pipeline: &gradient::Pipeline,
        meshes: &[Mesh],
        transformation: Transformation,
        opacity: f32,
    ) {
        let count = mesh::attribute_count_of(meshes);
        let _ = self.index_buffer.resize(device, count.indices);
        let _ = self.solid.vertices.resize(device, count.solid_vertices);
        let _ = self
            .gradient
            .vertices
            .resize(device, count.gradient_vertices);
        self.resize_uniforms(device, solid_pipeline, gradient_pipeline, count);
        self.rebuild_draws(meshes, count);
        self.solid_uniforms
            .reserve(count.solids.saturating_sub(self.solid_uniforms.len()));
        self.gradient_uniforms
            .reserve(count.gradients.saturating_sub(self.gradient_uniforms.len()));

        let _ = self.index_buffer.write_many(
            encoder,
            belt,
            0,
            count.indices,
            meshes
                .iter()
                .filter(|mesh| !mesh.is_empty())
                .map(Mesh::indices),
        );
        let _ = self.solid.vertices.write_many(
            encoder,
            belt,
            0,
            count.solid_vertices,
            meshes.iter().filter_map(|mesh| match mesh {
                Mesh::Solid { buffers, .. } if !mesh.is_empty() => {
                    Some(buffers.vertices.as_slice())
                }
                Mesh::Gradient { .. } => None,
                Mesh::Solid { .. } => None,
            }),
        );
        let _ = self.gradient.vertices.write_many(
            encoder,
            belt,
            0,
            count.gradient_vertices,
            meshes.iter().filter_map(|mesh| match mesh {
                Mesh::Gradient { buffers, .. } if !mesh.is_empty() => {
                    Some(buffers.vertices.as_slice())
                }
                Mesh::Solid { .. } => None,
                Mesh::Gradient { .. } => None,
            }),
        );
        self.prepare_uniforms(encoder, belt, meshes, transformation, opacity);
    }

    /// Updates only transforms for retained geometry; vertex and index data
    /// remain untouched when the cache version has not changed.
    pub fn prepare_transforms(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        meshes: &[Mesh],
        transformation: Transformation,
        opacity: f32,
    ) {
        self.prepare_uniforms(encoder, belt, meshes, transformation, opacity);
    }

    pub fn render<'a>(
        &'a self,
        solid: &'a solid::Pipeline,
        gradient: &'a gradient::Pipeline,
        bounds: Rectangle,
        transformation: Transformation,
        pass: &mut wgpu::RenderPass<'a>,
    ) {
        let mut active_kind = None;

        for draw in &self.draws {
            let Some(clip) = bounds
                .intersection(&(draw.clip_bounds * transformation))
                .and_then(Rectangle::snap)
            else {
                continue;
            };
            pass.set_scissor_rect(clip.x, clip.y, clip.width, clip.height);

            if active_kind != Some(draw.kind) {
                match draw.kind {
                    Kind::Solid => pass.set_pipeline(&solid.raw),
                    Kind::Gradient => pass.set_pipeline(&gradient.raw),
                }
                active_kind = Some(draw.kind);
            }

            match draw.kind {
                Kind::Solid => {
                    pass.set_bind_group(
                        0,
                        &self.solid.constants,
                        &[Uniforms::dynamic_offset(draw.uniform)],
                    );
                    pass.set_vertex_buffer(
                        0,
                        self.solid
                            .vertices
                            .range(draw.vertices.start, draw.vertices.end),
                    );
                }
                Kind::Gradient => {
                    pass.set_bind_group(
                        0,
                        &self.gradient.constants,
                        &[Uniforms::dynamic_offset(draw.uniform)],
                    );
                    pass.set_vertex_buffer(
                        0,
                        self.gradient
                            .vertices
                            .range(draw.vertices.start, draw.vertices.end),
                    );
                }
            }
            pass.set_index_buffer(
                self.index_buffer
                    .range(draw.indices.start, draw.indices.end),
                wgpu::IndexFormat::Uint32,
            );
            pass.draw_indexed(0..(draw.indices.end - draw.indices.start) as u32, 0, 0..1);
        }
    }

    fn resize_uniforms(
        &mut self,
        device: &wgpu::Device,
        solid_pipeline: &solid::Pipeline,
        gradient_pipeline: &gradient::Pipeline,
        count: mesh::AttributeCount,
    ) {
        if self.solid.uniforms.resize(device, count.solids) {
            self.solid.rebind(device, &solid_pipeline.constants_layout);
        }
        if self.gradient.uniforms.resize(device, count.gradients) {
            self.gradient
                .rebind(device, &gradient_pipeline.constants_layout);
        }
    }

    fn rebuild_draws(&mut self, meshes: &[Mesh], count: mesh::AttributeCount) {
        self.draws.clear();
        self.draws.reserve(meshes.len());
        let mut index = 0;
        let mut solid_vertex = 0;
        let mut gradient_vertex = 0;
        let mut solid_uniform = 0;
        let mut gradient_uniform = 0;

        for mesh in meshes {
            if mesh.is_empty() {
                continue;
            }

            let index_end = index + mesh.indices().len();
            let draw = match mesh {
                Mesh::Solid { buffers, .. } => {
                    let vertex_end = solid_vertex + buffers.vertices.len();
                    let draw = Draw {
                        kind: Kind::Solid,
                        vertices: solid_vertex..vertex_end,
                        indices: index..index_end,
                        uniform: solid_uniform,
                        clip_bounds: mesh.clip_bounds(),
                    };
                    solid_vertex = vertex_end;
                    solid_uniform += 1;
                    draw
                }
                Mesh::Gradient { buffers, .. } => {
                    let vertex_end = gradient_vertex + buffers.vertices.len();
                    let draw = Draw {
                        kind: Kind::Gradient,
                        vertices: gradient_vertex..vertex_end,
                        indices: index..index_end,
                        uniform: gradient_uniform,
                        clip_bounds: mesh.clip_bounds(),
                    };
                    gradient_vertex = vertex_end;
                    gradient_uniform += 1;
                    draw
                }
            };
            self.draws.push(draw);
            index = index_end;
        }

        debug_assert_eq!(index, count.indices);
        debug_assert_eq!(solid_vertex, count.solid_vertices);
        debug_assert_eq!(gradient_vertex, count.gradient_vertices);
    }

    fn prepare_uniforms(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        meshes: &[Mesh],
        transformation: Transformation,
        opacity: f32,
    ) {
        self.solid_uniforms.clear();
        self.gradient_uniforms.clear();
        for mesh in meshes {
            if mesh.is_empty() {
                continue;
            }

            let transformed_clip = mesh.clip_bounds() * transformation;
            let snap = transformed_clip
                .snap()
                .map(|snapped| {
                    Point::new(snapped.x as f32, snapped.y as f32) - transformed_clip.position()
                })
                .unwrap_or(Vector::ZERO);
            let uniform = Uniforms::new(
                transformation * mesh.transformation() * Transformation::translate(snap.x, snap.y),
                opacity,
            );

            match mesh {
                Mesh::Solid { .. } => self.solid_uniforms.push(uniform),
                Mesh::Gradient { .. } => self.gradient_uniforms.push(uniform),
            }
        }

        let _ = self.solid.uniforms.write_many(
            encoder,
            belt,
            0,
            self.solid_uniforms.len(),
            std::iter::once(self.solid_uniforms.as_slice()),
        );
        let _ = self.gradient.uniforms.write_many(
            encoder,
            belt,
            0,
            self.gradient_uniforms.len(),
            std::iter::once(self.gradient_uniforms.as_slice()),
        );
    }
}

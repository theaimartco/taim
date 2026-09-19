//! Draw triangles!
use crate::color;
use crate::core::{Rectangle, Transformation};
use crate::gradient;

use bytemuck::{Pod, Zeroable};

mod cache;

pub use cache::{Cache, Id};

/// A low-level primitive to render a mesh of triangles.
#[derive(Debug, Clone, PartialEq)]
pub enum Mesh {
    /// A mesh with a solid color.
    Solid {
        /// The vertices and indices of the mesh.
        buffers: Indexed<SolidVertex2D>,

        /// The [`Transformation`] for the vertices of the [`Mesh`].
        transformation: Transformation,

        /// The clip bounds of the [`Mesh`].
        clip_bounds: Rectangle,
    },
    /// A mesh with a gradient.
    Gradient {
        /// The vertices and indices of the mesh.
        buffers: Indexed<GradientVertex2D>,

        /// The [`Transformation`] for the vertices of the [`Mesh`].
        transformation: Transformation,

        /// The clip bounds of the [`Mesh`].
        clip_bounds: Rectangle,
    },
}

impl Mesh {
    /// Returns the indices of the [`Mesh`].
    pub fn indices(&self) -> &[u32] {
        self.profile().indices
    }

    /// Returns the number of vertices owned by this mesh.
    #[inline]
    pub fn vertex_count(&self) -> usize {
        self.profile().vertices
    }

    /// Returns whether this mesh cannot produce a triangle draw.
    #[inline]
    pub fn is_empty(&self) -> bool {
        !self.profile().drawable()
    }

    /// Returns the number of complete triangles referenced by this mesh.
    #[inline]
    pub fn triangle_count(&self) -> usize {
        self.profile().indices.len() / 3
    }

    /// Returns the exact number of indices submitted by this mesh.
    #[inline]
    pub fn index_count(&self) -> usize {
        self.profile().indices.len()
    }

    /// Returns the greatest referenced vertex, or `None` for an index-free mesh.
    pub fn max_vertex_index(&self) -> Option<u32> {
        self.profile().indices.iter().copied().max()
    }

    /// Returns how many leading vertices are reachable from the index buffer.
    pub fn referenced_vertex_count(&self) -> usize {
        self.max_vertex_index()
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| index.checked_add(1))
            .unwrap_or(0)
    }

    /// Returns whether the vertex buffer contains an unreachable suffix.
    pub fn has_unused_vertices(&self) -> bool {
        self.referenced_vertex_count() < self.vertex_count()
    }

    /// Validates triangle grouping and every referenced vertex index.
    ///
    /// Render submission can use this at trust boundaries before uploading
    /// externally generated mesh data to the GPU.
    pub fn is_well_formed(&self) -> bool {
        let profile = self.profile();
        profile.indices.len().is_multiple_of(3)
            && self
                .max_vertex_index()
                .is_none_or(|index| (index as usize) < profile.vertices)
    }

    /// Returns the [`Transformation`] of the [`Mesh`].
    pub fn transformation(&self) -> Transformation {
        match self {
            Self::Solid { transformation, .. } | Self::Gradient { transformation, .. } => {
                *transformation
            }
        }
    }

    /// Returns the clip bounds of the [`Mesh`].
    pub fn clip_bounds(&self) -> Rectangle {
        match self {
            Self::Solid {
                clip_bounds,
                transformation,
                ..
            }
            | Self::Gradient {
                clip_bounds,
                transformation,
                ..
            } => *clip_bounds * *transformation,
        }
    }

    /// Multiplies every vertex alpha in this mesh.
    pub fn scale_alpha(mut self, factor: f32) -> Self {
        let factor = if factor.is_finite() {
            factor.clamp(0.0, 1.0)
        } else {
            0.0
        };

        if factor == 1.0 {
            return self;
        }

        match &mut self {
            Self::Solid { buffers, .. } => {
                for vertex in &mut buffers.vertices {
                    vertex.color = vertex.color.scale_alpha(factor);
                }
            }
            Self::Gradient { buffers, .. } => {
                for vertex in &mut buffers.vertices {
                    vertex.gradient = vertex.gradient.scale_alpha(factor);
                }
            }
        }

        self
    }

    #[inline]
    fn profile(&self) -> Profile<'_> {
        match self {
            Self::Solid { buffers, .. } => Profile {
                kind: Kind::Solid,
                vertices: buffers.vertices.len(),
                indices: &buffers.indices,
            },
            Self::Gradient { buffers, .. } => Profile {
                kind: Kind::Gradient,
                vertices: buffers.vertices.len(),
                indices: &buffers.indices,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Solid,
    Gradient,
}

#[derive(Debug, Clone, Copy)]
struct Profile<'a> {
    kind: Kind,
    vertices: usize,
    indices: &'a [u32],
}

impl Profile<'_> {
    #[inline]
    fn drawable(self) -> bool {
        self.vertices != 0 && self.indices.len() >= 3
    }
}

/// A set of vertices and indices representing a list of triangles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Indexed<T> {
    /// The vertices of the mesh
    pub vertices: Vec<T>,

    /// The list of vertex indices that defines the triangles of the mesh.
    ///
    /// Therefore, this list should always have a length that is a multiple of 3.
    pub indices: Vec<u32>,
}

/// A two-dimensional vertex with a color.
#[derive(Copy, Clone, Debug, PartialEq, Zeroable, Pod)]
#[repr(C)]
pub struct SolidVertex2D {
    /// The vertex position in 2D space.
    pub position: [f32; 2],

    /// The color of the vertex in __linear__ RGBA.
    pub color: color::Packed,
}

/// A vertex which contains 2D position & packed gradient data.
#[derive(Copy, Clone, Debug, PartialEq, Zeroable, Pod)]
#[repr(C)]
pub struct GradientVertex2D {
    /// The vertex position in 2D space.
    pub position: [f32; 2],

    /// The packed vertex data of the gradient.
    pub gradient: gradient::Packed,
}

/// The result of counting the attributes of a set of meshes.
#[derive(Debug, Clone, Copy, Default)]
pub struct AttributeCount {
    /// The total amount of solid vertices.
    pub solid_vertices: usize,

    /// The total amount of solid meshes.
    pub solids: usize,

    /// The total amount of gradient vertices.
    pub gradient_vertices: usize,

    /// The total amount of gradient meshes.
    pub gradients: usize,

    /// The total amount of indices.
    pub indices: usize,
}

/// Returns the number of total vertices & total indices of all [`Mesh`]es.
pub fn attribute_count_of(meshes: &[Mesh]) -> AttributeCount {
    let mut total = AttributeCount::default();

    for profile in meshes
        .iter()
        .map(Mesh::profile)
        .filter(|mesh| mesh.drawable())
    {
        total.indices = total.indices.saturating_add(profile.indices.len());

        match profile.kind {
            Kind::Solid => {
                total.solids = total.solids.saturating_add(1);
                total.solid_vertices = total.solid_vertices.saturating_add(profile.vertices);
            }
            Kind::Gradient => {
                total.gradients = total.gradients.saturating_add(1);
                total.gradient_vertices = total.gradient_vertices.saturating_add(profile.vertices);
            }
        }
    }

    total
}

/// A renderer capable of drawing a [`Mesh`].
pub trait Renderer {
    /// Draws the given [`Mesh`].
    fn draw_mesh(&mut self, mesh: Mesh);

    /// Draws the given [`Cache`].
    fn draw_mesh_cache(&mut self, cache: Cache);
}

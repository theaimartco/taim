use crate::core::{Rectangle, Transformation};
use crate::graphics::color;
use crate::graphics::geometry::fill;
use crate::graphics::geometry::{LineCap, LineJoin, Path, Style};
use crate::graphics::gradient;
use crate::graphics::mesh::{self, Mesh};

use lyon::tessellation;

enum Buffer {
    Solid(tessellation::VertexBuffers<mesh::SolidVertex2D, u32>),
    Gradient(tessellation::VertexBuffers<mesh::GradientVertex2D, u32>),
}

pub(super) struct Stack {
    buffers: Vec<Buffer>,
}

impl Stack {
    pub fn new() -> Self {
        Self {
            buffers: Vec::with_capacity(4),
        }
    }

    pub fn fill_path(
        &mut self,
        tessellator: &mut tessellation::FillTessellator,
        path: &Path,
        options: &tessellation::FillOptions,
        style: &Style,
    ) -> Result<(), tessellation::TessellationError> {
        match (style, self.for_style(style)) {
            (Style::Solid(color), Buffer::Solid(buffer)) => tessellator.tessellate_path(
                path.raw(),
                options,
                &mut tessellation::BuffersBuilder::new(buffer, SolidBuilder(color::pack(*color))),
            ),
            (Style::Gradient(gradient), Buffer::Gradient(buffer)) => tessellator.tessellate_path(
                path.raw(),
                options,
                &mut tessellation::BuffersBuilder::new(
                    buffer,
                    GradientBuilder {
                        gradient: gradient.pack(),
                    },
                ),
            ),
            _ => unreachable!("style selects matching geometry buffer"),
        }
    }

    pub fn fill_rectangle(
        &mut self,
        tessellator: &mut tessellation::FillTessellator,
        rectangle: &lyon::math::Box2D,
        options: &tessellation::FillOptions,
        style: &Style,
    ) -> Result<(), tessellation::TessellationError> {
        match (style, self.for_style(style)) {
            (Style::Solid(color), Buffer::Solid(buffer)) => tessellator.tessellate_rectangle(
                rectangle,
                options,
                &mut tessellation::BuffersBuilder::new(buffer, SolidBuilder(color::pack(*color))),
            ),
            (Style::Gradient(gradient), Buffer::Gradient(buffer)) => tessellator
                .tessellate_rectangle(
                    rectangle,
                    options,
                    &mut tessellation::BuffersBuilder::new(
                        buffer,
                        GradientBuilder {
                            gradient: gradient.pack(),
                        },
                    ),
                ),
            _ => unreachable!("style selects matching geometry buffer"),
        }
    }

    pub fn stroke_path(
        &mut self,
        tessellator: &mut tessellation::StrokeTessellator,
        path: &Path,
        options: &tessellation::StrokeOptions,
        style: &Style,
    ) -> Result<(), tessellation::TessellationError> {
        match (style, self.for_style(style)) {
            (Style::Solid(color), Buffer::Solid(buffer)) => tessellator.tessellate_path(
                path.raw(),
                options,
                &mut tessellation::BuffersBuilder::new(buffer, SolidBuilder(color::pack(*color))),
            ),
            (Style::Gradient(gradient), Buffer::Gradient(buffer)) => tessellator.tessellate_path(
                path.raw(),
                options,
                &mut tessellation::BuffersBuilder::new(
                    buffer,
                    GradientBuilder {
                        gradient: gradient.pack(),
                    },
                ),
            ),
            _ => unreachable!("style selects matching geometry buffer"),
        }
    }

    pub fn stroke_rectangle(
        &mut self,
        tessellator: &mut tessellation::StrokeTessellator,
        rectangle: &lyon::math::Box2D,
        options: &tessellation::StrokeOptions,
        style: &Style,
    ) -> Result<(), tessellation::TessellationError> {
        match (style, self.for_style(style)) {
            (Style::Solid(color), Buffer::Solid(buffer)) => tessellator.tessellate_rectangle(
                rectangle,
                options,
                &mut tessellation::BuffersBuilder::new(buffer, SolidBuilder(color::pack(*color))),
            ),
            (Style::Gradient(gradient), Buffer::Gradient(buffer)) => tessellator
                .tessellate_rectangle(
                    rectangle,
                    options,
                    &mut tessellation::BuffersBuilder::new(
                        buffer,
                        GradientBuilder {
                            gradient: gradient.pack(),
                        },
                    ),
                ),
            _ => unreachable!("style selects matching geometry buffer"),
        }
    }

    pub fn into_meshes(self, clip_bounds: Rectangle) -> impl Iterator<Item = Mesh> {
        self.buffers.into_iter().filter_map(move |buffer| {
            let (solid, gradient) = match buffer {
                Buffer::Solid(buffer) if !buffer.indices.is_empty() => (Some(buffer), None),
                Buffer::Gradient(buffer) if !buffer.indices.is_empty() => (None, Some(buffer)),
                _ => return None,
            };

            Some(if let Some(buffer) = solid {
                Mesh::Solid {
                    buffers: mesh::Indexed {
                        vertices: buffer.vertices,
                        indices: buffer.indices,
                    },
                    clip_bounds,
                    transformation: Transformation::IDENTITY,
                }
            } else {
                let buffer = gradient.expect("gradient buffer selected");
                Mesh::Gradient {
                    buffers: mesh::Indexed {
                        vertices: buffer.vertices,
                        indices: buffer.indices,
                    },
                    clip_bounds,
                    transformation: Transformation::IDENTITY,
                }
            })
        })
    }

    fn for_style(&mut self, style: &Style) -> &mut Buffer {
        let matching = matches!(
            (style, self.buffers.last()),
            (Style::Solid(_), Some(Buffer::Solid(_)))
                | (Style::Gradient(_), Some(Buffer::Gradient(_)))
        );
        if !matching {
            self.buffers.push(match style {
                Style::Solid(_) => Buffer::Solid(tessellation::VertexBuffers::new()),
                Style::Gradient(_) => Buffer::Gradient(tessellation::VertexBuffers::new()),
            });
        }
        self.buffers.last_mut().expect("geometry buffer inserted")
    }
}

pub(super) fn fill_options(rule: fill::Rule) -> tessellation::FillOptions {
    tessellation::FillOptions::default().with_fill_rule(match rule {
        fill::Rule::NonZero => tessellation::FillRule::NonZero,
        fill::Rule::EvenOdd => tessellation::FillRule::EvenOdd,
    })
}

pub(super) fn stroke_options(
    width: f32,
    cap: LineCap,
    join: LineJoin,
) -> tessellation::StrokeOptions {
    tessellation::StrokeOptions::default()
        .with_line_width(width)
        .with_start_cap(line_cap(cap))
        .with_end_cap(line_cap(cap))
        .with_line_join(match join {
            LineJoin::Miter => tessellation::LineJoin::Miter,
            LineJoin::Round => tessellation::LineJoin::Round,
            LineJoin::Bevel => tessellation::LineJoin::Bevel,
        })
}

fn line_cap(cap: LineCap) -> tessellation::LineCap {
    match cap {
        LineCap::Butt => tessellation::LineCap::Butt,
        LineCap::Square => tessellation::LineCap::Square,
        LineCap::Round => tessellation::LineCap::Round,
    }
}

struct SolidBuilder(color::Packed);

impl tessellation::FillVertexConstructor<mesh::SolidVertex2D> for SolidBuilder {
    fn new_vertex(&mut self, vertex: tessellation::FillVertex<'_>) -> mesh::SolidVertex2D {
        let position = vertex.position();
        mesh::SolidVertex2D {
            position: [position.x, position.y],
            color: self.0,
        }
    }
}

impl tessellation::StrokeVertexConstructor<mesh::SolidVertex2D> for SolidBuilder {
    fn new_vertex(&mut self, vertex: tessellation::StrokeVertex<'_, '_>) -> mesh::SolidVertex2D {
        let position = vertex.position();
        mesh::SolidVertex2D {
            position: [position.x, position.y],
            color: self.0,
        }
    }
}

struct GradientBuilder {
    gradient: gradient::Packed,
}

impl tessellation::FillVertexConstructor<mesh::GradientVertex2D> for GradientBuilder {
    fn new_vertex(&mut self, vertex: tessellation::FillVertex<'_>) -> mesh::GradientVertex2D {
        let position = vertex.position();
        mesh::GradientVertex2D {
            position: [position.x, position.y],
            gradient: self.gradient,
        }
    }
}

impl tessellation::StrokeVertexConstructor<mesh::GradientVertex2D> for GradientBuilder {
    fn new_vertex(&mut self, vertex: tessellation::StrokeVertex<'_, '_>) -> mesh::GradientVertex2D {
        let position = vertex.position();
        mesh::GradientVertex2D {
            position: [position.x, position.y],
            gradient: self.gradient,
        }
    }
}

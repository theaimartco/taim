use super::Layer;
use crate::core::{self, Background, Color, Point, Rectangle, Svg, Transformation, renderer};
use crate::graphics;
use crate::graphics::Mesh;
use crate::graphics::color;
use crate::graphics::mesh;
use crate::graphics::text::{Editor, Paragraph};
use crate::image::Image;
use crate::primitive::{self, Primitive};
use crate::quad::Quad;
use crate::text::{self, Text};
use crate::triangle;

impl Layer {
    pub fn is_empty(&self) -> bool {
        self.quads.is_empty()
            && self.triangles.is_empty()
            && self.primitives.is_empty()
            && self.images.is_empty()
            && self.text.is_empty()
            && self.effects.is_empty()
            && self.pending_meshes.is_empty()
            && self.pending_text.is_empty()
    }

    pub fn draw_quad(
        &mut self,
        quad: renderer::Quad,
        background: Background,
        transformation: Transformation,
    ) {
        let bounds = quad.bounds * transformation;
        let scale = transformation.scale_factors();
        let uniform_scale = scale.x.min(scale.y);
        let luminance = quad.luminance;
        let material = if color::GAMMA_CORRECTION {
            quad.material.sanitised()
        } else {
            core::material::Material::OPAQUE
        };

        let quad_data = Quad {
            position: [bounds.x, bounds.y],
            size: [bounds.width, bounds.height],
            border_color: color::pack(quad.border.color).scale_light(if color::GAMMA_CORRECTION {
                quad.luminance
            } else {
                1.0
            }),
            border_radius: (quad.border.radius * uniform_scale).into(),
            border_width: quad.border.width * uniform_scale,
            shadow_color: color::pack(quad.shadow.color),
            shadow_offset: (quad.shadow.offset * uniform_scale).into(),
            shadow_blur_radius: quad.shadow.blur_radius * uniform_scale,
            snap: u32::from(quad.snap),
            material: [
                material.roughness,
                material.thickness_mm,
                material.ior,
                material.dispersion,
            ],
            transmission: material.transmission,
        };

        // Liquid glass needs linear light like any material, and a flat tint.
        if let (Some(liquid), Background::Color(tint), true) =
            (quad.liquid, &background, color::GAMMA_CORRECTION)
        {
            self.quads.add_liquid(crate::quad::Liquid::new(
                color::pack(*tint).scale_light(luminance),
                quad_data,
                &liquid,
                transformation,
                uniform_scale,
            ));
            return;
        }

        self.quads.add(quad_data, &background, luminance);
    }

    pub fn draw_paragraph(
        &mut self,
        paragraph: &Paragraph,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        self.pending_text.push(Text::Paragraph {
            paragraph: paragraph.downgrade(),
            position,
            color,
            clip_bounds,
            transformation,
        });
    }

    pub fn draw_editor(
        &mut self,
        editor: &Editor,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        self.pending_text.push(Text::Editor {
            editor: editor.downgrade(),
            position,
            color,
            clip_bounds,
            transformation,
        });
    }

    pub fn draw_text(
        &mut self,
        text: crate::core::Text,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        let scale = transformation.scale_factor();
        self.pending_text.push(Text::Cached {
            content: text.content,
            bounds: Rectangle::new(position, text.bounds) * transformation,
            color,
            size: text.size * scale,
            line_height: text.line_height.to_absolute(text.size) * scale,
            font: text.font,
            align_x: text.align_x,
            align_y: text.align_y,
            shaping: text.shaping,
            wrapping: text.wrapping,
            ellipsis: text.ellipsis,
            clip_bounds: clip_bounds * transformation,
        });
    }

    pub fn draw_text_raw(&mut self, raw: graphics::text::Raw, transformation: Transformation) {
        self.pending_text.push(Text::Raw {
            raw,
            transformation,
        });
    }

    pub fn draw_image(&mut self, image: Image, transformation: Transformation) {
        match image {
            Image::Raster {
                image,
                bounds,
                clip_bounds,
            } => self.draw_raster(image, bounds, clip_bounds, transformation),
            Image::Vector {
                svg,
                bounds,
                clip_bounds,
            } => self.draw_svg(svg, bounds, clip_bounds, transformation),
        }
    }

    pub fn draw_raster(
        &mut self,
        image: core::Image,
        bounds: Rectangle,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        let scale = transformation.scale_factors();
        self.images.push(Image::Raster {
            image: core::Image {
                border_radius: image.border_radius * scale.x.min(scale.y),
                ..image
            },
            bounds: bounds * transformation,
            clip_bounds: clip_bounds * transformation,
        });
    }

    pub fn draw_svg(
        &mut self,
        svg: Svg,
        bounds: Rectangle,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        self.images.push(Image::Vector {
            svg,
            bounds: bounds * transformation,
            clip_bounds: clip_bounds * transformation,
        });
    }

    pub fn draw_mesh(&mut self, mut mesh: Mesh, transformation: Transformation) {
        if !mesh_is_visible(&mesh, transformation, self.bounds) {
            return;
        }

        match &mut mesh {
            Mesh::Solid {
                transformation: local,
                ..
            }
            | Mesh::Gradient {
                transformation: local,
                ..
            } => *local = *local * transformation,
        }

        self.pending_meshes.push(mesh);
    }

    pub fn draw_mesh_group(&mut self, mut meshes: Vec<Mesh>, transformation: Transformation) {
        meshes.retain(|mesh| mesh_is_visible(mesh, transformation, self.bounds));
        if meshes.is_empty() {
            return;
        }

        self.flush_meshes();
        self.triangles.push(triangle::Item::Group {
            meshes,
            transformation,
        });
    }

    pub fn draw_mesh_cache(
        &mut self,
        cache: mesh::Cache,
        transformation: Transformation,
        opacity: f32,
    ) {
        if cache.is_empty()
            || !opacity.is_finite()
            || opacity <= 0.0
            || !cache
                .batch()
                .iter()
                .any(|mesh| mesh_is_visible(mesh, transformation, self.bounds))
        {
            return;
        }

        self.flush_meshes();
        self.triangles.push(triangle::Item::Cached {
            cache,
            transformation,
            opacity: opacity.min(1.0),
        });
    }

    pub fn draw_text_group(&mut self, text: Vec<Text>, transformation: Transformation) {
        self.flush_text();
        self.text.push(text::Item::Group {
            text,
            transformation,
        });
    }

    pub fn draw_text_cache(&mut self, cache: text::Cache, transformation: Transformation) {
        self.flush_text();
        self.text.push(text::Item::Cached {
            cache,
            transformation,
        });
    }

    pub fn draw_primitive(
        &mut self,
        bounds: Rectangle,
        primitive: impl Primitive,
        transformation: Transformation,
    ) {
        let bounds = bounds * transformation;

        if bounds.width <= 0.0 || bounds.height <= 0.0 || !bounds.intersects(&self.bounds) {
            return;
        }

        self.primitives
            .push(primitive::Instance::new(bounds, primitive));
    }

    pub(super) fn flush_meshes(&mut self) {
        if self.pending_meshes.is_empty() {
            return;
        }

        let replacement = self.mesh_pool.pop().unwrap_or_default();
        let meshes = std::mem::replace(&mut self.pending_meshes, replacement);
        self.triangles.push(triangle::Item::Group {
            meshes,
            transformation: Transformation::IDENTITY,
        });
    }

    pub(super) fn flush_text(&mut self) {
        if self.pending_text.is_empty() {
            return;
        }

        let replacement = self.text_pool.pop().unwrap_or_default();
        let text = std::mem::replace(&mut self.pending_text, replacement);
        self.text.push(text::Item::Group {
            text,
            transformation: Transformation::IDENTITY,
        });
    }
}

#[inline]
fn mesh_is_visible(mesh: &Mesh, transformation: Transformation, viewport: Rectangle) -> bool {
    let bounds = mesh.clip_bounds() * transformation;

    bounds.width > 0.0
        && bounds.height > 0.0
        && bounds.x.is_finite()
        && bounds.y.is_finite()
        && bounds.intersects(&viewport)
}

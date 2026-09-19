use super::{Engine, into_color, path};
use crate::core::renderer::Quad;
use crate::core::{Background, Gradient, Point, Rectangle, Size, Transformation};

impl Engine {
    #[allow(clippy::too_many_arguments)]
    pub fn draw_quad(
        &mut self,
        quad: &Quad,
        background: &Background,
        transformation: Transformation,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: &mut tiny_skia::Mask,
        clip_bounds: Rectangle,
    ) {
        let physical_bounds = quad.bounds * transformation;
        let shadow_visible = if quad.shadow.color.a > 0.0 {
            let shadow_bounds = Rectangle::new(
                Point::new(
                    quad.bounds.x + quad.shadow.offset.x - quad.shadow.blur_radius,
                    quad.bounds.y + quad.shadow.offset.y - quad.shadow.blur_radius,
                ),
                Size::new(
                    quad.bounds.width + quad.shadow.blur_radius * 2.0,
                    quad.bounds.height + quad.shadow.blur_radius * 2.0,
                ),
            ) * transformation;
            clip_bounds.intersects(&shadow_bounds)
        } else {
            false
        };
        if !shadow_visible && !clip_bounds.intersects(&physical_bounds) {
            return;
        }

        let half_width = quad.bounds.width * 0.5;
        let half_height = quad.bounds.height * 0.5;
        let border_width = quad.border.width.max(0.0).min(half_width).min(half_height);
        let radii = <[f32; 4]>::from(quad.border.radius)
            .map(|radius| radius.max(0.0).min(half_width).min(half_height));
        let Some(shape) = path::rounded_rectangle(quad.bounds, radii) else {
            return;
        };

        self.shadow_cache.draw(
            quad.bounds,
            quad.shadow,
            radii,
            transformation,
            pixels,
            clip_mask,
        );

        if !clip_bounds.intersects(&physical_bounds) {
            return;
        }
        let mask = (!physical_bounds.is_within(&clip_bounds)).then_some(clip_mask as &_);
        let paint = tiny_skia::Paint {
            shader: shader(background, quad.bounds),
            anti_alias: true,
            ..Default::default()
        };
        let transform = path::transform(transformation);
        pixels.fill_path(
            &shape,
            &paint,
            tiny_skia::FillRule::EvenOdd,
            transform,
            mask,
        );

        if border_width > 0.0 && quad.border.color.a > 0.0 {
            let ring = path::border_ring(&shape, quad.bounds, radii, border_width);
            pixels.fill_path(
                &ring,
                &tiny_skia::Paint {
                    shader: tiny_skia::Shader::SolidColor(into_color(quad.border.color)),
                    anti_alias: true,
                    ..Default::default()
                },
                tiny_skia::FillRule::EvenOdd,
                transform,
                mask,
            );
        }
    }
}

fn shader(background: &Background, bounds: Rectangle) -> tiny_skia::Shader<'static> {
    match background {
        Background::Color(color) => tiny_skia::Shader::SolidColor(into_color(*color)),
        Background::Gradient(Gradient::Linear(gradient)) => {
            let (start, end) = bounds.chord(gradient.angle);
            let mut stops = gradient
                .stops
                .into_iter()
                .flatten()
                .map(|stop| tiny_skia::GradientStop::new(stop.offset, into_color(stop.color)))
                .collect::<Vec<_>>();
            if stops.is_empty() {
                stops.push(tiny_skia::GradientStop::new(0.0, tiny_skia::Color::BLACK));
            }
            tiny_skia::LinearGradient::new(
                tiny_skia::Point {
                    x: start.x,
                    y: start.y,
                },
                tiny_skia::Point { x: end.x, y: end.y },
                stops,
                tiny_skia::SpreadMode::Pad,
                tiny_skia::Transform::identity(),
            )
            .unwrap_or(tiny_skia::Shader::SolidColor(tiny_skia::Color::BLACK))
        }
    }
}

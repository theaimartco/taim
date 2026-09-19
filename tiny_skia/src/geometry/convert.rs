use crate::graphics::Gradient;
use crate::graphics::geometry::fill;
use crate::graphics::geometry::stroke::{self, Stroke};
use crate::graphics::geometry::{Path, Style};

pub(super) fn path(path: &Path) -> Option<tiny_skia::Path> {
    use taim_graphics::geometry::path::lyon_path;

    let mut builder = tiny_skia::PathBuilder::new();
    let mut cursor = None;

    for event in path.raw() {
        match event {
            lyon_path::Event::Begin { at } => {
                builder.move_to(at.x, at.y);
                cursor = Some(at);
            }
            lyon_path::Event::Line { from, to } => {
                move_if_needed(&mut builder, &mut cursor, from);
                builder.line_to(to.x, to.y);
                cursor = Some(to);
            }
            lyon_path::Event::Quadratic { from, ctrl, to } => {
                move_if_needed(&mut builder, &mut cursor, from);
                builder.quad_to(ctrl.x, ctrl.y, to.x, to.y);
                cursor = Some(to);
            }
            lyon_path::Event::Cubic {
                from,
                ctrl1,
                ctrl2,
                to,
            } => {
                move_if_needed(&mut builder, &mut cursor, from);
                builder.cubic_to(ctrl1.x, ctrl1.y, ctrl2.x, ctrl2.y, to.x, to.y);
                cursor = Some(to);
            }
            lyon_path::Event::End { close, last, .. } => {
                if close {
                    builder.close();
                }
                cursor = Some(last);
            }
        }
    }

    let converted = builder.finish();
    if converted.is_none() {
        log::warn!("TAIM software geometry rejected an invalid path");
    }
    converted
}

fn move_if_needed(
    builder: &mut tiny_skia::PathBuilder,
    cursor: &mut Option<taim_graphics::geometry::path::lyon_path::math::Point>,
    from: taim_graphics::geometry::path::lyon_path::math::Point,
) {
    if cursor.is_none_or(|cursor| cursor != from) {
        builder.move_to(from.x, from.y);
        *cursor = Some(from);
    }
}

pub(super) fn paint(style: Style) -> tiny_skia::Paint<'static> {
    let shader = match style {
        Style::Solid(color) => tiny_skia::Shader::SolidColor(crate::engine::into_color(color)),
        Style::Gradient(Gradient::Linear(linear)) => {
            let mut stops: Vec<_> = linear
                .stops
                .into_iter()
                .flatten()
                .map(|stop| {
                    tiny_skia::GradientStop::new(stop.offset, crate::engine::into_color(stop.color))
                })
                .collect();
            if stops.is_empty() {
                stops.push(tiny_skia::GradientStop::new(0.0, tiny_skia::Color::BLACK));
            }

            tiny_skia::LinearGradient::new(
                tiny_skia::Point {
                    x: linear.start.x,
                    y: linear.start.y,
                },
                tiny_skia::Point {
                    x: linear.end.x,
                    y: linear.end.y,
                },
                stops,
                tiny_skia::SpreadMode::Pad,
                tiny_skia::Transform::identity(),
            )
            .unwrap_or(tiny_skia::Shader::SolidColor(tiny_skia::Color::BLACK))
        }
    };

    tiny_skia::Paint {
        shader,
        anti_alias: true,
        ..Default::default()
    }
}

pub(super) fn fill_rule(rule: fill::Rule) -> tiny_skia::FillRule {
    match rule {
        fill::Rule::EvenOdd => tiny_skia::FillRule::EvenOdd,
        fill::Rule::NonZero => tiny_skia::FillRule::Winding,
    }
}

pub(super) fn stroke(stroke: &Stroke<'_>) -> tiny_skia::Stroke {
    tiny_skia::Stroke {
        width: stroke.width,
        line_cap: match stroke.line_cap {
            stroke::LineCap::Butt => tiny_skia::LineCap::Butt,
            stroke::LineCap::Square => tiny_skia::LineCap::Square,
            stroke::LineCap::Round => tiny_skia::LineCap::Round,
        },
        line_join: match stroke.line_join {
            stroke::LineJoin::Miter => tiny_skia::LineJoin::Miter,
            stroke::LineJoin::Round => tiny_skia::LineJoin::Round,
            stroke::LineJoin::Bevel => tiny_skia::LineJoin::Bevel,
        },
        dash: if stroke.line_dash.segments.is_empty() {
            None
        } else {
            tiny_skia::StrokeDash::new(
                stroke.line_dash.segments.into(),
                stroke.line_dash.offset as f32,
            )
        },
        ..Default::default()
    }
}

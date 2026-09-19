use crate::core::{Rectangle, Transformation};

pub(super) fn transform(transformation: Transformation) -> tiny_skia::Transform {
    let matrix: [f32; 16] = transformation.into();
    tiny_skia::Transform {
        sx: matrix[0],
        kx: matrix[4],
        ky: matrix[1],
        sy: matrix[5],
        tx: matrix[12],
        ty: matrix[13],
    }
}

pub(super) fn rounded_rectangle(bounds: Rectangle, radii: [f32; 4]) -> Option<tiny_skia::Path> {
    let [top_left, top_right, bottom_right, bottom_left] = radii;
    let rectangle = tiny_skia::Rect::from_xywh(bounds.x, bounds.y, bounds.width, bounds.height)?;
    if radii.iter().all(|radius| *radius == 0.0) {
        return Some(tiny_skia::PathBuilder::from_rect(rectangle));
    }
    if radii.iter().all(|radius| *radius == top_left)
        && top_left == bounds.width * 0.5
        && top_left == bounds.height * 0.5
    {
        return tiny_skia::PathBuilder::from_circle(
            bounds.x + bounds.width * 0.5,
            bounds.y + bounds.height * 0.5,
            top_left,
        );
    }

    let mut path = tiny_skia::PathBuilder::new();
    path.move_to(bounds.x + top_left, bounds.y);
    path.line_to(bounds.x + bounds.width - top_right, bounds.y);
    corner(
        &mut path,
        bounds.x + bounds.width - top_right,
        bounds.y,
        bounds.x + bounds.width,
        bounds.y + top_right,
        top_right,
    );
    line_if_needed(
        &mut path,
        bounds.x + bounds.width,
        bounds.y + bounds.height - bottom_right,
    );
    corner(
        &mut path,
        bounds.x + bounds.width,
        bounds.y + bounds.height - bottom_right,
        bounds.x + bounds.width - bottom_right,
        bounds.y + bounds.height,
        bottom_right,
    );
    line_if_needed(&mut path, bounds.x + bottom_left, bounds.y + bounds.height);
    corner(
        &mut path,
        bounds.x + bottom_left,
        bounds.y + bounds.height,
        bounds.x,
        bounds.y + bounds.height - bottom_left,
        bottom_left,
    );
    line_if_needed(&mut path, bounds.x, bounds.y + top_left);
    corner(
        &mut path,
        bounds.x,
        bounds.y + top_left,
        bounds.x + top_left,
        bounds.y,
        top_left,
    );
    path.finish()
}

pub(super) fn border_ring(
    outer: &tiny_skia::Path,
    bounds: Rectangle,
    outer_radii: [f32; 4],
    width: f32,
) -> tiny_skia::Path {
    let mut builder = tiny_skia::PathBuilder::new();
    builder.push_path(outer);
    let inner = Rectangle {
        x: bounds.x + width,
        y: bounds.y + width,
        width: bounds.width - width * 2.0,
        height: bounds.height - width * 2.0,
    };
    if inner.width > 0.0 && inner.height > 0.0 {
        let radii = outer_radii.map(|radius| {
            (radius - width)
                .max(0.0)
                .min(inner.width * 0.5)
                .min(inner.height * 0.5)
        });
        if let Some(inner) = rounded_rectangle(inner, radii) {
            builder.push_path(&inner);
        }
    }
    builder.finish().unwrap_or_else(|| outer.clone())
}

fn line_if_needed(path: &mut tiny_skia::PathBuilder, x: f32, y: f32) {
    if path.last_point() != Some(tiny_skia::Point { x, y }) {
        path.line_to(x, y);
    }
}

fn corner(
    path: &mut tiny_skia::PathBuilder,
    x_from: f32,
    y_from: f32,
    x_to: f32,
    y_to: f32,
    radius: f32,
) {
    const KAPPA: f32 = 0.552_284_8;
    if radius <= 0.0 || !radius.is_finite() {
        path.line_to(x_to, y_to);
        return;
    }

    let control = radius * KAPPA;
    let (first_x, first_y, second_x, second_y) = match (x_to >= x_from, y_to >= y_from) {
        (true, true) => (x_from + control, y_from, x_to, y_to - control),
        (false, true) => (x_from, y_from + control, x_to + control, y_to),
        (false, false) => (x_from - control, y_from, x_to, y_to + control),
        (true, false) => (x_from, y_from - control, x_to - control, y_to),
    };
    path.cubic_to(first_x, first_y, second_x, second_y, x_to, y_to);
}

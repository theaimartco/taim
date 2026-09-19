use super::{DrawBatch, Layer, Quad};
use crate::Primitive;
use crate::core::{Point, Rectangle, Size};
use crate::graphics::Image;
use crate::graphics::text::Text;

pub(super) fn between(previous: &Layer, current: &Layer) -> Vec<Rectangle> {
    if previous.bounds != current.bounds {
        return vec![previous.bounds, current.bounds];
    }

    let layer_bounds = current.bounds.expand(1.0);
    let mut output = Vec::with_capacity(16);

    compare_values(&previous.quads, &current.quads, &mut output, |quad| {
        quad_bounds(quad, layer_bounds)
    });
    compare_batches(
        &previous.primitives,
        &current.primitives,
        &mut output,
        layer_bounds,
        primitive_bounds,
    );
    compare_batches(
        &previous.images,
        &current.images,
        &mut output,
        layer_bounds,
        image_bounds,
    );
    compare_batches(
        &previous.text,
        &current.text,
        &mut output,
        layer_bounds,
        text_bounds,
    );

    output
}

fn compare_values<T: PartialEq>(
    previous: &[T],
    current: &[T],
    output: &mut Vec<Rectangle>,
    bounds: impl Fn(&T) -> Option<Rectangle>,
) {
    let common = previous.len().min(current.len());
    for index in 0..common {
        if previous[index] != current[index] {
            push(output, bounds(&previous[index]));
            push(output, bounds(&current[index]));
        }
    }
    for value in &previous[common..] {
        push(output, bounds(value));
    }
    for value in &current[common..] {
        push(output, bounds(value));
    }
}

fn compare_batches<T: PartialEq>(
    previous: &[DrawBatch<T>],
    current: &[DrawBatch<T>],
    output: &mut Vec<Rectangle>,
    layer_bounds: Rectangle,
    visible: fn(&T) -> Option<Rectangle>,
) {
    let common = previous.len().min(current.len());
    for index in 0..common {
        let old = &previous[index];
        let new = &current[index];
        if old.unchanged(new) {
            continue;
        }

        if old.clip_bounds() == new.clip_bounds() && old.transformation() == new.transformation() {
            compare_batch_values(old, new, output, layer_bounds, visible);
        } else {
            push_batch(output, old, layer_bounds, visible);
            push_batch(output, new, layer_bounds, visible);
        }
    }
    for batch in &previous[common..] {
        push_batch(output, batch, layer_bounds, visible);
    }
    for batch in &current[common..] {
        push_batch(output, batch, layer_bounds, visible);
    }
}

fn compare_batch_values<T: PartialEq>(
    previous: &DrawBatch<T>,
    current: &DrawBatch<T>,
    output: &mut Vec<Rectangle>,
    layer_bounds: Rectangle,
    visible: fn(&T) -> Option<Rectangle>,
) {
    let old = previous.as_slice();
    let new = current.as_slice();
    let common = old.len().min(new.len());
    for index in 0..common {
        if old[index] != new[index] {
            push(
                output,
                batch_bounds(previous, &old[index], layer_bounds, visible),
            );
            push(
                output,
                batch_bounds(current, &new[index], layer_bounds, visible),
            );
        }
    }
    for value in &old[common..] {
        push(output, batch_bounds(previous, value, layer_bounds, visible));
    }
    for value in &new[common..] {
        push(output, batch_bounds(current, value, layer_bounds, visible));
    }
}

fn push_batch<T>(
    output: &mut Vec<Rectangle>,
    batch: &DrawBatch<T>,
    layer_bounds: Rectangle,
    visible: fn(&T) -> Option<Rectangle>,
) {
    for value in batch.as_slice() {
        push(output, batch_bounds(batch, value, layer_bounds, visible));
    }
}

fn batch_bounds<T>(
    batch: &DrawBatch<T>,
    value: &T,
    layer_bounds: Rectangle,
    visible: fn(&T) -> Option<Rectangle>,
) -> Option<Rectangle> {
    visible(value)
        .map(|bounds| bounds * batch.transformation())
        .and_then(|bounds| bounds.intersection(&batch.clip_bounds()))
        .and_then(|bounds| bounds.intersection(&layer_bounds))
}

fn quad_bounds(quad: &Quad, layer_bounds: Rectangle) -> Option<Rectangle> {
    let mut bounds = quad.primitive.bounds * quad.transformation;
    let shadow = quad.primitive.shadow;
    if shadow.color.a > 0.0 {
        let local_shadow = Rectangle::new(
            Point::new(
                quad.primitive.bounds.x + shadow.offset.x - shadow.blur_radius,
                quad.primitive.bounds.y + shadow.offset.y - shadow.blur_radius,
            ),
            Size::new(
                quad.primitive.bounds.width + shadow.blur_radius * 2.0,
                quad.primitive.bounds.height + shadow.blur_radius * 2.0,
            ),
        ) * quad.transformation;
        bounds = bounds.union(&local_shadow);
    }
    bounds.expand(1.0).intersection(&layer_bounds)
}

fn primitive_bounds(primitive: &Primitive) -> Option<Rectangle> {
    Some(primitive.visible_bounds())
}

fn image_bounds(image: &Image) -> Option<Rectangle> {
    Some(image.bounds().expand(1.0))
}

fn text_bounds(text: &Text) -> Option<Rectangle> {
    text.visible_bounds()
}

fn push(output: &mut Vec<Rectangle>, bounds: Option<Rectangle>) {
    if let Some(bounds) = bounds.filter(|bounds| bounds.width > 0.0 && bounds.height > 0.0) {
        output.push(bounds);
    }
}

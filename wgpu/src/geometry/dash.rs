use crate::core::Point;
use crate::graphics::geometry::{LineDash, Path};

use lyon::algorithms::walk::{RepeatedPattern, WalkerEvent, walk_along_path};
use lyon::path::iterator::PathIterator;
use smallvec::SmallVec;

pub(super) fn apply(path: &Path, dash: LineDash<'_>) -> Path {
    let mut doubled = SmallVec::<[f32; 16]>::new();
    let intervals = if dash.segments.len() % 2 == 1 {
        doubled.extend_from_slice(dash.segments);
        doubled.extend_from_slice(dash.segments);
        doubled.as_slice()
    } else {
        dash.segments
    };

    Path::new(|builder| {
        let mut draw = false;
        walk_along_path(
            path.raw()
                .iter()
                .flattened(lyon::tessellation::StrokeOptions::DEFAULT_TOLERANCE),
            0.0,
            lyon::tessellation::StrokeOptions::DEFAULT_TOLERANCE,
            &mut RepeatedPattern {
                callback: |event: WalkerEvent<'_>| {
                    let point = Point::new(event.position.x, event.position.y);
                    if draw {
                        builder.line_to(point);
                    } else {
                        builder.move_to(point);
                    }
                    draw = !draw;
                    true
                },
                index: dash.offset,
                intervals,
            },
        );
    })
}

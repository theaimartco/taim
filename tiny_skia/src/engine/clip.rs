use crate::core::Rectangle;

#[derive(Debug, Clone, Copy)]
struct Span {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
}

#[derive(Debug)]
pub(super) struct Tracker {
    previous: Option<(Rectangle, Span)>,
    fresh_frame: bool,
}

impl Tracker {
    pub fn new() -> Self {
        Self {
            previous: None,
            fresh_frame: true,
        }
    }

    pub fn begin_frame(&mut self) {
        self.fresh_frame = true;
    }

    pub fn set(&mut self, mask: &mut tiny_skia::Mask, bounds: Rectangle) {
        if !self.fresh_frame
            && self
                .previous
                .is_some_and(|(previous, _)| previous == bounds)
        {
            return;
        }

        if self.fresh_frame {
            mask.clear();
            self.fresh_frame = false;
        } else if let Some((_, span)) = self.previous {
            clear_span(mask, span);
        }

        let Some(rectangle) =
            tiny_skia::Rect::from_xywh(bounds.x, bounds.y, bounds.width, bounds.height)
        else {
            self.previous = None;
            return;
        };
        let span = span(bounds, mask.width(), mask.height());
        let path = tiny_skia::PathBuilder::from_rect(rectangle);
        mask.fill_path(
            &path,
            tiny_skia::FillRule::EvenOdd,
            false,
            tiny_skia::Transform::identity(),
        );
        self.previous = Some((bounds, span));
    }
}

fn span(bounds: Rectangle, width: u32, height: u32) -> Span {
    Span {
        left: (bounds.x.floor().max(0.0) as u32).min(width),
        top: (bounds.y.floor().max(0.0) as u32).min(height),
        right: ((bounds.x + bounds.width).ceil().max(0.0) as u32).min(width),
        bottom: ((bounds.y + bounds.height).ceil().max(0.0) as u32).min(height),
    }
}

fn clear_span(mask: &mut tiny_skia::Mask, span: Span) {
    let row_width = mask.width() as usize;
    let left = span.left as usize;
    let right = span.right as usize;
    if left >= right {
        return;
    }
    for row in span.top as usize..span.bottom as usize {
        mask.data_mut()[row * row_width + left..row * row_width + right].fill(0);
    }
}

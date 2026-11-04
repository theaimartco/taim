//! A long list paints only the rows that can be seen.
//!
//! Lives with the touch suite because it is what keeps flinging a long list
//! cheap: painting every row of a thousand-row list each frame made a frame
//! cost grow with the list rather than with the screen.
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use taim::prelude::*;
use taim::view::{Constraints, DescribeCx, MeasureCx, PaintCx, Root};
use taim_core::renderer::Headless as _;
use taim_core::time::Instant;
use taim_core::{Color, Event, Point, Size, mouse, semantics};

const WINDOW: Size = Size {
    width: 300.0,
    height: 600.0,
};
const ROW: f32 = 40.0;

/// A fixed-height row that counts how often it is painted.
struct Row {
    painted: Arc<AtomicUsize>,
}

impl View for Row {
    fn measure(&mut self, _: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        Size::new(constraints.max.width.min(WINDOW.width), ROW)
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let _ = self.painted.fetch_add(1, Ordering::SeqCst);
        cx.fill(cx.bounds(), Color::BLACK, taim_core::Border::default());
    }

    fn describe(&self, _: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }
}

fn renderer() -> taim::Renderer {
    taim_futures::futures::executor::block_on(taim::Renderer::new(
        taim_core::renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .expect("the CPU renderer starts headless")
}

fn rows(count: usize, painted: &Arc<AtomicUsize>) -> Vec<AnyView> {
    (0..count)
        .map(|_| {
            AnyView::from(Row {
                painted: painted.clone(),
            })
        })
        .collect()
}

fn paint(root: &mut Root, renderer: &mut taim::Renderer, painted: &AtomicUsize) -> usize {
    use taim_core::Renderer as _;

    painted.store(0, Ordering::SeqCst);
    renderer.reset(taim_core::Rectangle::with_size(WINDOW));
    root.paint(renderer, Color::BLACK, None, Instant::now());
    painted.load(Ordering::SeqCst)
}

#[test]
fn a_scrolled_list_paints_only_its_visible_rows() {
    let mut renderer = renderer();
    let painted = Arc::new(AtomicUsize::new(0));
    let mut root = Root::new({
        let painted = painted.clone();
        move |_| scroll(column(rows(1_000, &painted)).fill_width())
    });
    root.layout(&renderer, WINDOW);

    // 600px of 40px rows is 15 rows; a row's bleed margin may add one at
    // each edge.
    let shown = paint(&mut root, &mut renderer, &painted);
    assert!((15..=17).contains(&shown), "painted {shown} of 1000 rows");

    // Scroll far down; the rows now in view are painted instead.
    let at = Point::new(100.0, 300.0);
    let _ = root.handle(
        &Event::Mouse(mouse::Event::CursorMoved { position: at }),
        Some(at),
        Instant::now(),
    );
    let _ = root.handle(
        &Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Pixels {
                x: 0.0,
                y: -20_000.0,
            },
        }),
        Some(at),
        Instant::now(),
    );
    let shown = paint(&mut root, &mut renderer, &painted);
    assert!(
        (15..=18).contains(&shown),
        "painted {shown} rows after scrolling"
    );
}

#[test]
fn without_a_clipping_ancestor_every_row_is_painted() {
    // Culling only ever applies inside a view that clips: outside one, a
    // view may draw anywhere and is always painted.
    let mut renderer = renderer();
    let painted = Arc::new(AtomicUsize::new(0));
    let mut root = Root::new({
        let painted = painted.clone();
        move |_| column(rows(50, &painted))
    });
    root.layout(&renderer, WINDOW);

    assert_eq!(paint(&mut root, &mut renderer, &painted), 50);
}

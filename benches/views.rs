//! The view tree's per-frame and per-event costs on a long list, with the
//! CPU renderer so it runs anywhere.
//!
//! `cargo bench --bench views`
#![allow(missing_docs)]
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

use taim::prelude::*;
use taim::view::Root;
use taim_core::renderer::Headless as _;
use taim_core::time::Instant;
use taim_core::{Color, Event, Point, Size, mouse};

criterion_main!(benches);
criterion_group!(benches, views);

const WINDOW: Size = Size {
    width: 400.0,
    height: 800.0,
};

fn renderer() -> taim::Renderer {
    taim_futures::futures::executor::block_on(taim::Renderer::new(
        taim_core::renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .expect("the CPU renderer starts headless")
}

fn list(rows: usize) -> Root {
    Root::new(move |_| {
        let rows: Vec<AnyView> = (0..rows)
            .map(|index| button(format!("Row {index}")).into())
            .collect();
        scroll(column(rows).gap(4.0).fill_width())
    })
}

fn views(c: &mut Criterion) {
    let mut renderer = renderer();

    for rows in [100_usize, 1_000] {
        let mut root = list(rows);
        root.layout(&renderer, WINDOW);

        // A pointer moving over the list: every view is offered the event.
        let mut group = c.benchmark_group("views/pointer_move");
        let mut y = 0.0_f32;
        let _ = group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, _| {
            b.iter(|| {
                y = (y + 7.0) % WINDOW.height;
                let at = Point::new(100.0, y);
                black_box(root.handle(
                    &Event::Mouse(mouse::Event::CursorMoved { position: at }),
                    Some(at),
                    Instant::now(),
                ))
            });
        });
        group.finish();

        let mut group = c.benchmark_group("views/is_animating");
        let _ = group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, _| {
            b.iter(|| black_box(root.is_animating(Instant::now())));
        });
        group.finish();

        let mut group = c.benchmark_group("views/layout");
        let _ = group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, _| {
            b.iter(|| root.layout(&renderer, WINDOW));
        });
        group.finish();

        let mut group = c.benchmark_group("views/paint");
        let _ = group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, _| {
            b.iter(|| {
                use taim_core::Renderer as _;
                renderer.reset(taim_core::Rectangle::with_size(WINDOW));
                root.paint(&mut renderer, Color::BLACK, None, Instant::now());
            });
        });
        group.finish();
    }
}

//! Per-phase cost of one showcase frame, in microseconds.
//!
//! `cargo run --release -p taim-showcase --example timing`
use std::time::Instant;

use taim::view::Root;
use taim_core::renderer::Headless as _;
use taim_core::{Color, Event, Point, Size, mouse};

type Renderer = taim::Renderer;

const WINDOW: Size = Size {
    width: 1024.0,
    height: 768.0,
};
const RUNS: usize = 2000;

fn micros(mut work: impl FnMut()) -> (f64, f64) {
    for _ in 0..50 {
        work();
    }
    let mut samples: Vec<f64> = (0..RUNS)
        .map(|_| {
            let start = Instant::now();
            work();
            start.elapsed().as_secs_f64() * 1e6
        })
        .collect();
    samples.sort_by(f64::total_cmp);
    (samples[RUNS / 2], samples[RUNS * 99 / 100])
}

fn main() {
    let mut renderer = futures::executor::block_on(Renderer::new(
        taim_core::renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .expect("the CPU renderer starts headless");

    let mut root = Root::new(taim_showcase::build);
    root.layout(&renderer, WINDOW);

    let nodes = taim_accessibility::Tree::from_forest(&[root.describe(WINDOW)]).len();
    println!("{nodes} semantic nodes in the tree");

    let (p50, p99) = micros(|| root.layout(&renderer, WINDOW));
    println!("layout        p50 {p50:8.1} us   p99 {p99:8.1} us");

    let bounds = taim_core::Rectangle::with_size(WINDOW);
    let (p50, p99) = micros(|| {
        use taim_core::Renderer as _;
        renderer.reset(bounds);
        root.paint(
            &mut renderer,
            Color::WHITE,
            None,
            taim_core::time::Instant::now(),
        );
    });
    println!("reset+paint   p50 {p50:8.1} us   p99 {p99:8.1} us");

    let physical = Size::new(1024_u32, 768);
    let (p50, p99) = micros(|| {
        use taim_core::Renderer as _;
        renderer.reset(bounds);
        root.paint(
            &mut renderer,
            Color::WHITE,
            None,
            taim_core::time::Instant::now(),
        );
        let _ = renderer.screenshot(physical, 1.0, Color::BLACK);
    });
    println!("full CPU frame (paint + rasterize 1024x768)  p50 {p50:8.1} us   p99 {p99:8.1} us");

    let at = Point::new(400.0, 300.0);
    let moved = Event::Mouse(mouse::Event::CursorMoved { position: at });
    let (p50, p99) = micros(|| {
        let _ = root.handle(&moved, Some(at), taim_core::time::Instant::now());
    });
    println!("pointer move  p50 {p50:8.1} us   p99 {p99:8.1} us");

    let (p50, p99) = micros(|| {
        let _ = root.describe(WINDOW);
    });
    println!("describe      p50 {p50:8.1} us   p99 {p99:8.1} us");
}

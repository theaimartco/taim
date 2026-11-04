//! Phase 3 End-to-End Production Execution Benchmarks.
//!
//! Measures performance across the 4 critical production paths:
//! 1. View-root construction (not full windowed cold start)
//! 2. Signal-to-paint preparation (retained versus cold)
//! 3. Wake storm throughput under heavy concurrent task wakeups with generational validation
//! 4. Idle CPU overhead in quiescent event loop with FrameGate

#![allow(missing_docs)]
use criterion::{Criterion, black_box, criterion_group, criterion_main};

use taim::prelude::*;
use taim::view::Root;
use taim::{Color, Rectangle, Size};
use taim_core::Renderer as _;
use taim_core::time::Instant;
use taim_core::window;
use taim_futures::futures::executor;
use taim_futures::scheduler::TaskScheduler;
use taim_renderer::Renderer;
use taim_renderer::core::renderer::{self, Headless as _};
use taim_winit::frame_gate::FrameGate;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

criterion_main!(benches);
criterion_group!(benches, bench_phase3_production);

fn view_root(count: u32) -> (Root, State<u32>) {
    let mut state = None;
    let root = Root::new(|cx| {
        let value = cx.signal(count);
        state = Some(value);
        column(views![
            text(move || format!("Counter: {}", value.get())),
            button("Click").on_press(move || value.update(|n| *n = n.wrapping_add(1))),
        ])
    });
    (root, state.expect("root initialized the state"))
}

struct CaptureFuture {
    waker: Arc<std::sync::Mutex<Option<std::task::Waker>>>,
    done: bool,
}

impl std::future::Future for CaptureFuture {
    type Output = ();
    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<()> {
        if !self.done {
            *self.waker.lock().unwrap() = Some(cx.waker().clone());
            self.done = true;
            std::task::Poll::Pending
        } else {
            std::task::Poll::Ready(())
        }
    }
}

pub fn bench_phase3_production(c: &mut Criterion) {
    let mut group = c.benchmark_group("phase3_production");

    // Root construction only: no window, renderer or first frame.
    let _ = group.bench_function("view_root_construct", |b| {
        b.iter(|| {
            let _ = black_box(view_root(black_box(0)));
        });
    });

    // Signal update to paint preparation, with no submit or pixel readback.
    let mut renderer = executor::block_on(Renderer::new(
        renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .expect("software renderer must be available");
    let viewport = Size::new(800.0, 600.0);
    let _ = group.bench_function("signal_to_paint_retained", |b| {
        let (mut root, count) = view_root(0);
        b.iter(|| {
            count.update(|n| *n = n.wrapping_add(1));
            let _ = root.take_changes();
            root.layout(&renderer, viewport);
            renderer.reset(Rectangle::with_size(viewport));
            root.paint(&mut renderer, Color::BLACK, None, Instant::now());
        });
    });
    let _ = group.bench_function("signal_to_paint_cold", |b| {
        b.iter(|| {
            let (mut root, count) = view_root(0);
            count.update(|n| *n = n.wrapping_add(1));
            root.layout(&renderer, viewport);
            renderer.reset(Rectangle::with_size(viewport));
            root.paint(&mut renderer, Color::BLACK, None, Instant::now());
            let _ = black_box(root);
        });
    });

    // 3. Wake storm: concurrent task wakeups through TaskScheduler
    let _ = group.bench_function("wake_storm", |b| {
        b.iter(|| {
            let mut scheduler = TaskScheduler::new();
            let wake_count = Arc::new(AtomicUsize::new(0));
            let wake_clone = wake_count.clone();

            scheduler.set_notifier(move || {
                let _ = wake_clone.fetch_add(1, Ordering::Relaxed);
            });

            // Spawn 100 tasks capturing their wakers
            let mut wakers = Vec::with_capacity(100);
            for _ in 0..100 {
                let captured_waker = Arc::new(std::sync::Mutex::new(None));
                let future = CaptureFuture {
                    waker: Arc::clone(&captured_waker),
                    done: false,
                };
                let _ = scheduler.spawn(Box::pin(future));
                wakers.push(captured_waker);
            }

            // Initial poll to capture all wakers
            let _ = scheduler.poll_ready();

            // Simulate wake storm: all 100 tasks wake concurrently
            for captured in &wakers {
                if let Some(waker) = captured.lock().unwrap().take() {
                    waker.wake();
                }
            }

            // Poll ready tasks resulting from the wake storm
            let polled = scheduler.poll_ready();
            let _ = black_box(polled);
        });
    });

    // 4. Idle CPU / Quiescent loop: FrameGate evaluating clean windows
    let _ = group.bench_function("idle_cpu", |b| {
        let mut gate = FrameGate::new();
        let win = window::Id::unique();
        gate.track_window(win, 1.0);
        gate.on_frame_presented(win);

        b.iter(|| {
            let is_idle = gate.is_idle();
            let immediate = gate.has_immediate_work();
            let deadline = gate.next_wake_deadline();
            let should_render = gate
                .tracker(win)
                .map(taim_winit::WindowStateTracker::should_render);

            let _ = black_box((is_idle, immediate, deadline, should_render));
        });
    });

    group.finish();
}

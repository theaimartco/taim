//! Phase 3 End-to-End Production Execution Benchmarks.
//!
//! Measures performance across the 4 critical production paths:
//! 1. Cold start / boot latency (`AppRunner::new`)
//! 2. Keystroke-to-raster update & draw latency
//! 3. Wake storm throughput under heavy concurrent task wakeups with generational validation
//! 4. Idle CPU overhead in quiescent event loop with FrameGate

#![allow(missing_docs)]
use criterion::{black_box, Criterion, criterion_group, criterion_main};

use taim::widget::{button, column, text};
use taim::window;
use taim::{App, AppContext, AppRunner, Element, Size, Task, Theme, WindowContext};
use taim_futures::scheduler::TaskScheduler;
use taim_winit::frame_gate::FrameGate;
use taim_renderer::Renderer;
use taim_renderer::core::renderer::{self, Headless as _};
use taim_runtime::UserInterface;
use taim_runtime::user_interface;
use taim_futures::futures::executor;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

criterion_main!(benches);
criterion_group!(benches, bench_phase3_production);

#[derive(Debug, Clone)]
struct BenchmarkApp {
    count: u32,
}

#[derive(Debug, Clone)]
enum BenchMessage {
    Keystroke(char),
}

impl App for BenchmarkApp {
    type State = u32;
    type Message = BenchMessage;
    type Theme = Theme;
    type Renderer = Renderer;
    type Executor = taim::executor::Default;

    fn name() -> &'static str {
        "bench_phase3"
    }

    fn boot(&self, _context: &mut AppContext) -> (Self::State, Task<Self::Message>) {
        (self.count, Task::none())
    }

    fn update(
        &self,
        state: &mut Self::State,
        message: Self::Message,
        context: &mut AppContext,
    ) -> Task<Self::Message> {
        match message {
            BenchMessage::Keystroke(c) => {
                let _ = black_box(c);
                *state = state.wrapping_add(1);
                context.request_redraw();
                Task::none()
            }
        }
    }

    fn view<'a>(
        &self,
        state: &'a Self::State,
        _window: &WindowContext,
    ) -> Element<'a, Self::Message, Self::Theme, Self::Renderer> {
        column![
            text(format!("Counter: {state}")),
            button("Click").on_press(BenchMessage::Keystroke('c')),
        ]
        .into()
    }
}

struct CaptureFuture {
    waker: Arc<std::sync::Mutex<Option<std::task::Waker>>>,
    done: bool,
}

impl std::future::Future for CaptureFuture {
    type Output = ();
    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<()> {
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

    // 1. Cold start benchmark: measuring AppRunner::new instantiation
    let _ = group.bench_function("cold_start", |b| {
        let app = BenchmarkApp { count: 0 };
        b.iter(|| {
            let (runner, task) = AppRunner::new(black_box(app.clone()));
            let _ = black_box(runner);
            let _ = black_box(task);
        });
    });

    // 2. Keystroke-to-raster: message update -> UI build -> draw layout
    let mut renderer = executor::block_on(Renderer::new(
        renderer::Settings::default(),
        Some("software"),
    ))
    .expect("software renderer must be available");

    let _ = group.bench_function("keystroke_to_raster", |b| {
        let app = BenchmarkApp { count: 0 };
        let (mut runner, _) = AppRunner::new(app);
        let win_ctx = WindowContext::new(window::Id::unique(), 1.0);
        let viewport = Size::new(800.0, 600.0);

        b.iter(|| {
            // Update
            let _ = runner.update(BenchMessage::Keystroke('a'));

            // Build UI
            let view = runner.view(&win_ctx);
            let mut ui = UserInterface::build(
                view,
                viewport,
                user_interface::Cache::default(),
                &mut renderer,
            );

            // Draw
            ui.draw(
                &mut renderer,
                &Theme::Dark,
                &renderer::Style::default(),
                taim::mouse::Cursor::Unavailable,
            );
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
            let should_render = gate.tracker(win).map(taim_winit::WindowStateTracker::should_render);

            let _ = black_box((is_idle, immediate, deadline, should_render));
        });
    });

    group.finish();
}

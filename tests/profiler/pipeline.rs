//! The profiler, reached the way an application reaches it.
//!
//! `aura_profiler`'s own tests prove the collector and the export. They do
//! not prove that anything *calls* it — which is the gap the accessibility
//! engine sat in for an entire phase: every stage correct, tested, and
//! wired to nothing.
//!
//! These drive a real `UserInterface` through a real frame and read the
//! trace that falls out.

use aura::widget::{button, column, text};
use aura::{Element, Size, Theme};
use aura_profiler::export;
use aura_profiler::{Collector, Phase as Traced};
use aura_renderer::Renderer;
use aura_runtime::diagnostics::Trace;
use aura_runtime::user_interface::{Cache, UserInterface};

type Message = &'static str;

const WINDOW: Size = Size {
    width: 400.0,
    height: 400.0,
};

fn headless_renderer() -> Renderer {
    aura_futures::futures::executor::block_on(
        <Renderer as aura::advanced::renderer::Headless>::new(
            aura::advanced::renderer::Settings::default(),
            Some("tiny-skia"),
        ),
    )
    .expect("tiny-skia renderer must be available")
}

fn view() -> Element<'static, Message, Theme, Renderer> {
    column![button("Save").on_press("save"), text("ready")].into()
}

/// Which phases a trace saw, in order of first appearance.
fn phases(collector: &Collector) -> Vec<Traced> {
    let mut seen = Vec::new();

    for event in collector.events() {
        if let Some(phase) = event.phase()
            && !seen.contains(&phase)
        {
            seen.push(phase);
        }
    }

    seen
}

#[test]
fn a_real_frame_produces_a_real_trace() {
    // The regression this exists to prevent: `diagnostics::Trace` being
    // correct, tested, and called by nothing.
    let collector = Collector::enabled();
    let mut renderer = headless_renderer();

    let mut ui = UserInterface::build(
        view(),
        WINDOW,
        Cache::new().with_trace(Trace::new(collector.clone())),
        &mut renderer,
    );

    let mut messages = Vec::new();
    let _ = ui.update(
        &aura_core::window::Headless,
        &aura_core::shell::Waker::noop(),
        &[],
        aura_core::mouse::Cursor::Unavailable,
        &mut renderer,
        &mut messages,
    );

    ui.draw(
        &mut renderer,
        &Theme::Dark,
        &aura_core::renderer::Style::default(),
        aura_core::mouse::Cursor::Unavailable,
    );

    let _ = ui.semantics(&renderer);

    let seen = phases(&collector);

    for phase in [
        Traced::Diff,
        Traced::Layout,
        Traced::Input,
        Traced::Paint,
        Traced::Semantics,
    ] {
        assert!(
            seen.contains(&phase),
            "an ordinary frame must record {phase:?}: saw {seen:?}"
        );
    }
}

#[test]
fn a_frames_time_is_attributed_to_its_phases() {
    // Phase 15's exit gate, through the real pipeline: every metric traces
    // to a phase. A total with no breakdown is a number a developer can do
    // nothing with.
    let collector = Collector::enabled();
    let mut renderer = headless_renderer();

    let mut ui = UserInterface::build(
        view(),
        WINDOW,
        Cache::new().with_trace(Trace::new(collector.clone())),
        &mut renderer,
    );

    ui.draw(
        &mut renderer,
        &Theme::Dark,
        &aura_core::renderer::Style::default(),
        aura_core::mouse::Cursor::Unavailable,
    );

    let totals = export::phase_totals(&collector.events());

    assert!(
        !totals.is_empty(),
        "a drawn frame must attribute time to at least one phase"
    );

    for (phase, total) in &totals {
        assert!(
            *total > 0,
            "{phase:?} recorded a zero-length span; the clock is not being read"
        );
    }
}

#[test]
fn every_span_the_pipeline_opens_is_closed() {
    // A leaked span does not fail, it lies: it silently charges the rest
    // of the frame -- and every frame after it -- to whichever phase
    // stayed open.
    let collector = Collector::enabled();
    let mut renderer = headless_renderer();

    let mut ui = UserInterface::build(
        view(),
        WINDOW,
        Cache::new().with_trace(Trace::new(collector.clone())),
        &mut renderer,
    );

    // `draw` returns early when there is no overlay -- which is this
    // view's case, and the path a hand-written `end` would have missed.
    ui.draw(
        &mut renderer,
        &Theme::Dark,
        &aura_core::renderer::Style::default(),
        aura_core::mouse::Cursor::Unavailable,
    );

    let mut open: Vec<Traced> = Vec::new();

    for event in collector.events() {
        match event {
            aura_profiler::Event::PhaseBegin { phase, .. } => open.push(phase),
            aura_profiler::Event::PhaseEnd { phase, .. } => {
                let found = open.iter().rposition(|held| *held == phase);

                assert!(found.is_some(), "{phase:?} ended without beginning");

                let _ = open.remove(found.unwrap());
            }
            _ => {}
        }
    }

    assert!(open.is_empty(), "spans left open: {open:?}");
}

#[test]
fn a_trace_survives_a_relayout() {
    // A resize rebuilds the `UserInterface`. A trace that switched itself
    // off the first time the window changed size would be a puzzle rather
    // than a tool.
    let collector = Collector::enabled();
    let mut renderer = headless_renderer();

    let ui = UserInterface::build(
        view(),
        WINDOW,
        Cache::new().with_trace(Trace::new(collector.clone())),
        &mut renderer,
    );

    let before = collector.len();

    let mut ui = ui.relayout(Size::new(200.0, 200.0), &mut renderer);

    assert!(
        collector.len() > before,
        "the rebuild must still be recorded"
    );

    ui.draw(
        &mut renderer,
        &Theme::Dark,
        &aura_core::renderer::Style::default(),
        aura_core::mouse::Cursor::Unavailable,
    );

    assert!(phases(&collector).contains(&Traced::Paint));
}

#[test]
fn an_application_that_did_not_ask_for_a_trace_records_nothing() {
    // The default, and what a shipped build gets. If this ever fails, the
    // 1% disabled-overhead budget is gone and nobody asked for it.
    let mut renderer = headless_renderer();

    let mut ui = UserInterface::build(view(), WINDOW, Cache::new(), &mut renderer);

    let mut messages = Vec::new();
    let _ = ui.update(
        &aura_core::window::Headless,
        &aura_core::shell::Waker::noop(),
        &[],
        aura_core::mouse::Cursor::Unavailable,
        &mut renderer,
        &mut messages,
    );

    ui.draw(
        &mut renderer,
        &Theme::Dark,
        &aura_core::renderer::Style::default(),
        aura_core::mouse::Cursor::Unavailable,
    );

    assert!(
        ui.trace().collector().is_empty(),
        "a UserInterface built without a trace must record nothing"
    );
}

#[test]
fn a_trace_of_a_real_frame_exports_something_a_tool_can_open() {
    let collector = Collector::enabled();
    let mut renderer = headless_renderer();

    let mut ui = UserInterface::build(
        view(),
        WINDOW,
        Cache::new().with_trace(Trace::new(collector.clone())),
        &mut renderer,
    );

    ui.draw(
        &mut renderer,
        &Theme::Dark,
        &aura_core::renderer::Style::default(),
        aura_core::mouse::Cursor::Unavailable,
    );

    let trace = export::aura(&collector);

    assert!(trace.starts_with("{\"schema\":1,"), "{trace}");
    assert!(trace.contains("\"phase\":\"layout\""), "{trace}");
    assert!(trace.contains("\"traceEvents\""), "{trace}");
    assert!(
        trace.contains("\"overflow\":0"),
        "an ordinary frame must not overflow the buffer: {trace}"
    );
}

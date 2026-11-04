//! Native view-root lifecycle and platform frame-gate contracts.
//! Program/AppRunner were removed; these exercise the live root contract.
use taim::prelude::*;
use taim::view::Root;
use taim_core::window;
use taim_winit::frame_gate::{DirtyFlags, FrameGate, PlatformEvent};

#[test]
fn native_state_updates_invalidate_the_persistent_view_root() {
    let mut state = None;
    let mut root = Root::new(|cx| {
        let count = cx.signal(100_u32);
        state = Some(count);
        text(move || format!("Count: {}", count.get()))
    });
    // Reactive closures establish their dependencies on the first layout.
    use taim_core::renderer::Headless as _;
    let renderer = taim_futures::futures::executor::block_on(taim::Renderer::new(
        taim_core::renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .expect("CPU renderer");
    root.layout(&renderer, taim::Size::new(320.0, 120.0));
    assert_eq!(root.take_changes(), (false, false));
    state.unwrap().update(|count| *count += 50);
    assert_eq!(state.unwrap().get(), 150);
    assert_eq!(root.take_changes(), (true, true));
    assert_eq!(root.take_changes(), (false, false));
}

#[test]
fn building_an_application_does_not_open_a_platform_window() {
    let app = taim::app("Native contract", |_| text("Ready"));
    assert!(format!("{app:?}").contains("Native contract"));
}

#[test]
fn test_frame_gate_controls_real_redraw_path() {
    let mut gate = FrameGate::new();
    let id = window::Id::unique();
    gate.track_window(id, 1.0);
    gate.on_frame_presented(id);

    assert!(!gate.tracker(id).unwrap().should_render());

    gate.invalidate(id, DirtyFlags::PAINT_DIRTY);
    assert!(gate.tracker(id).unwrap().should_render());

    gate.on_frame_presented(id);
    assert!(!gate.tracker(id).unwrap().should_render());
}

#[test]
fn test_duplicate_redraws_result_in_one_render() {
    let mut gate = FrameGate::new();
    let id = window::Id::unique();
    gate.track_window(id, 1.0);
    gate.on_frame_presented(id);

    for _ in 0..10 {
        gate.invalidate(id, DirtyFlags::PAINT_DIRTY);
    }

    assert_eq!(gate.collapsed_redraws(), 9);
    assert!(gate.tracker(id).unwrap().should_render());

    gate.on_frame_presented(id);
    assert_eq!(gate.tracker(id).unwrap().dirty_flags, DirtyFlags::CLEAN);
    assert!(!gate.tracker(id).unwrap().should_render());
}

#[test]
fn test_clean_window_performs_no_render_work() {
    let mut gate = FrameGate::new();
    let id = window::Id::unique();
    gate.track_window(id, 1.0);
    gate.on_frame_presented(id);

    assert_eq!(gate.tracker(id).unwrap().dirty_flags, DirtyFlags::CLEAN);
    assert!(!gate.tracker(id).unwrap().should_render());
    assert_eq!(gate.dirty_windows().count(), 0);
}

#[test]
fn test_occluded_window_skips_render() {
    let mut gate = FrameGate::new();
    let id = window::Id::unique();
    gate.track_window(id, 1.0);
    gate.on_frame_presented(id);

    let _ = gate.handle_event(id, PlatformEvent::OcclusionChanged(true));
    gate.invalidate(id, DirtyFlags::PAINT_DIRTY);

    assert!(!gate.tracker(id).unwrap().should_render());
    assert_eq!(gate.dirty_windows().count(), 0);

    let _ = gate.handle_event(id, PlatformEvent::OcclusionChanged(false));
    assert!(gate.tracker(id).unwrap().should_render());
    assert_eq!(gate.dirty_windows().count(), 1);
}

#[test]
fn test_idle_runtime_has_zero_internal_frame_generation() {
    let mut gate = FrameGate::new();
    let id = window::Id::unique();
    gate.track_window(id, 1.0);
    gate.on_frame_presented(id);

    let initial_gen = gate.tracker(id).unwrap().frame_generation;
    assert!(gate.is_idle());
    assert!(gate.is_idle_now());
    assert!(!gate.has_immediate_work());
    assert_eq!(gate.next_wake_deadline(), None);
    assert_eq!(gate.tracker(id).unwrap().frame_generation, initial_gen);
}

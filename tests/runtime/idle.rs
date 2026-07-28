//! Proves that a quiescent [`Scheduler`] produces no further frames, and
//! that a real, unchanging [`UserInterface`] settles into that
//! quiescent state rather than requesting redraws forever.
//!
//! "Idle gate passes" (`phase.md` Phase 6 exit gate) means exactly this:
//! once nothing has changed, the scheduler stops being asked to (and
//! stops agreeing to) produce frames, submit work, or lay
//! out/paint anything.

use aura::advanced::widget::{self, Widget};
use aura::advanced::{Layout, mouse, renderer};
use aura::{Element, Length, Rectangle, Size, Theme};
use aura_renderer::Renderer;
use aura_runtime::frame_scheduler::{MotionId, Scheduler};
use aura_runtime::user_interface::{self, UserInterface};
use std::time::{Duration, Instant};

use aura_core::animation::MotionClock;
use aura_motion::{Easing, Transition};

/// A widget that records every time it is asked to compute layout or
/// draw, and never requests a redraw or reports any change on its own --
/// the "nothing is happening" baseline a real idle application would
/// eventually settle into.
struct Quiet {
    layout_calls: std::rc::Rc<std::cell::Cell<u32>>,
    draw_calls: std::rc::Rc<std::cell::Cell<u32>>,
}

impl<Message> Widget<Message, Theme, Renderer> for Quiet {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(10.0), Length::Fixed(10.0))
    }

    fn layout(
        &mut self,
        _tree: &mut widget::Tree,
        _renderer: &Renderer,
        _limits: &aura::advanced::layout::Limits,
    ) -> aura::advanced::layout::Node {
        self.layout_calls.set(self.layout_calls.get() + 1);
        aura::advanced::layout::Node::new(Size::new(10.0, 10.0))
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        self.draw_calls.set(self.draw_calls.get() + 1);
    }
}

fn headless_renderer() -> Renderer {
    aura_futures::futures::executor::block_on(
        <Renderer as aura::advanced::renderer::Headless>::new(
            aura::advanced::renderer::Settings::default(),
            Some("tiny-skia"),
        ),
    )
    .expect("tiny-skia renderer must be available")
}

#[test]
fn a_scheduler_with_nothing_pending_never_produces_a_frame() {
    let scheduler = Scheduler::new();
    let now = Instant::now();

    for offset in [0, 1, 100, 10_000] {
        assert!(!scheduler.should_produce_frame(now + Duration::from_millis(offset)));
    }
}

/// Drives a real [`aura_motion::Transition`] through the real
/// [`Scheduler`], proving the two systems compose the way `phase.md`
/// Phase 9's "idle resumes immediately when motion completes" requires:
/// while the transition is running, a driver keeps it registered and the
/// scheduler agrees to produce frames; the instant the transition
/// settles, the driver unregisters it and the scheduler goes idle on the
/// very next check -- no trailing speculative frame.
#[test]
fn a_running_motion_keeps_the_scheduler_awake_and_idle_resumes_on_completion() {
    let origin = Instant::now();
    let clock = MotionClock::new(origin);

    let mut transition = Transition::new(0.0_f32, clock.at(origin));
    transition.retarget(1.0, clock.at(origin), 0.5, Easing::Linear);

    let mut scheduler = Scheduler::new();
    let motion = MotionId(1);

    // A driver registers the motion for as long as it is running, and
    // samples/re-checks it each frame, exactly as a real animation
    // subscription would.
    let mut drive = |scheduler: &mut Scheduler, at: Instant| {
        if transition.is_running_at(clock.at(at)) {
            scheduler.register_motion(motion);
        } else {
            scheduler.unregister_motion(motion);
        }
        let _ = transition.value_at(clock.at(at));
    };

    // Mid-flight: the motion is running, so the scheduler stays awake.
    let midpoint = origin + Duration::from_millis(250);
    drive(&mut scheduler, midpoint);
    assert_eq!(scheduler.active_motion_count(), 1);
    assert!(
        scheduler.should_produce_frame(midpoint),
        "a registered, running motion must keep the scheduler producing frames"
    );

    // Still running just before the end.
    let almost = origin + Duration::from_millis(499);
    drive(&mut scheduler, almost);
    assert_eq!(scheduler.active_motion_count(), 1);

    // At completion: the driver unregisters, and the scheduler goes idle
    // immediately -- no further frame is produced.
    let done = origin + Duration::from_millis(500);
    drive(&mut scheduler, done);
    assert_eq!(
        scheduler.active_motion_count(),
        0,
        "a completed motion must be unregistered"
    );
    assert!(
        !scheduler.should_produce_frame(done + Duration::from_millis(1)),
        "idle must resume immediately once the last motion completes"
    );
}

#[test]
fn an_unchanging_real_ui_settles_and_stops_requesting_redraws() {
    let layout_calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let draw_calls = std::rc::Rc::new(std::cell::Cell::new(0));

    let element: Element<'_, ()> = Element::new(Quiet {
        layout_calls: layout_calls.clone(),
        draw_calls: draw_calls.clone(),
    });

    let mut renderer = headless_renderer();
    let mut ui = UserInterface::build(
        element,
        Size::new(200.0, 200.0),
        user_interface::Cache::new(),
        &mut renderer,
    );

    ui.draw(
        &mut renderer,
        &Theme::Light,
        &renderer::Style::default(),
        mouse::Cursor::Unavailable,
    );

    let after_first_build_layout_calls = layout_calls.get();
    let after_first_build_draw_calls = draw_calls.get();

    assert!(
        after_first_build_layout_calls >= 1,
        "the first build must lay out at least once"
    );
    assert!(
        after_first_build_draw_calls >= 1,
        "the first draw must draw at least once"
    );

    // Nothing changes: no events, no messages. A real event loop would
    // not even call `update` again without an event to process, but
    // this proves that if it did, an unchanging widget contributes no
    // further layout work and requests no redraw.
    let window = aura_core::window::Headless;
    let waker = aura_core::shell::Waker::noop();
    let mut messages = Vec::new();

    let (state, _statuses) = ui.update(
        &window,
        &waker,
        &[],
        mouse::Cursor::Unavailable,
        &mut renderer,
        &mut messages,
    );

    assert!(
        messages.is_empty(),
        "an unchanging widget must not publish any message"
    );
    assert!(
        !state.is_animating(),
        "a widget that never calls Shell::request_animation must not report animating"
    );
    assert_eq!(
        layout_calls.get(),
        after_first_build_layout_calls,
        "update() on an unchanging UI with no events must not re-run layout"
    );
}

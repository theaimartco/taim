//! Damage correctness, checked against real pixels.
//!
//! `damage_motion.rs` checks that the damage *computation* covers a
//! movement's bounds -- a property of the arithmetic. This checks the
//! properties that arithmetic depends on, by rasterising frames and
//! comparing them. Its module doc used to say "we cannot render actual
//! pixels without a GPU context in unit tests"; with `tiny-skia` the whole
//! suite rasterises on the CPU, so that is no longer true.
//!
//! # What partial present needs to be true
//!
//! 1. **Rendering is deterministic.** Two renders of the same tree must be
//!    byte identical, or every damage optimisation is unsound and every
//!    golden is flaky.
//! 2. **A change stays inside the widget that changed.** If a widget paints
//!    outside its own layout bounds, a damage rect computed from those
//!    bounds will not cover it, and a partial present leaves a stale
//!    rectangle on screen forever.
//!
//! Neither is checked anywhere else, and (2) is the one that fails silently.
//!
//! # What is not checked here yet
//!
//! Driving a real event (scroll, hover) through `UserInterface::update` and
//! asserting the runtime's own `State::damage()` covers the resulting pixel
//! change. The plumbing to make a headless interface respond to a wheel
//! event needs work this suite does not have yet, and a test that pretended
//! otherwise would be worse than an honest gap. Recorded in `Capstone.md`.

type Element<'a, M, T = taim_core::Theme, R = taim_renderer::Renderer> =
    taim_core::Element<'a, M, T, R>;
use taim_core::Renderer as _;
use taim_core::renderer::Headless as _;
use taim_core::theme::Base as _;
use taim_core::{Rectangle, Size, Theme};
use taim_renderer::Renderer;
use taim_widget::{column, container, text};

const WIDTH: u32 = 240;
const HEIGHT: u32 = 120;

/// Renders one tree and returns its pixels.
fn render(build: impl FnOnce() -> Element<'static, ()>) -> Vec<u8> {
    let theme = Theme::Dark;
    let base = theme.base();
    let logical = Size::new(WIDTH as f32, HEIGHT as f32);

    let mut renderer = taim_futures::futures::executor::block_on(
        <Renderer as taim_core::renderer::Headless>::new(
            taim_core::renderer::Settings::default(),
            Some("tiny-skia"),
        ),
    )
    .expect("the tiny-skia renderer must be available");

    renderer.reset(Rectangle::with_size(logical));

    let mut interface = taim_runtime::UserInterface::build(
        build(),
        logical,
        taim_runtime::user_interface::Cache::new(),
        &mut renderer,
    );

    interface.draw(
        &mut renderer,
        &theme,
        &taim_core::renderer::Style {
            text_color: base.text_color,
        },
        taim_core::mouse::Cursor::Unavailable,
    );

    // The tree must outlive the screenshot; see `harness::render_element`.
    let rgba = renderer.screenshot(Size::new(WIDTH, HEIGHT), 1.0, base.background_color);

    drop(interface);

    rgba
}

/// Pixels differing from the background.
fn ink(rgba: &[u8]) -> usize {
    let background = &rgba[0..4];

    rgba.chunks_exact(4)
        .filter(|pixel| {
            pixel[..3]
                .iter()
                .zip(&background[..3])
                .any(|(a, b)| a.abs_diff(*b) > 8)
        })
        .count()
}

/// The rows in which two frames differ, as `(first, last)`.
fn changed_rows(before: &[u8], after: &[u8]) -> Option<(u32, u32)> {
    let mut first = None;
    let mut last = None;

    for y in 0..HEIGHT {
        let start = (y * WIDTH * 4) as usize;
        let end = start + (WIDTH * 4) as usize;

        if before[start..end] != after[start..end] {
            let _ = first.get_or_insert(y);
            last = Some(y);
        }
    }

    Some((first?, last?))
}

#[test]
fn rendering_is_deterministic() {
    let build = || {
        container(column![text("Stable").size(16), text("Frame").size(16)].spacing(8))
            .padding(12)
            .into()
    };

    let first = render(build);
    let second = render(build);

    // Non-vacuous: a pair of blank frames would be trivially equal, and this
    // test would pass while proving nothing at all.
    assert!(
        ink(&first) > 0,
        "the fixture rendered nothing, so equality proves nothing"
    );

    assert_eq!(
        first, second,
        "two renders of an identical tree produced different pixels; every \
         damage optimisation and every golden in this suite depends on this \
         not happening"
    );
}

#[test]
fn a_change_is_confined_to_the_widget_that_changed() {
    // Two rows, only the second differs. Every changed pixel must fall in
    // the second row's band -- a widget that painted outside its own bounds
    // would show up here as a change in the first row's rows, and would
    // break any partial present computed from layout bounds.
    let before = render(|| {
        column![text("Row one").size(16), text("Row two").size(16)]
            .spacing(8)
            .into()
    });

    let after = render(|| {
        column![text("Row one").size(16), text("CHANGED").size(16)]
            .spacing(8)
            .into()
    });

    assert!(ink(&before) > 0, "the fixture rendered nothing");
    assert_ne!(before, after, "the two frames must actually differ");

    let (first, last) = changed_rows(&before, &after).expect("frames differ, so rows must differ");

    // The first row's glyphs occupy roughly the top 20 rows at size 16.
    // The change must begin below them.
    assert!(
        first >= 18,
        "a change to the second row altered pixels from row {first}, which is \
         inside the first row's band -- the widget painted outside its own \
         layout bounds"
    );

    assert!(last < HEIGHT, "changed rows must stay inside the surface");
}

#[test]
fn a_tree_rebuilt_with_different_content_reports_no_damage() {
    // Recorded because it is surprising and load-bearing: damage is
    // attributed from dirty flags raised while handling events, so a caller
    // that swaps the widget tree between frames gets `None` -- not `Full`.
    // Anything built on `State::damage()` (a partial present, `Capstone.md`
    // F-08) must treat `None` as "no information", never as "nothing
    // changed", or it will present a stale frame.
    let logical = Size::new(WIDTH as f32, HEIGHT as f32);

    let mut renderer = taim_futures::futures::executor::block_on(
        <Renderer as taim_core::renderer::Headless>::new(
            taim_core::renderer::Settings::default(),
            Some("tiny-skia"),
        ),
    )
    .expect("the tiny-skia renderer must be available");

    renderer.reset(Rectangle::with_size(logical));

    let first: Element<'static, ()> = text("Alpha").size(16).into();

    let cache = taim_runtime::UserInterface::build(
        first,
        logical,
        taim_runtime::user_interface::Cache::new(),
        &mut renderer,
    )
    .into_cache();

    let second: Element<'static, ()> = text("Omega").size(16).into();

    let mut interface = taim_runtime::UserInterface::build(second, logical, cache, &mut renderer);

    let mut messages = Vec::new();

    let (state, _statuses) = interface.update(
        &taim_core::window::Headless,
        &taim_core::shell::Waker::noop(),
        &[],
        taim_core::mouse::Cursor::Unavailable,
        &mut renderer,
        &mut messages,
    );

    assert!(
        state.damage_region().is_none(),
        "if this now reports damage, the note above is out of date and \
         partial present has a source it did not have before"
    );
}

// ── Partial present: which region is safe to confine a frame to ───────
//
// `Renderer::accumulate_damage` decides that. It is the part of partial
// present that can be wrong silently: too large a region only costs work,
// while too small a region leaves stale pixels on screen that never repaint.

/// A `wgpu` renderer, or `None` when no adapter is available.
fn gpu() -> Option<taim_wgpu::Renderer> {
    taim_futures::futures::executor::block_on(
        <taim_wgpu::Renderer as taim_core::renderer::Headless>::new(
            taim_core::renderer::Settings::default(),
            None,
        ),
    )
}

macro_rules! needs_gpu {
    ($renderer:ident) => {
        let Some(mut $renderer) = gpu() else {
            eprintln!("skipped: no wgpu adapter available");
            return;
        };
    };
}

fn area(x: f32, y: f32, w: f32, h: f32) -> taim_wgpu::graphics::damage::DamageRegion {
    taim_wgpu::graphics::damage::DamageRegion::from_rect(taim::Rectangle {
        x,
        y,
        width: w,
        height: h,
    })
    .expect("a non-empty rectangle is a valid damage region")
}

const SURFACE: taim::Size<u32> = taim::Size {
    width: 400,
    height: 300,
};

#[test]
fn unknown_damage_forces_a_full_redraw() {
    needs_gpu!(renderer);

    assert_eq!(
        renderer.accumulate_damage(None, SURFACE, 3),
        None,
        "`None` means the runtime could not attribute damage; confining the \
         frame to anything would be a guess"
    );
}

#[test]
fn full_damage_forces_a_full_redraw() {
    needs_gpu!(renderer);

    let full = taim_wgpu::graphics::damage::DamageRegion::full();

    assert_eq!(renderer.accumulate_damage(Some(&full), SURFACE, 3), None);
}

#[test]
fn a_history_shallower_than_the_swapchain_forces_a_full_redraw() {
    needs_gpu!(renderer);

    let small = area(10.0, 10.0, 20.0, 20.0);

    // The first two frames cannot know what the third buffer holds.
    assert_eq!(renderer.accumulate_damage(Some(&small), SURFACE, 3), None);
    assert_eq!(renderer.accumulate_damage(Some(&small), SURFACE, 3), None);

    assert!(
        renderer
            .accumulate_damage(Some(&small), SURFACE, 3)
            .is_some(),
        "once the history is as deep as the swapchain, every buffer's staleness \
         is accounted for and a partial frame is safe"
    );
}

#[test]
fn the_region_covers_every_frame_still_stale_in_some_buffer() {
    needs_gpu!(renderer);

    // Three frames, three disjoint areas. Whichever of three buffers comes
    // back, it is missing at most these three, so all three must be redrawn.
    let regions = [
        area(0.0, 0.0, 20.0, 20.0),
        area(100.0, 100.0, 20.0, 20.0),
        area(200.0, 40.0, 30.0, 30.0),
    ];

    let mut last = None;

    for region in &regions {
        last = renderer.accumulate_damage(Some(region), SURFACE, 3);
    }

    let union = last.expect("the third frame has a deep enough history");

    assert!(
        union.x == 0 && union.y == 0,
        "the union must reach the earliest frame's origin, got {union:?}"
    );

    assert!(
        union.x + union.width >= 230 && union.y + union.height >= 120,
        "the union must reach the far corner of every frame in the history, \
         got {union:?}; anything less leaves a buffer showing stale pixels"
    );
}

#[test]
fn a_union_covering_the_surface_is_reported_as_a_full_redraw() {
    needs_gpu!(renderer);

    let whole = area(0.0, 0.0, 400.0, 300.0);

    for _ in 0..3 {
        let _ = renderer.accumulate_damage(Some(&whole), SURFACE, 3);
    }

    assert_eq!(
        renderer.accumulate_damage(Some(&whole), SURFACE, 3),
        None,
        "scissoring to the whole surface is a full redraw with extra steps"
    );
}

#[test]
fn a_full_frame_resets_the_history() {
    needs_gpu!(renderer);

    let corner = area(300.0, 200.0, 40.0, 40.0);

    for _ in 0..3 {
        let _ = renderer.accumulate_damage(Some(&corner), SURFACE, 3);
    }

    // A full redraw makes every buffer current, so nothing before it is stale.
    assert_eq!(renderer.accumulate_damage(None, SURFACE, 3), None);

    let near_origin = area(0.0, 0.0, 10.0, 10.0);

    assert_eq!(
        renderer.accumulate_damage(Some(&near_origin), SURFACE, 3),
        None,
        "the history restarts after a full frame, so the next frames are full \
         again until it is deep enough"
    );
}

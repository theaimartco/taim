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

use aura::widget::{column, container, text};
use aura::{Element, Rectangle, Size, Theme};
use aura_core::Renderer as _;
use aura_core::renderer::Headless as _;
use aura_core::theme::Base as _;
use aura_renderer::Renderer;

const WIDTH: u32 = 240;
const HEIGHT: u32 = 120;

/// Renders one tree and returns its pixels.
fn render(build: impl FnOnce() -> Element<'static, ()>) -> Vec<u8> {
    let theme = Theme::Dark;
    let base = theme.base();
    let logical = Size::new(WIDTH as f32, HEIGHT as f32);

    let mut renderer = aura_futures::futures::executor::block_on(
        <Renderer as aura_core::renderer::Headless>::new(
            aura_core::renderer::Settings::default(),
            Some("tiny-skia"),
        ),
    )
    .expect("the tiny-skia renderer must be available");

    renderer.reset(Rectangle::with_size(logical));

    let mut interface = aura_runtime::UserInterface::build(
        build(),
        logical,
        aura_runtime::user_interface::Cache::new(),
        &mut renderer,
    );

    interface.draw(
        &mut renderer,
        &theme,
        &aura_core::renderer::Style {
            text_color: base.text_color,
        },
        aura_core::mouse::Cursor::Unavailable,
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

    let mut renderer = aura_futures::futures::executor::block_on(
        <Renderer as aura_core::renderer::Headless>::new(
            aura_core::renderer::Settings::default(),
            Some("tiny-skia"),
        ),
    )
    .expect("the tiny-skia renderer must be available");

    renderer.reset(Rectangle::with_size(logical));

    let first: Element<'static, ()> = text("Alpha").size(16).into();

    let cache = aura_runtime::UserInterface::build(
        first,
        logical,
        aura_runtime::user_interface::Cache::new(),
        &mut renderer,
    )
    .into_cache();

    let second: Element<'static, ()> = text("Omega").size(16).into();

    let mut interface = aura_runtime::UserInterface::build(second, logical, cache, &mut renderer);

    let mut messages = Vec::new();

    let (state, _statuses) = interface.update(
        &aura_core::window::Headless,
        &aura_core::shell::Waker::noop(),
        &[],
        aura_core::mouse::Cursor::Unavailable,
        &mut renderer,
        &mut messages,
    );

    assert!(
        state.damage_region().is_none(),
        "if this now reports damage, the note above is out of date and \
         partial present has a source it did not have before"
    );
}

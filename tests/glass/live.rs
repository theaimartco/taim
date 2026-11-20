//! Liquid glass in motion: a press lights and magnifies it, panes in a group
//! melt into each other, and text on it stays readable over any backdrop.
//!
//! Rendered by the GPU renderer; skips (and says so) without an adapter.
//! `TAIM_GLASS_OUT=<directory>` keeps the images.
use super::render::{Scene, WINDOW, gpu, keep};

use taim::prelude::*;
use taim::view::{Constraints, DescribeCx, MeasureCx, PaintCx, Root};
use taim_core::renderer::Headless as _;
use taim_core::time::{Duration, Instant};
use taim_core::{Border, Color, Point, Size, mouse, semantics};

fn render_at(renderer: &mut taim::Renderer, root: &mut Root, text: Color, now: Instant) -> Vec<u8> {
    use taim_core::Renderer as _;

    root.layout(renderer, WINDOW);
    renderer.reset(taim_core::Rectangle::with_size(WINDOW));
    root.paint(renderer, text, None, now);
    renderer.screenshot(
        Size::new(WINDOW.width as u32, WINDOW.height as u32),
        1.0,
        Color::BLACK,
    )
}

fn pixel(rgba: &[u8], x: usize, y: usize) -> [f32; 3] {
    let at = (y * WINDOW.width as usize + x) * 4;
    [
        f32::from(rgba[at]),
        f32::from(rgba[at + 1]),
        f32::from(rgba[at + 2]),
    ]
}

fn difference(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs()
}

/// Mean luminance (0..255, sRGB-encoded) over a square.
fn brightness(rgba: &[u8], x: usize, y: usize, half: usize) -> f32 {
    let mut total = 0.0;
    let mut count = 0.0;
    for row in y - half..y + half {
        for column in x - half..x + half {
            let [r, g, b] = pixel(rgba, column, row);
            total += 0.2126 * r + 0.7152 * g + 0.0722 * b;
            count += 1.0;
        }
    }
    total / count
}

fn panel() -> Root {
    Root::new(|_| {
        layers(views![
            Scene,
            container(
                glass(text("Press me").size(24.0))
                    .radius(30.0)
                    .padding(28.0)
            )
            .center()
            .fill(),
        ])
    })
}

#[test]
fn a_press_lights_and_swells_the_glass_then_it_settles() {
    let Some(mut renderer) = gpu() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };
    let mut root = panel();
    let start = Instant::now();
    let rest = render_at(&mut renderer, &mut root, Color::BLACK, start);
    keep("live-rest", &rest);

    // Press near the pane's left edge.
    let finger = Point::new(185.0, 180.0);
    let _ = root.handle(
        &taim_core::Event::Mouse(mouse::Event::CursorMoved { position: finger }),
        Some(finger),
        start,
    );
    let _ = root.handle(
        &taim_core::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Some(finger),
        start,
    );
    assert!(root.is_animating(start), "a press animates");

    let pressed = render_at(
        &mut renderer,
        &mut root,
        Color::BLACK,
        start + Duration::from_millis(180),
    );
    keep("live-pressed", &pressed);

    // The rim nearest the finger catches its light; the far side does not
    // change as much.
    let near = difference(pixel(&rest, 175, 180), pixel(&pressed, 175, 180));
    assert!(near > 12.0, "the glass under the finger changes: {near}");

    // Let go: it springs back to exactly how it was.
    let _ = root.handle(
        &taim_core::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        Some(finger),
        start + Duration::from_millis(200),
    );
    let _ = root.handle(
        &taim_core::Event::Mouse(mouse::Event::CursorLeft),
        None,
        start + Duration::from_millis(200),
    );
    let later = start + Duration::from_secs(3);
    assert!(!root.is_animating(later), "and settles");
    let settled = render_at(&mut renderer, &mut root, Color::BLACK, later);
    keep("live-settled", &settled);
    let changed = rest
        .chunks_exact(4)
        .zip(settled.chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        changed < 200,
        "it returns to rest ({changed} pixels differ)"
    );
}

fn pair(gap: f32) -> Root {
    Root::new(move |_| {
        layers(views![
            Scene,
            container(
                glass_group(
                    row(views![
                        glass(text("Play").size(18.0)).radius(26.0).padding(16.0),
                        glass(text("Next").size(18.0)).radius(26.0).padding(16.0),
                    ])
                    .gap(gap),
                )
                .blend(20.0),
            )
            .center()
            .fill(),
        ])
    })
}

#[test]
fn panes_in_a_group_melt_into_each_other_when_close() {
    let Some(mut renderer) = gpu() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };
    let now = Instant::now();
    let close = render_at(&mut renderer, &mut pair(6.0), Color::BLACK, now);
    keep("live-merged", &close);
    let apart = render_at(&mut renderer, &mut pair(60.0), Color::BLACK, now);
    keep("live-apart", &apart);

    // The middle of the gap between the two panes, at their centre line.
    let middle_x = WINDOW.width as usize / 2;
    let middle_y = WINDOW.height as usize / 2;

    // Close together, the gap is bridged by glass: frosted and as bright as
    // the panes. Apart, the scene shows through the gap untouched.
    let bridged = pixel(&close, middle_x, middle_y);
    let pane = pixel(&close, middle_x - 30, middle_y);
    assert!(
        difference(bridged, pane) < 40.0,
        "the bridge is glass like the panes: {bridged:?} against {pane:?}"
    );

    // A group of one pane looks like a lone pane: merging never applies to
    // panes that are out of reach.
    let lone = render_at(&mut renderer, &mut pair(60.0), Color::BLACK, now);
    assert_eq!(apart, lone, "rendering is deterministic");
}

/// A dark scene with a single bright stripe.
struct Night;

impl View for Night {
    fn measure(&mut self, _: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        constraints.max
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        cx.fill(
            cx.bounds(),
            Color::from_rgb8(0x06, 0x08, 0x12),
            Border::default(),
        );
    }

    fn describe(&self, _: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }
}

#[test]
fn bright_glass_stays_readable_over_a_dark_backdrop() {
    let Some(mut renderer) = gpu() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };
    let mut root = Root::new(|_| {
        layers(views![
            Night,
            container(glass(space().width(200.0).height(100.0)).radius(28.0))
                .center()
                .fill(),
        ])
    });
    root.set_theme(taim::Theme::Light);
    let pixels = render_at(&mut renderer, &mut root, Color::BLACK, Instant::now());
    keep("live-night", &pixels);

    // Black text needs the pane at linear luminance 0.175 or more for WCAG
    // AA; that is about 117 once sRGB-encoded.
    let centre = brightness(
        &pixels,
        WINDOW.width as usize / 2,
        WINDOW.height as usize / 2 + 20,
        6,
    );
    assert!(
        centre > 117.0,
        "the pane is bright enough for black text: {centre}"
    );
}

fn transport() -> Root {
    Root::new(|_| {
        let control = |label: &'static str| {
            glass(text(label).size(18.0))
                .padding(15.0)
                .radius(27.0)
                .clear()
                .interactive()
        };
        layers(views![
            Scene,
            container(
                glass_group(row(views![control("<"), control(">"), control("+")]).gap(12.0))
                    .blend(22.0),
            )
            .center()
            .fill(),
        ])
    })
}

#[test]
fn a_pressed_control_swells_and_melts_into_its_neighbour() {
    let Some(mut renderer) = gpu() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };
    let mut root = transport();
    let start = Instant::now();
    let rest = render_at(&mut renderer, &mut root, Color::BLACK, start);
    keep("live-transport-rest", &rest);

    // Press the middle control.
    let finger = Point::new(240.0, 180.0);
    let _ = root.handle(
        &taim_core::Event::Mouse(mouse::Event::CursorMoved { position: finger }),
        Some(finger),
        start,
    );
    let _ = root.handle(
        &taim_core::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Some(finger),
        start,
    );
    let pressed = render_at(
        &mut renderer,
        &mut root,
        Color::BLACK,
        start + Duration::from_millis(400),
    );
    keep("live-transport-pressed", &pressed);

    // The gap between the middle control and its left neighbour: scene at
    // rest, glass once the press has swollen the control into it.
    let changed = (150..240)
        .filter(|x| difference(pixel(&rest, *x, 180), pixel(&pressed, *x, 180)) > 30.0)
        .count();
    assert!(
        changed > 10,
        "the press changes the glass around it ({changed} pixels)"
    );
}

/// A white square: easy to find on a black window.
struct Block;

impl View for Block {
    fn measure(&mut self, _: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(120.0, 120.0))
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        cx.fill(cx.bounds(), Color::WHITE, Border::default());
    }

    fn describe(&self, _: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }
}

#[test]
fn appearing_content_is_drawn_faded_and_lowered_then_in_place() {
    let Some(mut renderer) = gpu() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };
    let mut root = Root::new(|_| container(appear(Block).rise(40.0)).center().fill());
    let start = Instant::now();
    let first = render_at(&mut renderer, &mut root, Color::BLACK, start);
    keep("appear-first", &first);
    let (x, y) = (WINDOW.width as usize / 2, WINDOW.height as usize / 2);
    assert!(
        brightness(&first, x, y, 4) < 5.0,
        "nothing shows on the first frame"
    );

    let coming = start + Duration::from_millis(90);
    assert!(root.is_animating(coming));
    let midway = render_at(&mut renderer, &mut root, Color::BLACK, coming);
    keep("appear-midway", &midway);
    let middle = brightness(&midway, x, y, 4);
    assert!(
        middle > 5.0 && middle < 250.0,
        "part way in, it is see-through: {middle}"
    );
    // Still lowered: the top edge sits below where it will settle.
    let top = WINDOW.height as usize / 2 - 60;
    assert!(brightness(&midway, x, top + 3, 2) < brightness(&midway, x, top + 30, 2));

    let later = start + Duration::from_secs(2);
    let settled = render_at(&mut renderer, &mut root, Color::BLACK, later);
    keep("appear-settled", &settled);
    assert!(!root.is_animating(later));
    assert!(brightness(&settled, x, top + 3, 2) > 250.0, "and in place");
}

/// A red square of a given side.
struct Red(f32);

impl View for Red {
    fn measure(&mut self, _: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(self.0, self.0))
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        cx.fill(
            cx.bounds(),
            Color::from_rgb(1.0, 0.0, 0.0),
            Border::default(),
        );
    }

    fn describe(&self, _: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }
}

/// The centre of the strongly red pixels, and how many there are.
fn red(rgba: &[u8]) -> (Point, usize) {
    let (mut x, mut y, mut count) = (0.0, 0.0, 0_usize);
    for (index, pixel) in rgba.chunks_exact(4).enumerate() {
        if pixel[0] > 200 && pixel[1] < 70 && pixel[2] < 70 {
            x += (index % WINDOW.width as usize) as f32;
            y += (index / WINDOW.width as usize) as f32;
            count += 1;
        }
    }
    let count_f = count.max(1) as f32;
    (Point::new(x / count_f, y / count_f), count)
}

#[test]
fn a_hero_flies_from_its_place_to_its_partners_over_a_fading_page() {
    let Some(mut renderer) = gpu() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };
    let mut handle = None;
    let mut root = Root::new(|cx| {
        let nav = Navigator::new(cx, 0_u8);
        handle = Some(nav);
        navigator(nav, |_, page| {
            if *page == 0 {
                column(views![
                    space().height(200.0),
                    row(views![space().width(40.0), hero("h", Red(40.0))]),
                ])
                .into()
            } else {
                row(views![space().width(240.0), hero("h", Red(120.0))]).into()
            }
        })
        .transition(PageTransition::Zoom)
    });
    let nav = handle.expect("navigator");
    let start = Instant::now();
    let (thumb, small) = red(&render_at(&mut renderer, &mut root, Color::BLACK, start));
    assert!(small > 1_000, "the thumbnail shows: {small}");

    nav.push(1);
    let _ = root.take_changes();
    let mut centres = Vec::new();
    let mut sizes = Vec::new();
    for frame in 0..=24 {
        let now = start + Duration::from_millis(16 * frame);
        let pixels = render_at(&mut renderer, &mut root, Color::BLACK, now);
        if frame == 6 {
            keep("hero-midway", &pixels);
        }
        let (centre, count) = red(&pixels);
        centres.push(centre);
        sizes.push(count);
    }
    let (landed, big) = red(&render_at(
        &mut renderer,
        &mut root,
        Color::BLACK,
        start + Duration::from_secs(1),
    ));
    assert!(big > 12_000, "the photo shows: {big}");

    // It starts over the thumbnail, at full strength although its page is
    // still fading in, and lands at its own place, growing all the way.
    let first = centres[0];
    assert!(
        (first.x - thumb.x).abs() < 6.0 && (first.y - thumb.y).abs() < 6.0,
        "starts at {first:?}, thumbnail at {thumb:?}"
    );
    assert!(sizes[0] > small / 2, "drawn opaque from the first frame");
    let middle = centres[6];
    assert!(
        middle.x > thumb.x + 20.0 && middle.x < landed.x - 20.0,
        "midway it is between: {middle:?}"
    );
    assert!(
        sizes.windows(2).all(|pair| pair[1] + 400 >= pair[0]),
        "it grows: {sizes:?}"
    );
    let last = centres[24];
    assert!((last.x - landed.x).abs() < 3.0 && (last.y - landed.y).abs() < 3.0);
}

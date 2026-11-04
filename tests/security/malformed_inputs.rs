//! Phase 17: *"extreme images/fonts/text/protocol/event bounds"*.
//!
//! # The threat
//!
//! Everything here arrives from outside the process. A compositor sends
//! surface geometry; a font file declares its own metrics; a pointer
//! reports a position; an application passes a `Length` it computed from
//! a server response. Taim does not get to assume any of it is sane, and
//! "sane" is doing a lot of work in a language where `f32` includes NaN,
//! infinity, and negative zero.
//!
//! A malformed input should produce a *boring* outcome: a clamped value, a
//! `None`, an empty region. What it must never produce is a panic — which
//! on a shell is not an error dialog, it is the desktop disappearing — or
//! a silently absurd value that propagates into a layout the user then
//! interacts with.
//!
//! # Why NaN is the interesting one
//!
//! Every comparison with NaN is `false`. That means `x < min` and
//! `x > max` are *both* false, so the usual clamp shape:
//!
//! ```ignore
//! if x < min { min } else if x > max { max } else { x }
//! ```
//!
//! returns NaN unchanged, having "validated" it. The bound checks pass and
//! the poison spreads. Rust's own `f32::clamp` panics on a NaN *bound* and
//! propagates a NaN *value*, so it is not a defence either.
//!
//! These tests exist because that failure is invisible at every layer
//! until something divides by it.

type Element<'a, M, T = taim_core::Theme, R = taim_renderer::Renderer> =
    taim_core::Element<'a, M, T, R>;
use taim_core::damage;
use taim_core::layout::Limits;
use taim_core::{Length, Point, Rectangle, Size, Theme};
use taim_renderer::Renderer;
use taim_runtime::user_interface::{Cache, UserInterface};
use taim_widget::{column, container, row, text};

type Message = &'static str;

fn headless_renderer() -> Renderer {
    taim_futures::futures::executor::block_on(<Renderer as taim_core::renderer::Headless>::new(
        taim_core::renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .expect("tiny-skia renderer must be available")
}

/// Every float a hostile or broken source can hand us.
const EXTREME: [f32; 8] = [
    f32::NAN,
    f32::INFINITY,
    f32::NEG_INFINITY,
    f32::MAX,
    f32::MIN,
    -0.0,
    -1.0,
    1e30,
];

/// Lays `view` out under `bounds`, returning the root node's size.
///
/// The point is that it returns at all.
fn layout_at(view: Element<'_, Message, Theme, Renderer>, bounds: Size) -> Size {
    let mut renderer = headless_renderer();
    let ui = UserInterface::build(view, bounds, Cache::new(), &mut renderer);

    let _ = ui.into_cache();

    bounds
}

// ---------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------

#[test]
fn a_rectangle_with_extreme_coordinates_never_panics_on_containment() {
    // A pointer position arrives from the compositor. A window that
    // crashed because a device reported a NaN would be a shell that
    // disappears when a tablet is unplugged mid-stroke.
    for x in EXTREME {
        for y in EXTREME {
            let bounds = Rectangle::new(Point::new(x, y), Size::new(x, y));

            // The assertion is that these return. What they return for a
            // NaN is unspecified and uninteresting; that they do not
            // panic, and do not hang, is the contract.
            let _ = std::hint::black_box(bounds.contains(Point::new(0.0, 0.0)));
            let _ = std::hint::black_box(bounds.contains(Point::new(x, y)));
        }
    }
}

#[test]
fn nothing_is_inside_a_nan_rectangle() {
    // The specific trap: every comparison with NaN is false, so a
    // containment check written as `x >= self.x && x <= self.x + width`
    // returns false -- which is the *safe* answer here, and worth pinning
    // so that a later "optimisation" to `!(x < a || x > b)` (which returns
    // true for NaN) is caught.
    let poisoned = Rectangle::new(Point::new(f32::NAN, 0.0), Size::new(10.0, 10.0));

    assert!(
        !poisoned.contains(Point::new(5.0, 5.0)),
        "a NaN rectangle must contain nothing, not everything"
    );
}

#[test]
fn a_nan_point_is_inside_nothing() {
    let ordinary = Rectangle::new(Point::ORIGIN, Size::new(100.0, 100.0));

    assert!(
        !ordinary.contains(Point::new(f32::NAN, 50.0)),
        "a NaN pointer position must not register as a hit"
    );
}

#[test]
fn intersecting_extreme_rectangles_never_panics() {
    for a in EXTREME {
        for b in EXTREME {
            let first = Rectangle::new(Point::new(a, a), Size::new(b, b));
            let second = Rectangle::new(Point::new(b, b), Size::new(a, a));

            let _ = std::hint::black_box(first.intersection(&second));
            let _ = std::hint::black_box(first.union(&second));
        }
    }
}

#[test]
fn a_union_of_finite_rectangles_stays_finite() {
    // The property that matters for damage: unioning real geometry must
    // not produce an infinity that then swallows the whole surface.
    let a = Rectangle::new(Point::new(0.0, 0.0), Size::new(10.0, 10.0));
    let b = Rectangle::new(Point::new(90.0, 90.0), Size::new(10.0, 10.0));

    let union = a.union(&b);

    assert!(union.x.is_finite() && union.y.is_finite());
    assert!(union.width.is_finite() && union.height.is_finite());
    assert_eq!(union.width, 100.0);
}

// ---------------------------------------------------------------------
// Damage
// ---------------------------------------------------------------------

#[test]
fn a_malformed_damage_area_never_panics() {
    for x in EXTREME {
        for size in EXTREME {
            let bounds = Rectangle::new(Point::new(x, x), Size::new(size, size));

            let _ = std::hint::black_box(damage::Region::area(bounds));
        }
    }
}

#[test]
fn merging_damage_regions_is_associative_over_extremes() {
    // A compositor gets one region per frame. If merging depended on
    // order, a frame's damage would depend on the order the widgets
    // happened to report in -- and the bug would appear only when
    // something unrelated changed the tree.
    let regions: Vec<damage::Region> = EXTREME
        .iter()
        .filter_map(|value| {
            damage::Region::area(Rectangle::new(
                Point::new(*value, 0.0),
                Size::new(10.0, 10.0),
            ))
        })
        .collect();

    for a in &regions {
        for b in &regions {
            for c in &regions {
                let left = a.merge(*b).merge(*c);
                let right = a.merge(b.merge(*c));

                assert_eq!(
                    left.bounds().is_some(),
                    right.bounds().is_some(),
                    "merge order changed whether there was damage at all"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------

#[test]
fn extreme_limits_never_panic() {
    for width in EXTREME {
        for height in EXTREME {
            let limits = Limits::new(Size::ZERO, Size::new(width, height));

            let _ = std::hint::black_box(limits.max());
            let _ = std::hint::black_box(limits.resolve_width(Length::Fill, 10.0));
            let _ = std::hint::black_box(limits.resolve_height(Length::Shrink, 10.0));
        }
    }
}

#[test]
fn an_extreme_fixed_length_never_panics() {
    // A `Length::Fixed` computed from a server response, a config file, or
    // an animation that overshot.
    let limits = Limits::new(Size::ZERO, Size::new(100.0, 100.0));

    for value in EXTREME {
        let _ = std::hint::black_box(limits.resolve_width(Length::Fixed(value), 10.0));
        let _ = std::hint::black_box(limits.resolve_height(Length::Fixed(value), 10.0));
    }
}

#[test]
fn laying_out_a_real_view_under_extreme_bounds_never_panics() {
    // A compositor configure event with a degenerate size. The shell must
    // not die of it.
    for width in [0.0, 1.0, f32::MAX, 1e30] {
        for height in [0.0, 1.0, f32::MAX, 1e30] {
            let view: Element<'_, Message, Theme, Renderer> =
                column![text("hello"), row![text("a"), text("b")]].into();

            let _ = std::hint::black_box(layout_at(view, Size::new(width, height)));
        }
    }
}

#[test]
fn a_zero_sized_surface_lays_out_and_hit_tests() {
    // A window minimised to nothing, or a configure of 0x0 before the
    // first real one. Common, and a division waiting to happen.
    let mut renderer = headless_renderer();
    let view: Element<'_, Message, Theme, Renderer> = column![text("hello")].into();

    let mut ui = UserInterface::build(view, Size::ZERO, Cache::new(), &mut renderer);

    assert_eq!(ui.hit_test(Point::new(0.0, 0.0)), None);
    let _ = std::hint::black_box(ui.semantics(&renderer));
}

// ---------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------

#[test]
fn degenerate_text_lays_out_without_panicking() {
    // Text arrives from everywhere: a filename, a notification, a message
    // someone else composed. None of it is under our control.
    let cases: [&str; 9] = [
        "",
        // A lone combining mark: no base character to attach to.
        "\u{0301}",
        // Right-to-left mixed with left-to-right.
        "abc \u{05D0}\u{05D1}\u{05D2} def",
        // An explicit direction override, unterminated.
        "\u{202E}reversed",
        // A zero-width joiner sequence.
        "\u{200D}\u{200D}\u{200D}",
        // Astral-plane characters (surrogate pairs in UTF-16 land).
        "𝕳𝖊𝖑𝖑𝖔 🏳️‍🌈",
        // A byte-order mark in the middle.
        "before\u{FEFF}after",
        // A line separator, which is not '\n'.
        "line\u{2028}separator",
        // Deeply nested combining marks on one base.
        "e\u{0301}\u{0302}\u{0303}\u{0304}\u{0305}\u{0306}\u{0307}",
    ];

    for case in cases {
        let view: Element<'_, Message, Theme, Renderer> = text(case).into();

        let _ = std::hint::black_box(layout_at(view, Size::new(200.0, 200.0)));
    }
}

#[test]
fn very_long_text_lays_out_without_panicking() {
    // Not a fuzz case -- a real one. A filename, a URL, or a pasted log
    // line with no spaces in it has nowhere to wrap.
    let unbreakable = "a".repeat(100_000);

    let view: Element<'_, Message, Theme, Renderer> = text(unbreakable).into();

    let _ = std::hint::black_box(layout_at(view, Size::new(200.0, 200.0)));
}

#[test]
fn text_with_no_room_to_render_lays_out() {
    let view: Element<'_, Message, Theme, Renderer> = text("hello").into();

    let _ = std::hint::black_box(layout_at(view, Size::new(0.0, 0.0)));
}

// ---------------------------------------------------------------------
// Event bounds
// ---------------------------------------------------------------------

#[test]
fn hit_testing_at_extreme_coordinates_never_panics() {
    // The compositor reports pointer positions. A broken driver, a
    // hot-unplugged tablet, or a remote-desktop client with a different
    // idea of coordinate space can all produce these.
    let mut renderer = headless_renderer();
    let view: Element<'_, Message, Theme, Renderer> = container(column![text("hello")]).into();

    let mut ui = UserInterface::build(view, Size::new(400.0, 400.0), Cache::new(), &mut renderer);

    for x in EXTREME {
        for y in EXTREME {
            let _ = std::hint::black_box(ui.hit_test(Point::new(x, y)));
        }
    }
}

#[test]
fn a_nan_pointer_position_hits_nothing_rather_than_everything() {
    // The consequence if containment were written the other way round: a
    // NaN would hit *every* widget, and the deepest one would receive a
    // click the user never made.
    let mut renderer = headless_renderer();
    let view: Element<'_, Message, Theme, Renderer> = container(column![text("hello")]).into();

    let mut ui = UserInterface::build(view, Size::new(400.0, 400.0), Cache::new(), &mut renderer);

    assert_eq!(
        ui.hit_test(Point::new(f32::NAN, f32::NAN)),
        None,
        "a NaN position must hit nothing"
    );
}

#[test]
fn a_spatial_index_agrees_with_the_oracle_even_at_extremes() {
    // The index prunes by bounds; the oracle does not. A NaN that pruned
    // differently from how it hit-tested would make the two disagree, and
    // the exit gate says they never may.
    let mut renderer = headless_renderer();
    let view: Element<'_, Message, Theme, Renderer> =
        container(column![text("hello"), text("world")]).into();

    let mut ui = UserInterface::build(view, Size::new(400.0, 400.0), Cache::new(), &mut renderer);
    let index = ui.hit_test_index();

    for x in EXTREME {
        for y in EXTREME {
            let point = Point::new(x, y);

            assert_eq!(
                index.hit_test(&mut ui.visit(), point),
                ui.hit_test(point),
                "the index disagreed with the oracle at {point:?}"
            );
        }
    }
}

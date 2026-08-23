//! Pixel goldens for the shipped widget set, in both themes.
//!
//! # Why this exists
//!
//! Before it, nothing in this repository could detect a visual regression.
//! The harness was written, well tested, and never called: `compare` had
//! zero callers outside its own unit tests, and `tests/golden/images/` did
//! not exist. A change that rendered every button as a black rectangle
//! passed the entire suite.
//!
//! That was not hypothetical. `graphics/fonts/Aura-Icons.ttf` declared the
//! family `Iced-Icons` while both renderers asked for `Aura-Icons`, so every
//! checkmark, dropdown arrow, and scrollbar arrow in the framework resolved
//! to a missing glyph -- for as long as the fork has existed, through 1,705
//! passing tests, because not one of them looked at a pixel.
//!
//! # Determinism
//!
//! Rendered by `tiny-skia` on the CPU: no GPU, no driver, no Vulkan
//! implementation. A golden recorded here is reproducible on any machine
//! that can run the test, which a GPU backend -- llvmpipe included -- could
//! not promise. Fonts are pinned by the bundled Fira Sans, so text does not
//! depend on what the host happens to have installed.
//!
//! # Adopting a golden
//!
//! ```bash
//! AURA_UPDATE_GOLDEN=1 cargo test --test golden
//! ```
//!
//! Deliberately awkward. Read `harness`'s module docs before doing it.

use super::harness::{Harness, Image, Tolerance};

use aura::widget::icon::{self, Icon};
use aura::widget::segmented_control::Segment;
use aura::widget::tabs::Tab;
use aura::widget::{
    accordion, avatar, badge, button, checkbox, chip, column, container, progress_bar, radio, row,
    search_bar, segmented_control, skeleton, slider, stepper, tabs, text, text_input, toggler,
};
use aura::{Center, Element, Size, Theme};

/// No message is ever produced: these are pictures, not interactions.
type Case<'a> = Element<'a, ()>;

/// Both themes every case is rendered in.
const THEMES: [(&str, Theme); 2] = [("light", Theme::Light), ("dark", Theme::Dark)];

/// Renders one case, refuses a blank, and compares it against its golden.
///
/// The blank check is the important half. A widget that renders nothing
/// produces a uniform image, which is a perfectly plausible-looking golden
/// and would be adopted without complaint -- and a harness whose failure mode
/// is "silently pin an empty picture" is worse than no harness. Every case
/// here therefore has to prove it drew something before it is allowed to be
/// compared at all.
fn check(name: &str, size: (u32, u32), tolerance: Tolerance, build: impl Fn() -> Case<'static>) {
    let harness = Harness::default_directory()
        .with_updates(std::env::var(super::harness::UPDATE_ENV).is_ok());

    for (suffix, theme) in THEMES {
        let case = format!("{name}_{suffix}");

        let image = Harness::render_element(Size::new(size.0, size.1), 1.0, &theme, build())
            .expect("the tiny-skia renderer must be available");

        assert!(
            ink(&image) > 0,
            "{case}: rendered a uniform image -- the widget drew nothing. \
             Adopting this as a golden would pin a blank."
        );

        let outcome = harness.compare(&case, &image, tolerance);

        if outcome.is_failure() {
            // Write the artifact before failing, so CI has something to
            // upload and a reviewer has something to look at.
            harness.write_diff(&case, &image);
        }

        assert!(!outcome.is_failure(), "{}", outcome.report(&case));
    }
}

/// Pixels differing from the background, i.e. ink actually drawn.
fn ink(image: &Image) -> usize {
    let background = &image.rgba[0..4];

    image
        .rgba
        .chunks_exact(4)
        .filter(|pixel| {
            pixel[..3]
                .iter()
                .zip(&background[..3])
                .any(|(a, b)| a.abs_diff(*b) > 8)
        })
        .count()
}

// ── Controls ──────────────────────────────────────────────────────────

#[test]
fn button_enabled() {
    check("button_enabled", (160, 56), Tolerance::TEXT, || {
        button(text("Save")).on_press(()).into()
    });
}

#[test]
fn button_disabled() {
    // No `on_press`: the disabled appearance is the whole point, and it is
    // the state a catalogue is most likely to forget.
    check("button_disabled", (160, 56), Tolerance::TEXT, || {
        button(text("Save")).into()
    });
}

#[test]
fn checkbox_states() {
    check("checkbox_states", (200, 72), Tolerance::TEXT, || {
        column![
            checkbox(false).label("Off").on_toggle(|_| ()),
            checkbox(true).label("On").on_toggle(|_| ()),
        ]
        .spacing(8)
        .into()
    });
}

#[test]
fn radio_states() {
    check("radio_states", (200, 72), Tolerance::TEXT, || {
        column![
            radio("Off", 0u8, Some(1u8), |_| ()),
            radio("On", 1u8, Some(1u8), |_| ()),
        ]
        .spacing(8)
        .into()
    });
}

#[test]
fn toggler_states() {
    check("toggler_states", (200, 80), Tolerance::TEXT, || {
        column![
            toggler(false).label("Off").on_toggle(|_| ()),
            toggler(true).label("On").on_toggle(|_| ()),
        ]
        .spacing(10)
        .into()
    });
}

#[test]
fn slider_and_progress() {
    check("slider_and_progress", (240, 72), Tolerance::TEXT, || {
        column![
            slider(0.0..=1.0, 0.4f32, |_| ()).step(0.01),
            progress_bar(0.0..=1.0, 0.7f32),
        ]
        .spacing(16)
        .into()
    });
}

#[test]
fn stepper_at_rest() {
    check("stepper_at_rest", (220, 64), Tolerance::TEXT, || {
        stepper(3i32)
            .range(1..=5)
            .label("Quantity")
            .on_change(|_| ())
            .into()
    });
}

#[test]
fn segmented_control_selected() {
    check("segmented_control", (280, 56), Tolerance::TEXT, || {
        segmented_control(
            ["List", "Grid", "Compact"]
                .into_iter()
                .map(|mode| Segment::new(text(mode).size(13), mode).on_press(())),
        )
        .selected(1)
        .into()
    });
}

#[test]
fn chips_selected_and_not() {
    check("chips", (280, 56), Tolerance::TEXT, || {
        row![
            chip(text("Unread").size(13)).selected(true).on_press(()),
            chip(text("Starred").size(13)).on_press(()),
        ]
        .spacing(8)
        .into()
    });
}

// ── Text entry ────────────────────────────────────────────────────────

#[test]
fn text_input_with_value() {
    check("text_input_value", (280, 56), Tolerance::TEXT, || {
        text_input("Placeholder", "Typed text")
            .on_input(|_| ())
            .into()
    });
}

#[test]
fn text_input_placeholder() {
    check("text_input_placeholder", (280, 56), Tolerance::TEXT, || {
        text_input("Placeholder", "").on_input(|_| ()).into()
    });
}

#[test]
fn search_bar_empty() {
    check("search_bar", (280, 56), Tolerance::TEXT, || {
        search_bar("Search", "").on_input(|_| ()).into()
    });
}

// ── Collections and surfaces ──────────────────────────────────────────

#[test]
fn tabs_selected() {
    check("tabs", (300, 56), Tolerance::TEXT, || {
        tabs(
            ["Overview", "Activity", "Settings"]
                .into_iter()
                .map(|section| Tab::new(text(section).size(14), section).on_press(())),
        )
        .selected(0)
        .into()
    });
}

#[test]
fn accordion_open_and_closed() {
    check("accordion", (280, 140), Tolerance::TEXT, || {
        accordion([
            accordion::Section::new("Open", text("Body text").size(13)),
            accordion::Section::new("Closed", text("Hidden").size(13)),
        ])
        .expanded(Some(0))
        .on_toggle(|_| ())
        .into()
    });
}

#[test]
fn avatar_and_badge() {
    check("avatar_and_badge", (240, 64), Tolerance::TEXT, || {
        row![
            avatar("Ada Lovelace").size(36),
            badge(text("Inbox").size(14)).count(3).label("unread"),
        ]
        .spacing(12)
        .align_y(Center)
        .into()
    });
}

#[test]
fn skeleton_still() {
    // `still()` so the golden does not depend on where an animation happens
    // to be when the frame is taken.
    check("skeleton_still", (240, 64), Tolerance::Exact, || {
        column![
            skeleton().width(200).height(16).still(),
            skeleton().width(160).height(16).still(),
        ]
        .spacing(10)
        .into()
    });
}

#[test]
fn container_surface() {
    check("container_surface", (220, 80), Tolerance::TEXT, || {
        container(text("Card content").size(14)).padding(16).into()
    });
}

// ── Typography and icons ──────────────────────────────────────────────

#[test]
fn text_sizes() {
    check("text_sizes", (240, 120), Tolerance::TEXT, || {
        column![
            text("Heading").size(24),
            text("Body copy").size(16),
            text("Caption").size(12),
        ]
        .spacing(8)
        .into()
    });
}

#[test]
fn icon_row() {
    // The case that would have caught the `Iced-Icons` family mismatch.
    check("icon_row", (240, 56), Tolerance::TEXT, || {
        row![
            icon::sized(Icon::SEARCH, 28.0),
            icon::sized(Icon::HOME, 28.0),
            icon::sized(Icon::SETTINGS, 28.0),
            icon::sized(Icon::STAR, 28.0),
            icon::sized(Icon::CHECK, 28.0),
        ]
        .spacing(8)
        .align_y(Center)
        .into()
    });
}

#[test]
fn icon_sizes() {
    check("icon_sizes", (200, 56), Tolerance::TEXT, || {
        row![
            icon::sized(Icon::HEART, 16.0),
            icon::sized(Icon::HEART, 24.0),
            icon::sized(Icon::HEART, 32.0),
        ]
        .spacing(8)
        .align_y(Center)
        .into()
    });
}

// ── The mobile set ────────────────────────────────────────────────────
//
// These are the widgets built on top of the fork -- the ones the README
// leads with, and the ones with the least prior coverage of any kind. The
// accordion's blank headers and the search bar's plus-shaped "clear" both
// lived here, unseen, until something rendered them.

#[test]
fn app_bar_with_title() {
    check("app_bar", (300, 72), Tolerance::TEXT, || {
        aura::widget::app_bar(text("Inbox").size(18)).into()
    });
}

#[test]
fn bottom_nav_bar_selected() {
    use aura::widget::bottom_nav_bar::Item;

    check("bottom_nav_bar", (300, 80), Tolerance::TEXT, || {
        aura::widget::bottom_nav_bar(
            [
                (Icon::HOME, "Home"),
                (Icon::SEARCH, "Search"),
                (Icon::PERSON, "Profile"),
            ]
            .into_iter()
            .map(|(glyph, label)| {
                Item::new(
                    column![icon::sized(glyph, 20.0), text(label).size(11)]
                        .spacing(2)
                        .align_x(Center),
                    label,
                )
                .on_press(())
            }),
        )
        .selected(0)
        .into()
    });
}

#[test]
fn list_tile_row() {
    check("list_tile", (300, 72), Tolerance::TEXT, || {
        aura::widget::list_tile(text("Ada Lovelace").size(15))
            .on_press(())
            .into()
    });
}

#[test]
fn snackbar_visible() {
    check("snackbar", (300, 96), Tolerance::TEXT, || {
        aura::widget::snackbar(text("Message sent").size(14), true).into()
    });
}

#[test]
fn carousel_dots() {
    check("carousel", (280, 120), Tolerance::TEXT, || {
        aura::widget::carousel((0..3).map(|page| {
            container(text(format!("Page {page}")).size(15))
                .padding(20)
                .into()
        }))
        .into()
    });
}

#[test]
fn icon_button_glyph() {
    check("icon_button", (72, 72), Tolerance::TEXT, || {
        aura::widget::icon_button(Icon::SETTINGS.codepoint(), "Settings")
            .font(icon::FONT)
            .on_press(())
            .into()
    });
}

#[test]
fn date_picker_month() {
    use aura::widget::date::Date;

    check("date_picker", (320, 300), Tolerance::TEXT, || {
        aura::widget::date_picker(Date::new(2026, 8, 1))
            .selected(Date::new(2026, 8, 20))
            .today(Date::new(2026, 8, 13))
            .on_select(|_| ())
            .into()
    });
}

#[test]
fn time_picker_columns() {
    use aura::widget::time_picker::Time;

    check("time_picker", (240, 200), Tolerance::TEXT, || {
        aura::widget::time_picker(Time::new(9, 30))
            .minute_step(15)
            .on_change(|_| ())
            .into()
    });
}

#[test]
fn badge_dot_and_count() {
    check("badge_variants", (280, 96), Tolerance::TEXT, || {
        container(
            row![
                badge(text("Inbox").size(14)).count(3).label("unread"),
                badge(text("Alerts").size(14)).dot().label("new"),
            ]
            .spacing(24)
            .align_y(Center),
        )
        .padding(20)
        .into()
    });
}

#[test]
fn pull_to_refresh_idle() {
    check("pull_to_refresh", (280, 120), Tolerance::TEXT, || {
        aura::widget::pull_to_refresh(container(text("Pull me").size(14)).padding(20), false)
            .on_refresh(())
            .into()
    });
}

#[test]
fn stepper_at_its_limit() {
    // The disabled arm is the state a catalogue is most likely to forget,
    // and the one a user hits first.
    check("stepper_at_limit", (220, 64), Tolerance::TEXT, || {
        stepper(5i32)
            .range(1..=5)
            .label("Quantity")
            .on_change(|_| ())
            .into()
    });
}

#[test]
fn checkbox_disabled() {
    check("checkbox_disabled", (200, 48), Tolerance::TEXT, || {
        checkbox(true).label("Locked").into()
    });
}

#[test]
fn toggler_disabled() {
    check("toggler_disabled", (200, 48), Tolerance::TEXT, || {
        toggler(true).label("Locked").into()
    });
}

#[test]
fn progress_extremes() {
    // Empty and full: both are easy to render as the other by an off-by-one
    // in a fill width, and neither is visible in a mid-value golden.
    check("progress_extremes", (240, 72), Tolerance::Exact, || {
        column![
            progress_bar(0.0..=1.0, 0.0f32),
            progress_bar(0.0..=1.0, 1.0f32),
        ]
        .spacing(16)
        .into()
    });
}

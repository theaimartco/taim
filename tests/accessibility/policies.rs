//! The Phase 13 exit-gate clause: **large text and high contrast remain
//! usable on all device profiles**, plus the reduced-motion and
//! touch-target policies the component contract owes.
//!
//! These are the accessibility *policies* rather than the semantic
//! plumbing: they ask whether the interface is actually usable by someone
//! who needs a bigger font, more contrast, less motion, or a larger
//! target -- across every profile, not just the designer's.
//!
//! # What "all device profiles" means here
//!
//! `ViewportClass::{Compact, Medium, Expanded}` x `Density::{Compact,
//! Comfortable, Spacious}` -- phone, tablet, and desktop at three
//! densities. There is **no watch profile** in the system; that gap is
//! recorded in `docs/accessibility/contract.md` rather than papered over.

use taim_design::components::button::{Button, ButtonKind};
use taim_design::components::card::Card;
use taim_design::components::icon_button::IconButton;
use taim_design::components::list_tile::{ListLines, ListTile};
use taim_design::components::menu::Menu;
use taim_design::components::navigation::{Navigation, NavigationMode};
use taim_design::components::progress::Progress;
use taim_design::components::selection::{Selection, SelectionKind};
use taim_design::components::slider::Slider;
use taim_design::components::text_field::TextField;
use taim_design::components::{ComponentSpec, Density, MIN_TOUCH_TARGET, ViewportClass};
use taim_design::icon::Icon;
use taim_design::state::Interaction;
use taim_design::theme::Theme;
use taim_design::tokens::{Contrast, Scheme, TextStyle, Typography};

use taim_motion::{Effect, MotionPolicy};

use taim_core::semantics::Actions;
use taim_core::text::Script;

const EVERY_SCHEME: [Scheme; 3] = [Scheme::Light, Scheme::Dark, Scheme::HighContrast];

/// Every interactive component: what the touch-target and contrast
/// policies apply to.
fn every_control() -> Vec<(&'static str, Box<dyn ComponentSpec>)> {
    vec![
        (
            "button",
            Box::new(Button {
                kind: ButtonKind::Filled,
                loading: false,
                disabled: false,
            }),
        ),
        (
            "icon_button",
            Box::new(IconButton {
                icon: Icon::new("close", 24, false),
                tooltip: Some("Close"),
                disabled: false,
            }),
        ),
        (
            "selection",
            Box::new(Selection {
                kind: SelectionKind::Checkbox,
                checked: false,
                disabled: false,
            }),
        ),
        (
            "slider",
            Box::new(Slider {
                value: 0.5,
                range: (0.0, 1.0),
                rtl: false,
                disabled: false,
            }),
        ),
        (
            "text_field",
            Box::new(TextField {
                secure: false,
                multiline: false,
                error: false,
                disabled: false,
            }),
        ),
        (
            "menu",
            Box::new(Menu {
                expanded: false,
                has_submenu: true,
            }),
        ),
        (
            "navigation",
            Box::new(Navigation {
                mode: NavigationMode::Rail,
                selected: false,
            }),
        ),
        (
            "card",
            Box::new(Card {
                interactive: true,
                selected: false,
            }),
        ),
        (
            "list_tile",
            Box::new(ListTile {
                lines: ListLines::Two,
                interactive: true,
            }),
        ),
    ]
}

// ---------------------------------------------------------------------
// Touch targets, on every profile
// ---------------------------------------------------------------------

#[test]
fn every_control_meets_the_touch_target_at_every_density() {
    // The smallest density is the dangerous one: it is where a designer
    // squeezing a UI would break the minimum first.
    let mut checked = 0;

    for (name, spec) in every_control() {
        for density in Density::ALL {
            let size = spec.min_size(density);

            if size.width > 0.0 {
                assert!(
                    size.width >= MIN_TOUCH_TARGET,
                    "{name}/{density:?}: width {} is below the {MIN_TOUCH_TARGET}px minimum",
                    size.width
                );
            }
            assert!(
                size.height >= MIN_TOUCH_TARGET,
                "{name}/{density:?}: height {} is below the {MIN_TOUCH_TARGET}px minimum",
                size.height
            );

            checked += 1;
        }
    }

    assert_eq!(checked, every_control().len() * Density::ALL.len());
}

#[test]
fn the_compact_density_does_not_shrink_below_the_minimum() {
    // Compact is a density, not an excuse: it may tighten spacing but not
    // the target itself.
    for (name, spec) in every_control() {
        let compact = spec.min_size(Density::Compact);

        assert!(
            compact.height >= MIN_TOUCH_TARGET,
            "{name}: Compact density broke the touch target"
        );
    }
}

// ---------------------------------------------------------------------
// Contrast, on every scheme
// ---------------------------------------------------------------------

#[test]
fn every_control_is_readable_in_every_scheme_and_state() {
    // High contrast must not merely exist: every control has to pass in
    // it, in every state a user can put the control into.
    let states = [
        ("default", Interaction::default()),
        (
            "hover",
            Interaction {
                hovered: true,
                ..Interaction::default()
            },
        ),
        (
            "focus",
            Interaction {
                focus_visible: true,
                ..Interaction::default()
            },
        ),
        (
            "selected",
            Interaction {
                selected: true,
                ..Interaction::default()
            },
        ),
    ];

    for scheme in EVERY_SCHEME {
        let theme = Theme::new(scheme);

        for (name, spec) in every_control() {
            for (state, interaction) in states {
                let appearance = spec.appearance(interaction, &theme);
                let background = appearance.background.unwrap_or(theme.colors().surface);

                assert!(
                    Contrast::body_text(appearance.foreground, background),
                    "{name}/{state}/{scheme:?}: fails body contrast (ratio {:.2})",
                    Contrast::ratio(appearance.foreground, background)
                );
            }
        }
    }
}

#[test]
fn high_contrast_never_degrades_any_control_below_the_ordinary_schemes() {
    // The point of a high-contrast scheme is that it is *more* legible. A
    // control that reads worse there would be a regression aimed at the
    // exact users it is for.
    let high = Theme::new(Scheme::HighContrast);

    for (name, spec) in every_control() {
        let high_appearance = spec.appearance(Interaction::default(), &high);
        let high_ratio = Contrast::ratio(
            high_appearance.foreground,
            high_appearance.background.unwrap_or(high.colors().surface),
        );

        for scheme in [Scheme::Light, Scheme::Dark] {
            let theme = Theme::new(scheme);
            let appearance = spec.appearance(Interaction::default(), &theme);
            let ratio = Contrast::ratio(
                appearance.foreground,
                appearance.background.unwrap_or(theme.colors().surface),
            );

            assert!(
                high_ratio >= ratio,
                "{name}: HighContrast ({high_ratio:.2}) is less legible than {scheme:?} ({ratio:.2})"
            );
        }
    }
}

#[test]
fn the_focus_ring_is_visible_against_every_scheme() {
    // A focus ring that blends into the surface leaves a keyboard user
    // with no idea where they are. 3.0 is the non-text contrast bar.
    for scheme in EVERY_SCHEME {
        let theme = Theme::new(scheme);
        let colors = theme.colors();

        let ratio = Contrast::ratio(colors.focus, colors.surface);

        assert!(
            ratio >= 3.0,
            "{scheme:?}: the focus ring scores {ratio:.2} against the surface, \
             below the 3.0 non-text contrast bar"
        );
    }
}

// ---------------------------------------------------------------------
// Large text
// ---------------------------------------------------------------------

#[test]
fn every_component_declares_that_it_reflows_for_large_text() {
    for (name, spec) in every_control() {
        assert!(
            spec.supports_large_text(),
            "{name}: must reflow rather than clip when text is enlarged"
        );
    }
}

#[test]
fn line_metrics_leave_room_for_scripts_that_need_more_than_latin() {
    // "Large text remains usable" is not only about size: a script with
    // tall marks and deep descenders (Devanagari, Arabic) clips at a
    // Latin-derived line height, and the larger the text, the worse it
    // clips.
    let latin = Typography::style(TextStyle::Body, Script::Latin).line_height;

    for script in [
        Script::Devanagari,
        Script::Arabic,
        Script::Bengali,
        Script::Tamil,
        Script::Gurmukhi,
    ] {
        let line_height = Typography::style(TextStyle::Body, script).line_height;

        assert!(
            line_height > latin,
            "{script:?}: line height {line_height} is not taller than Latin's {latin}, \
             which clips its marks"
        );
    }
}

#[test]
fn the_type_ramp_is_monotonic_so_hierarchy_survives_scaling() {
    // If the ramp were not ordered, scaling text would invert the
    // hierarchy: a body line could end up larger than the heading above
    // it.
    let ramp = [
        TextStyle::Label,
        TextStyle::BodySmall,
        TextStyle::Body,
        TextStyle::Title,
        TextStyle::Headline,
    ];

    for pair in ramp.windows(2) {
        let smaller = Typography::style(pair[0], Script::Latin);
        let larger = Typography::style(pair[1], Script::Latin);

        assert!(
            larger.size > smaller.size,
            "{:?} ({}) must be larger than {:?} ({})",
            pair[1],
            larger.size,
            pair[0],
            smaller.size
        );
        assert!(
            larger.line_height > smaller.line_height,
            "{:?} must also lead more than {:?}",
            pair[1],
            pair[0]
        );
    }
}

#[test]
fn every_style_leads_more_than_its_own_size() {
    // Line height below font size overlaps consecutive lines, which is
    // unreadable at any scale and catastrophic at a large one.
    for style in [
        TextStyle::Label,
        TextStyle::BodySmall,
        TextStyle::Body,
        TextStyle::Title,
        TextStyle::Headline,
    ] {
        for script in [Script::Latin, Script::Devanagari, Script::Arabic] {
            let typography = Typography::style(style, script);

            assert!(
                typography.line_height > typography.size,
                "{style:?}/{script:?}: line height {} does not clear size {}",
                typography.line_height,
                typography.size
            );
        }
    }
}

// ---------------------------------------------------------------------
// Reduced motion
// ---------------------------------------------------------------------

#[test]
fn reduced_motion_substitutes_rather_than_deleting_the_cue() {
    // "Reduced motion is a semantic alternative, not simply a global
    // zero-duration switch": a user who asked for less motion still needs
    // to know something happened.
    assert_eq!(
        MotionPolicy::Reduced.effect(Effect::Transform),
        Effect::Fade,
        "spatial motion must become a fade, not nothing"
    );

    let duration = MotionPolicy::Reduced.duration(Effect::Fade, 0.5);
    assert!(
        duration > 0.0,
        "the substituted fade must still be visible, not instant"
    );
    assert!(
        duration < 0.5,
        "...but shorter than the authored motion it replaced"
    );
}

#[test]
fn reduced_motion_leaves_a_genuinely_instant_change_instant() {
    // Not everything is motion: a state change with no interpolation must
    // not acquire one just because a policy is active.
    assert_eq!(MotionPolicy::Reduced.duration(Effect::Instant, 0.5), 0.0);
    assert_eq!(MotionPolicy::Full.duration(Effect::Instant, 0.5), 0.0);
}

#[test]
fn an_indeterminate_progress_indicator_keeps_saying_busy_under_reduced_motion() {
    // Freezing a spinner reads as "stalled", which is worse than the
    // motion it was trying to avoid.
    let indeterminate = Progress {
        value: None,
        circular: true,
    };

    assert_eq!(
        indeterminate.respects_reduced_motion(MotionPolicy::Reduced),
        Effect::Fade,
        "a busy indicator must keep communicating, via a non-spatial cue"
    );
}

#[test]
fn full_motion_is_unaffected_by_the_reduced_motion_machinery() {
    // The default path must be untouched: a user who asked for nothing
    // gets the authored motion exactly.
    assert_eq!(
        MotionPolicy::Full.effect(Effect::Transform),
        Effect::Transform
    );
    assert_eq!(MotionPolicy::Full.duration(Effect::Transform, 0.42), 0.42);
}

// ---------------------------------------------------------------------
// Adaptive layout across profiles
// ---------------------------------------------------------------------

#[test]
fn every_control_lays_out_on_every_viewport_and_density_profile() {
    // The "all device profiles" sweep: phone/tablet/desktop x three
    // densities. Nothing may collapse, invert, or produce a nonsense
    // measurement on any of them.
    for (name, spec) in every_control() {
        for viewport in ViewportClass::ALL {
            for density in Density::ALL {
                let layout = spec.layout(viewport, density);

                assert!(
                    layout.padding >= 0.0 && layout.padding.is_finite(),
                    "{name}/{viewport:?}/{density:?}: nonsense padding"
                );
                assert!(
                    layout.gap >= 0.0 && layout.gap.is_finite(),
                    "{name}/{viewport:?}/{density:?}: nonsense gap"
                );
                assert!(
                    layout.min_size.height >= MIN_TOUCH_TARGET,
                    "{name}/{viewport:?}/{density:?}: layout broke the touch target"
                );
            }
        }
    }
}

#[test]
fn navigation_stays_reachable_on_every_viewport() {
    // The one component whose whole job is adapting: it must remain
    // focusable and touch-sized in every form it takes.
    for viewport in ViewportClass::ALL {
        let nav = Navigation::for_viewport(viewport);

        assert!(
            nav.actions().contains(Actions::FOCUS),
            "{viewport:?}: a navigation destination must stay focusable"
        );

        for density in Density::ALL {
            let size = nav.min_size(density);
            assert!(
                size.height >= MIN_TOUCH_TARGET && size.width >= MIN_TOUCH_TARGET,
                "{viewport:?}/{density:?}: navigation broke its touch target"
            );
        }
    }
}

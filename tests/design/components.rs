//! The production component state matrix.
//!
//! `phase.md` Phase 11's component completion contract requires that
//! *every* component define and test its behavior across default, hover,
//! focus-visible, pressed, selected, disabled, loading, error,
//! high-contrast, RTL, large-text, and reduced-motion conditions, and
//! declare its semantics, navigation, touch target, focus ring, adaptive
//! layout, token-only styling, and readability without effects.
//!
//! Rather than 18 near-identical hand-written modules, this drives every
//! component through the whole matrix generically, so a component cannot
//! quietly skip part of the contract and a newly added one is covered the
//! moment it joins [`every_component`].

use aura_design::components::app_bar::{AppBar, AppBarKind};
use aura_design::components::bottom_sheet::{Anchor, BottomSheet};
use aura_design::components::button::{Button, ButtonKind};
use aura_design::components::card::Card;
use aura_design::components::dialog::Dialog;
use aura_design::components::icon_button::IconButton;
use aura_design::components::list_tile::{ListLines, ListTile};
use aura_design::components::menu::Menu;
use aura_design::components::navigation::{Navigation, NavigationMode};
use aura_design::components::notification::{Notification, Privacy};
use aura_design::components::progress::Progress;
use aura_design::components::selection::{Selection, SelectionKind};
use aura_design::components::slider::Slider;
use aura_design::components::text_field::TextField;
use aura_design::components::toast::Toast;
use aura_design::components::tooltip::{Tooltip, Trigger};
use aura_design::components::window_chrome::WindowChrome;
use aura_design::components::*;

use aura_design::icon::Icon;
use aura_design::state::Interaction;
use aura_design::theme::Theme;
use aura_design::tokens::{Contrast, Scheme};

use aura_core::Size;
use aura_core::semantics::{Actions, Role};

const EVERY_SCHEME: [Scheme; 3] = [Scheme::Light, Scheme::Dark, Scheme::HighContrast];

/// Every component in its ordinary, enabled configuration, paired with a
/// name for failure messages. Variants that change the contract
/// (disabled, static, secure) get their own cases below.
fn every_component() -> Vec<(&'static str, Box<dyn ComponentSpec>)> {
    vec![
        (
            "app_bar",
            Box::new(AppBar {
                kind: AppBarKind::Standard,
                navigation_action: true,
            }),
        ),
        (
            "bottom_sheet",
            Box::new(BottomSheet {
                expanded: 0.5,
                dismissible: true,
            }),
        ),
        (
            "button",
            Box::new(Button {
                kind: ButtonKind::Filled,
                loading: false,
                disabled: false,
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
            "dialog",
            Box::new(Dialog {
                dismissible: true,
                focus_trapped: true,
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
            "list_tile",
            Box::new(ListTile {
                lines: ListLines::Two,
                interactive: true,
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
            "notification",
            Box::new(Notification {
                privacy: Privacy::Public,
                actionable: true,
                group_count: 1,
            }),
        ),
        (
            "progress",
            Box::new(Progress {
                value: Some(0.4),
                circular: false,
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
            "toast",
            Box::new(Toast {
                actionable: true,
                dismissible: true,
            }),
        ),
        (
            "tooltip",
            Box::new(Tooltip {
                focus_enabled: true,
                long_press_enabled: true,
            }),
        ),
        (
            "window_chrome",
            Box::new(WindowChrome {
                desktop: true,
                system_gestures_reserved: true,
            }),
        ),
    ]
}

/// Every interaction the matrix sweeps, named for failure messages.
fn every_interaction() -> Vec<(&'static str, Interaction)> {
    vec![
        ("default", Interaction::default()),
        (
            "hover",
            Interaction {
                hovered: true,
                ..Interaction::default()
            },
        ),
        (
            "focus_visible",
            Interaction {
                focus_visible: true,
                ..Interaction::default()
            },
        ),
        (
            "pressed",
            Interaction {
                pressed: true,
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
        (
            "disabled",
            Interaction {
                disabled: true,
                ..Interaction::default()
            },
        ),
        (
            "loading",
            Interaction {
                loading: true,
                ..Interaction::default()
            },
        ),
        (
            "error",
            Interaction {
                error: true,
                ..Interaction::default()
            },
        ),
    ]
}

// ---------------------------------------------------------------------
// The matrix: every component x every state x every theme
// ---------------------------------------------------------------------

#[test]
fn every_component_resolves_a_usable_appearance_in_every_state_and_theme() {
    for scheme in EVERY_SCHEME {
        let theme = Theme::new(scheme);

        for (name, component) in every_component() {
            for (state, interaction) in every_interaction() {
                let appearance = component.appearance(interaction, &theme);

                assert!(
                    (0.0..=1.0).contains(&appearance.overlay_alpha),
                    "{name}/{state}/{scheme:?}: overlay alpha {} out of range",
                    appearance.overlay_alpha
                );
                assert!(
                    appearance.foreground.a > 0.0,
                    "{name}/{state}/{scheme:?}: a fully transparent foreground is invisible"
                );
            }
        }
    }
}

#[test]
fn a_disabled_component_exposes_no_actions_and_reports_not_enabled() {
    // "Disabled" must be enforced in semantics, not merely painted: an
    // inert control still advertising ACTIVATE is a trap for assistive
    // technology.
    let disabled: Vec<(&str, Box<dyn ComponentSpec>)> = vec![
        (
            "button",
            Box::new(Button {
                kind: ButtonKind::Filled,
                loading: false,
                disabled: true,
            }),
        ),
        (
            "icon_button",
            Box::new(IconButton {
                icon: Icon::new("close", 24, false),
                tooltip: Some("Close"),
                disabled: true,
            }),
        ),
        (
            "selection",
            Box::new(Selection {
                kind: SelectionKind::Checkbox,
                checked: false,
                disabled: true,
            }),
        ),
        (
            "slider",
            Box::new(Slider {
                value: 0.5,
                range: (0.0, 1.0),
                rtl: false,
                disabled: true,
            }),
        ),
        (
            "text_field",
            Box::new(TextField {
                secure: false,
                multiline: false,
                error: false,
                disabled: true,
            }),
        ),
    ];

    let theme = Theme::default();

    for (name, component) in disabled {
        assert!(
            component.actions().is_empty(),
            "{name}: a disabled component must expose no actions"
        );
        assert!(
            !component.is_interactive(),
            "{name}: a disabled component must not report as interactive"
        );

        let appearance = component.appearance(
            Interaction {
                disabled: true,
                ..Interaction::default()
            },
            &theme,
        );
        assert!(
            !appearance.enabled,
            "{name}: appearance must report disabled"
        );
    }
}

#[test]
fn every_focusable_component_shows_a_focus_ring_only_when_focus_is_visible() {
    // A focusable control with no visible focus indicator is unusable by
    // keyboard. Non-negotiable in every theme.
    for scheme in EVERY_SCHEME {
        let theme = Theme::new(scheme);

        for (name, component) in every_component() {
            if !component.is_focusable() {
                continue;
            }

            let focused = component.appearance(
                Interaction {
                    focus_visible: true,
                    ..Interaction::default()
                },
                &theme,
            );

            let ring = focused.focus_ring.unwrap_or_else(|| {
                panic!("{name}/{scheme:?}: a focusable component must show a focus ring")
            });

            assert!(
                ring.width > 0.0,
                "{name}: a zero-width focus ring is invisible"
            );
            assert_eq!(
                ring.color,
                theme.colors().focus,
                "{name}: the focus ring must use the theme's focus token"
            );

            let unfocused = component.appearance(Interaction::default(), &theme);
            assert!(
                unfocused.focus_ring.is_none(),
                "{name}: a focus ring must only appear when focus is visible"
            );
        }
    }
}

#[test]
fn every_interactive_component_meets_the_minimum_touch_target() {
    for (name, component) in every_component() {
        if !component.is_interactive() {
            continue;
        }

        for density in Density::ALL {
            let size = component.min_size(density);

            // A container whose width is imposed by the window reports 0
            // and is exempt on that axis; its height must still qualify.
            if size.width > 0.0 {
                assert!(
                    size.width >= MIN_TOUCH_TARGET,
                    "{name}/{density:?}: width {} is below the {MIN_TOUCH_TARGET}px touch target",
                    size.width
                );
            }
            assert!(
                size.height >= MIN_TOUCH_TARGET,
                "{name}/{density:?}: height {} is below the {MIN_TOUCH_TARGET}px touch target",
                size.height
            );
        }
    }
}

#[test]
fn every_interactive_component_accepts_every_input_method() {
    // A pointer-only control excludes touch and keyboard users outright.
    for (name, component) in every_component() {
        if !component.is_interactive() {
            continue;
        }

        let inputs = component.input_methods();

        for required in [
            InputMethods::KEYBOARD,
            InputMethods::POINTER,
            InputMethods::TOUCH,
            InputMethods::GAMEPAD,
        ] {
            assert!(
                inputs.contains(required),
                "{name}: interactive components must accept {required:?}"
            );
        }
    }
}

#[test]
fn a_non_interactive_component_declares_no_inputs_and_takes_no_focus() {
    let inert: Vec<(&str, Box<dyn ComponentSpec>)> = vec![
        (
            "app_bar",
            Box::new(AppBar {
                kind: AppBarKind::Standard,
                navigation_action: true,
            }),
        ),
        (
            "progress",
            Box::new(Progress {
                value: None,
                circular: true,
            }),
        ),
        (
            "tooltip",
            Box::new(Tooltip {
                focus_enabled: true,
                long_press_enabled: true,
            }),
        ),
        (
            "window_chrome",
            Box::new(WindowChrome {
                desktop: false,
                system_gestures_reserved: true,
            }),
        ),
        (
            "card(static)",
            Box::new(Card {
                interactive: false,
                selected: false,
            }),
        ),
        (
            "list_tile(static)",
            Box::new(ListTile {
                lines: ListLines::One,
                interactive: false,
            }),
        ),
    ];

    for (name, component) in inert {
        assert!(
            !component.is_interactive(),
            "{name}: expected a non-interactive component"
        );
        assert!(
            component.input_methods().is_empty(),
            "{name}: a non-interactive component must declare no input methods"
        );
        assert!(
            !component.is_focusable(),
            "{name}: a non-interactive component must not be focusable"
        );
    }
}

#[test]
fn every_component_supports_rtl_large_text_and_reads_without_effects() {
    for (name, component) in every_component() {
        assert!(component.supports_rtl(), "{name}: must lay out mirrored");
        assert!(
            component.supports_large_text(),
            "{name}: must reflow for enlarged text"
        );
        assert!(
            component.readable_without_effects(),
            "{name}: must read with blur/shadow/noise disabled (Recovery tier)"
        );
    }
}

#[test]
fn every_component_declares_a_token_only_adaptive_layout() {
    use aura_design::tokens::Spacing;

    // "Theme tokens only -- no unexplained hard-coded spacing values":
    // every padding/gap must literally be a value from the scale.
    let scale = [
        Spacing::XS,
        Spacing::SM,
        Spacing::MD,
        Spacing::LG,
        Spacing::XL,
        Spacing::XXL,
    ];

    for (name, component) in every_component() {
        for viewport in ViewportClass::ALL {
            for density in Density::ALL {
                let layout = component.layout(viewport, density);

                assert!(
                    scale.contains(&layout.padding),
                    "{name}/{viewport:?}: padding {} is not a spacing token",
                    layout.padding
                );
                assert!(
                    scale.contains(&layout.gap),
                    "{name}/{viewport:?}: gap {} is not a spacing token",
                    layout.gap
                );
                assert_eq!(
                    layout.min_size,
                    component.min_size(density),
                    "{name}: layout min_size must agree with min_size()"
                );
            }
        }
    }
}

#[test]
fn a_component_only_claims_states_it_can_honor() {
    // Declaring DISABLED means actually going inert.
    let button = Button {
        kind: ButtonKind::Filled,
        loading: false,
        disabled: true,
    };
    assert!(button.supported_states().contains(StateSupport::DISABLED));
    assert!(button.actions().is_empty());

    // Declaring SELECTED means the appearance actually changes: a state
    // nobody can see is not a state.
    let theme = Theme::default();
    let selection = Selection {
        kind: SelectionKind::Checkbox,
        checked: true,
        disabled: false,
    };
    assert!(
        selection
            .supported_states()
            .contains(StateSupport::SELECTED)
    );

    let unselected = selection.appearance(Interaction::default(), &theme);
    let selected = selection.appearance(
        Interaction {
            selected: true,
            ..Interaction::default()
        },
        &theme,
    );
    assert_ne!(
        unselected.background, selected.background,
        "a component claiming SELECTED must look different when selected"
    );
}

#[test]
fn hover_and_press_produce_distinct_increasing_state_layers() {
    // The state layer is what makes a control feel alive; press must read
    // as stronger than hover, or the feedback is ambiguous.
    let theme = Theme::default();
    let button = Button::default();

    let rest = button.appearance(Interaction::default(), &theme);
    let hover = button.appearance(
        Interaction {
            hovered: true,
            ..Interaction::default()
        },
        &theme,
    );
    let pressed = button.appearance(
        Interaction {
            pressed: true,
            ..Interaction::default()
        },
        &theme,
    );

    assert!(rest.overlay_alpha < hover.overlay_alpha);
    assert!(hover.overlay_alpha < pressed.overlay_alpha);
}

// ---------------------------------------------------------------------
// Contrast: readability in every theme
// ---------------------------------------------------------------------

#[test]
fn every_components_foreground_is_readable_against_what_it_sits_on() {
    // The design system's central promise -- beauty from typography,
    // hierarchy, and colour -- is worthless if the text cannot be read.
    //
    // A component that paints its own fill is checked against that fill.
    // One that does not still has to be legible against the surface it
    // will actually sit on, so it is checked against the theme's surface
    // token rather than skipped -- otherwise this test would only cover
    // the three components that happen to draw a background.
    let mut checked = 0;

    for scheme in EVERY_SCHEME {
        let theme = Theme::new(scheme);

        for (name, component) in every_component() {
            for (state, interaction) in every_interaction() {
                let appearance = component.appearance(interaction, &theme);

                // A disabled control is deliberately dimmed by its state
                // layer; body contrast is not the right bar for it.
                if interaction.disabled {
                    continue;
                }

                let background = appearance.background.unwrap_or(theme.colors().surface);

                assert!(
                    Contrast::body_text(appearance.foreground, background),
                    "{name}/{state}/{scheme:?}: foreground fails body contrast against its \
                     background (ratio {:.2})",
                    Contrast::ratio(appearance.foreground, background)
                );
                checked += 1;
            }
        }
    }

    // Guard against a vacuous pass: if the skip rules ever swallow every
    // case, this test must fail rather than silently assert nothing.
    assert!(
        checked >= EVERY_SCHEME.len() * every_component().len(),
        "contrast sweep only checked {checked} cases -- it is not covering the matrix"
    );
}

#[test]
fn the_matrix_actually_covers_focusable_and_interactive_components() {
    // These sweeps skip components the rule does not apply to. If a
    // refactor ever made *everything* skippable, the sweeps above would
    // pass while checking nothing. Pin the coverage so that cannot happen
    // silently.
    let components = every_component();

    let focusable = components.iter().filter(|(_, c)| c.is_focusable()).count();
    let interactive = components
        .iter()
        .filter(|(_, c)| c.is_interactive())
        .count();

    assert!(
        focusable >= 8,
        "only {focusable} focusable components -- the focus-ring sweep has lost coverage"
    );
    assert!(
        interactive >= 10,
        "only {interactive} interactive components -- the touch-target and input sweeps have lost coverage"
    );
    assert!(
        components.len() >= 17,
        "the matrix is missing components: only {} registered",
        components.len()
    );
}

// ---------------------------------------------------------------------
// Component-specific contracts
// ---------------------------------------------------------------------

#[test]
fn a_secure_field_reports_the_secure_role_in_every_configuration() {
    for multiline in [false, true] {
        for error in [false, true] {
            for disabled in [false, true] {
                let field = TextField {
                    secure: true,
                    multiline,
                    error,
                    disabled,
                };
                assert_eq!(
                    field.role(),
                    Role::SecureField,
                    "a secure field must never downgrade to a plain TextField"
                );
            }
        }
    }
}

#[test]
fn a_modal_dialog_traps_focus_and_can_be_dismissed_from_every_input() {
    let dialog = Dialog {
        dismissible: true,
        focus_trapped: true,
    };

    assert!(dialog.actions().contains(Actions::DISMISS));
    assert!(dialog.traps_focus_correctly());
    assert_eq!(dialog.input_methods(), InputMethods::ALL);
}

#[test]
fn a_loading_button_stays_focusable_but_cannot_be_activated() {
    let loading = Button {
        kind: ButtonKind::Filled,
        loading: true,
        disabled: false,
    };

    assert!(
        loading.actions().contains(Actions::FOCUS),
        "a busy button must not eject a keyboard user"
    );
    assert!(
        !loading.actions().contains(Actions::ACTIVATE),
        "a busy button must not accept a second activation"
    );

    let appearance = loading.appearance(
        Interaction {
            loading: true,
            ..Interaction::default()
        },
        &Theme::default(),
    );
    assert!(appearance.busy);
}

#[test]
fn a_slider_mirrors_its_direction_in_rtl() {
    let ltr = Slider {
        value: 0.25,
        range: (0.0, 1.0),
        rtl: false,
        disabled: false,
    };
    let rtl = Slider { rtl: true, ..ltr };

    assert!((ltr.normalized() - 0.25).abs() < f32::EPSILON);
    assert!(
        (rtl.normalized() - 0.75).abs() < f32::EPSILON,
        "an RTL slider's maximum sits at the left"
    );
}

#[test]
fn a_slider_clamps_out_of_range_values_and_survives_a_degenerate_range() {
    let over = Slider {
        value: 99.0,
        range: (0.0, 1.0),
        rtl: false,
        disabled: false,
    };
    assert_eq!(over.normalized(), 1.0);

    let degenerate = Slider {
        value: 5.0,
        range: (3.0, 3.0),
        rtl: false,
        disabled: false,
    };
    assert_eq!(
        degenerate.normalized(),
        0.0,
        "a zero-width range must not divide by zero"
    );
}

#[test]
fn navigation_adapts_its_mode_to_the_viewport() {
    assert_eq!(
        Navigation::for_viewport(ViewportClass::Compact).mode,
        NavigationMode::Bar
    );
    assert_eq!(
        Navigation::for_viewport(ViewportClass::Medium).mode,
        NavigationMode::Rail
    );
    assert_eq!(
        Navigation::for_viewport(ViewportClass::Expanded).mode,
        NavigationMode::Sidebar
    );

    // Only a sidebar has room for a label beside the icon.
    assert!(Navigation::for_viewport(ViewportClass::Expanded).shows_label());
    assert!(!Navigation::for_viewport(ViewportClass::Compact).shows_label());
}

#[test]
fn a_bottom_sheet_snaps_to_the_nearest_anchor_and_never_dismisses_when_forbidden() {
    let dismissible = |expanded| BottomSheet {
        expanded,
        dismissible: true,
    };

    assert_eq!(dismissible(0.05).snap_target(), Anchor::Closed);
    assert_eq!(dismissible(0.45).snap_target(), Anchor::Half);
    assert_eq!(dismissible(0.95).snap_target(), Anchor::Full);

    // A non-dismissible sheet dragged to nearly closed must still settle
    // open -- a drag cannot dismiss what the caller said cannot be.
    let stubborn = BottomSheet {
        expanded: 0.0,
        dismissible: false,
    };
    assert_eq!(stubborn.snap_target(), Anchor::Half);
}

#[test]
fn notification_privacy_withholds_content_on_a_locked_screen() {
    assert!(Privacy::Public.shows_body(true));
    assert!(!Privacy::Sensitive.shows_body(true));
    assert!(Privacy::Sensitive.shows_title(true));
    assert!(!Privacy::Secret.shows_title(true));

    // Unlocked, every level shows everything.
    for privacy in Privacy::ALL {
        assert!(privacy.shows_title(false));
        assert!(privacy.shows_body(false));
    }
}

#[test]
fn an_actionable_toast_stays_visible_long_enough_to_reach_its_action() {
    let plain = Toast {
        actionable: false,
        dismissible: true,
    };
    let actionable = Toast {
        actionable: true,
        dismissible: true,
    };

    assert!(
        actionable.visible_duration() > plain.visible_duration(),
        "a toast with an action needs longer than one without"
    );
    assert!(actionable.announces_as_live_region());
}

#[test]
fn an_icon_only_button_requires_an_accessible_name() {
    let named = IconButton {
        icon: Icon::new("close", 24, false),
        tooltip: Some("Close"),
        disabled: false,
    };
    let unnamed = IconButton {
        tooltip: None,
        ..named
    };
    let blank = IconButton {
        tooltip: Some("   "),
        ..named
    };

    assert!(named.has_accessible_name());
    assert!(!unnamed.has_accessible_name());
    assert!(!blank.has_accessible_name(), "whitespace is not a name");
}

#[test]
fn a_tooltip_carrying_information_is_reachable_without_a_pointer() {
    let pointer_only = Tooltip {
        focus_enabled: false,
        long_press_enabled: false,
    };
    let reachable = Tooltip {
        focus_enabled: true,
        long_press_enabled: true,
    };

    assert!(!pointer_only.reachable_without_pointer());
    assert!(reachable.reachable_without_pointer());

    assert!(reachable.responds_to(Trigger::Focus));
    assert!(reachable.responds_to(Trigger::LongPress));
    assert!(pointer_only.responds_to(Trigger::Hover));
    assert!(!pointer_only.responds_to(Trigger::Focus));
}

#[test]
fn window_chrome_reserves_the_system_gesture_band() {
    let reserved = WindowChrome {
        desktop: false,
        system_gestures_reserved: true,
    };
    let unreserved = WindowChrome {
        desktop: true,
        system_gestures_reserved: false,
    };

    // An app control must not own a drag starting inside the reserved
    // band; the system's SYSTEM-priority edge_swipe wins there by policy.
    assert!(!reserved.may_own_drag_at(0.0));
    assert!(!reserved.may_own_drag_at(10.0));
    assert!(reserved.may_own_drag_at(40.0));

    // With nothing reserved, the app owns its whole surface.
    assert!(unreserved.may_own_drag_at(0.0));
}

#[test]
fn a_collapsing_app_bar_shrinks_and_others_do_not() {
    let collapsing = AppBar {
        kind: AppBarKind::Collapsing,
        navigation_action: true,
    };
    let standard = AppBar {
        kind: AppBarKind::Standard,
        navigation_action: false,
    };

    assert!(collapsing.height(0.0) > collapsing.height(1.0));
    assert_eq!(standard.height(0.0), standard.height(1.0));

    // Progress outside 0..=1 clamps rather than inverting the bar.
    assert_eq!(collapsing.height(-5.0), collapsing.height(0.0));
    assert_eq!(collapsing.height(5.0), collapsing.height(1.0));
}

#[test]
fn progress_clamps_its_fraction_and_substitutes_reduced_motion() {
    use aura_motion::{Effect, MotionPolicy};

    let determinate = Progress {
        value: Some(2.0),
        circular: false,
    };
    assert_eq!(determinate.fraction(), Some(1.0));
    assert!(!determinate.is_indeterminate());

    let indeterminate = Progress {
        value: None,
        circular: true,
    };
    assert!(indeterminate.is_indeterminate());

    // Reduced motion substitutes a fade for the spin rather than freezing
    // the indicator, which would read as "stalled".
    assert_eq!(
        indeterminate.respects_reduced_motion(MotionPolicy::Reduced),
        Effect::Fade
    );
    assert_eq!(
        indeterminate.respects_reduced_motion(MotionPolicy::Full),
        Effect::Transform
    );
}

#[test]
fn menu_typeahead_matches_prefixes_case_insensitively() {
    assert!(Menu::matches_typeahead("Save As...", "sa"));
    assert!(Menu::matches_typeahead("Save As...", "SAVE"));
    assert!(!Menu::matches_typeahead("Save As...", "as"));
    assert!(
        !Menu::matches_typeahead("Save As...", "   "),
        "an empty query must not jump the selection"
    );
}

/// The touch-target floor is a *policy*, and a policy that two crates
/// state independently is a policy that will eventually disagree with
/// itself. `phase.md` sets 48px; `aura-design` and the widgets must both
/// mean that same 48px, and the compatibility floor must stay an
/// explicit, separately-named opt-out rather than a second default.
#[test]
fn the_touch_target_policy_has_exactly_one_source() {
    assert_eq!(
        MIN_TOUCH_TARGET,
        aura_core::touch::MIN_TARGET,
        "aura-design must re-export the core constant, not restate it"
    );
    assert_eq!(
        MIN_TOUCH_TARGET, 48.0,
        "phase.md's default target is 48x48 logical pixels"
    );

    assert_eq!(
        MIN_TOUCH_TARGET_COMPATIBILITY,
        aura_core::touch::MIN_TARGET_COMPATIBILITY
    );
    const {
        assert!(
            MIN_TOUCH_TARGET_COMPATIBILITY < MIN_TOUCH_TARGET,
            "the compatibility floor only makes sense as a relaxation"
        );
    }

    assert_eq!(TouchTargetPolicy::default(), TouchTargetPolicy::Vyom);
    assert_eq!(TouchTargetPolicy::Vyom.min_edge(), MIN_TOUCH_TARGET);
    assert_eq!(
        TouchTargetPolicy::PlatformCompatibility.min_edge(),
        MIN_TOUCH_TARGET_COMPATIBILITY
    );
}

/// A target that is tall enough but too narrow is still a miss. Both
/// axes are checked, on both policies.
#[test]
fn touch_target_policies_check_both_axes() {
    for policy in TouchTargetPolicy::ALL {
        let edge = policy.min_edge();

        assert!(policy.is_satisfied_by(Size::new(edge, edge)));
        assert!(!policy.is_satisfied_by(Size::new(edge - 0.01, edge)));
        assert!(!policy.is_satisfied_by(Size::new(edge, edge - 0.01)));
    }
}

/// Every component that accepts input must clear the *default* policy at
/// every density -- the compatibility floor is not a licence to ship a
/// smaller control.
///
/// The policy governs **touch targets**, and only a component that
/// accepts input presents one. A pure container declares no actions and
/// takes at least one axis from its parent -- `app_bar` is the case in
/// point: it is a `Role::Group` with `Actions::empty()` whose width is
/// imposed by the window, so it has no intrinsic target to measure.
/// Skipping it here is not a relaxation of the policy, because the
/// controls it *hosts* (an `IconButton` in its navigation slot) are
/// separate specs that are checked on their own. The companion test
/// below pins the exempt set so this cannot quietly grow.
#[test]
fn every_component_clears_the_default_policy_at_every_density() {
    for (name, spec) in every_component() {
        if !spec.is_interactive() {
            continue;
        }

        for density in Density::ALL {
            let size = spec.min_size(density);

            assert!(
                TouchTargetPolicy::Vyom.is_satisfied_by(size),
                "{name}/{density:?}: {size:?} is below the {MIN_TOUCH_TARGET}px default policy"
            );
        }
    }
}

/// The exemption above is only sound while "non-interactive" stays a
/// deliberate property of a small, known set of containers.
///
/// Without this test, a real control that lost its actions -- through a
/// refactor, a wrong `disabled` default, a typo in `actions()` -- would
/// silently drop out of touch-target coverage instead of failing. Pinning
/// the set means that regression shows up here as an unexpected name.
#[test]
fn only_known_containers_are_exempt_from_the_touch_target_policy() {
    let exempt: Vec<&'static str> = every_component()
        .into_iter()
        .filter(|(_, spec)| !spec.is_interactive())
        .map(|(name, _)| name)
        .collect();

    // Each of these is presentation, not control: `app_bar` and
    // `window_chrome` are containers whose buttons are separate
    // `IconButton` specs, and `progress` and `tooltip` report status
    // without ever accepting input.
    assert_eq!(
        exempt,
        ["app_bar", "progress", "tooltip", "window_chrome"],
        "the set of components exempt from the touch-target policy changed; \
         a container may join it deliberately, but a control appearing here \
         means it stopped declaring actions"
    );
}

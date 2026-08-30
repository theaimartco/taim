//! Deterministic design-system oracles, independent of a GPU backend.
//!
//! In place of pixel goldens (which need a device and human visual
//! approval), this pins the *approved contract* of the design system:
//! the exact token values every scheme commits to, the state-resolution
//! rules, and the cross-scheme invariants that keep the language coherent
//! (`phase.md` Phase 11 exit gate: "design golden suite is reviewed at
//! phone/tablet/desktop/watch profiles; visual review finds one coherent
//! language rather than individually styled widgets").
//!
//! A change to any of these must update this oracle deliberately, exactly
//! as re-approving a golden image would.

use taim_design::components::button::{Button, ButtonKind};
use taim_design::components::{ComponentSpec, Density, ViewportClass};
use taim_design::state::Interaction;
use taim_design::theme::Theme;
use taim_design::tokens::{ColorTokens, Contrast, Radius, Scheme, Spacing};

const EVERY_SCHEME: [Scheme; 3] = [Scheme::Light, Scheme::Dark, Scheme::HighContrast];

#[test]
fn semantic_states_resolve_deterministically_in_every_theme() {
    for scheme in EVERY_SCHEME {
        let colors = ColorTokens::for_scheme(scheme);
        let state = Interaction {
            focus_visible: true,
            selected: true,
            ..Interaction::default()
        }
        .resolve(colors);

        assert_eq!(state.foreground, colors.primary);
        assert_eq!(state.focus_ring, Some(colors.focus));
    }
}

#[test]
fn the_state_layer_ramp_is_pinned_and_ordered() {
    // The approved state-layer alphas. These are the whole of Taim's
    // interaction feedback; changing one changes how every control feels.
    let colors = ColorTokens::for_scheme(Scheme::Dark);

    let rest = Interaction::default().resolve(colors);
    let hover = Interaction {
        hovered: true,
        ..Interaction::default()
    }
    .resolve(colors);
    let pressed = Interaction {
        pressed: true,
        ..Interaction::default()
    }
    .resolve(colors);
    let disabled = Interaction {
        disabled: true,
        ..Interaction::default()
    }
    .resolve(colors);

    assert_eq!(rest.overlay_alpha, 0.0);
    assert_eq!(hover.overlay_alpha, 0.08);
    assert_eq!(pressed.overlay_alpha, 0.16);
    assert_eq!(disabled.overlay_alpha, 0.38);

    // Ordering is the part a human would notice immediately if broken.
    assert!(rest.overlay_alpha < hover.overlay_alpha);
    assert!(hover.overlay_alpha < pressed.overlay_alpha);
}

#[test]
fn the_spacing_scale_is_pinned() {
    // A compact, canonical scale: every layout in the system is built
    // from exactly these six values.
    assert_eq!(Spacing::XS, 4.0);
    assert_eq!(Spacing::SM, 8.0);
    assert_eq!(Spacing::MD, 12.0);
    assert_eq!(Spacing::LG, 16.0);
    assert_eq!(Spacing::XL, 24.0);
    assert_eq!(Spacing::XXL, 32.0);
}

#[test]
fn the_radius_scale_is_pinned_and_ordered() {
    assert_eq!(Radius::None.pixels(), Some(0.0));
    assert_eq!(Radius::Small.pixels(), Some(6.0));
    assert_eq!(Radius::Medium.pixels(), Some(12.0));
    assert_eq!(Radius::Large.pixels(), Some(20.0));
    assert_eq!(
        Radius::Full.pixels(),
        None,
        "a pill radius is resolved from the component's own height, not a fixed value"
    );

    let finite = [Radius::None, Radius::Small, Radius::Medium, Radius::Large];
    for pair in finite.windows(2) {
        assert!(pair[0].pixels().unwrap() < pair[1].pixels().unwrap());
    }
}

#[test]
fn every_scheme_passes_body_contrast_on_its_core_pairings() {
    // The coherence guarantee: the same semantic pairing is readable in
    // every scheme, so a theme switch never silently breaks legibility.
    for scheme in EVERY_SCHEME {
        let colors = ColorTokens::for_scheme(scheme);

        for (name, foreground, background) in [
            ("on_surface/surface", colors.on_surface, colors.surface),
            (
                "on_surface/background",
                colors.on_surface,
                colors.background,
            ),
            ("on_primary/primary", colors.on_primary, colors.primary),
            (
                "on_surface_variant/surface",
                colors.on_surface_variant,
                colors.surface,
            ),
        ] {
            assert!(
                Contrast::body_text(foreground, background),
                "{scheme:?}/{name}: ratio {:.2} is below the 4.5 body-text bar",
                Contrast::ratio(foreground, background)
            );
        }
    }
}

#[test]
fn high_contrast_is_strictly_more_legible_than_the_ordinary_schemes() {
    // High contrast is not merely "a different palette": it must actually
    // out-perform the default schemes, or it fails the users who need it.
    let high = ColorTokens::for_scheme(Scheme::HighContrast);
    let high_ratio = Contrast::ratio(high.on_surface, high.surface);

    for scheme in [Scheme::Light, Scheme::Dark] {
        let colors = ColorTokens::for_scheme(scheme);
        let ratio = Contrast::ratio(colors.on_surface, colors.surface);

        assert!(
            high_ratio >= ratio,
            "HighContrast ({high_ratio:.2}) must be at least as legible as {scheme:?} ({ratio:.2})"
        );
    }

    // And it must clear the stricter AAA bar, which is the point of it.
    assert!(
        high_ratio >= 7.0,
        "HighContrast body text scores {high_ratio:.2}, below the 7.0 AAA bar"
    );
}

#[test]
fn a_theme_switch_changes_the_revision_exactly_once_per_real_change() {
    let theme = Theme::new(Scheme::Dark);

    // A no-op switch must not invalidate anything downstream.
    assert_eq!(theme.with_scheme(Scheme::Dark).revision(), theme.revision());

    // A real switch bumps the revision so caches know to rebuild.
    let switched = theme.with_scheme(Scheme::Light);
    assert!(switched.revision() > theme.revision());
    assert_eq!(switched.scheme(), Scheme::Light);
    assert_eq!(switched.colors(), ColorTokens::for_scheme(Scheme::Light));
}

#[test]
fn one_component_renders_coherently_across_every_profile() {
    // The "phone/tablet/desktop" review profiles, as a deterministic
    // contract: the same button keeps its identity (role, shape, tokens)
    // at every viewport and density, changing only its spacing.
    let button = Button {
        kind: ButtonKind::Filled,
        loading: false,
        disabled: false,
    };
    let theme = Theme::default();

    let appearance = button.appearance(Interaction::default(), &theme);
    assert_eq!(appearance.background, Some(theme.colors().primary));
    assert_eq!(appearance.foreground, theme.colors().on_primary);
    assert_eq!(appearance.radius, Radius::Full);

    let mut paddings = Vec::new();

    for viewport in ViewportClass::ALL {
        for density in Density::ALL {
            let layout = button.layout(viewport, density);

            // Identity is stable across every profile...
            assert_eq!(button.min_size(density), layout.min_size);
            paddings.push(layout.padding);
        }
    }

    // ...while spacing genuinely adapts (it is not one frozen value).
    assert!(
        paddings.iter().any(|&p| p != paddings[0]),
        "layout must actually adapt across viewports"
    );
}

#[test]
fn every_button_variant_stays_within_the_token_palette() {
    // "Theme tokens only -- no unexplained hard-coded color values":
    // whatever a variant paints must be a colour the theme published.
    let theme = Theme::default();
    let colors = theme.colors();
    let palette = [
        colors.background,
        colors.surface,
        colors.primary,
        colors.on_primary,
        colors.on_surface,
        colors.on_surface_variant,
        colors.error,
        colors.focus,
    ];

    for kind in [
        ButtonKind::Filled,
        ButtonKind::Tonal,
        ButtonKind::Outlined,
        ButtonKind::Text,
        ButtonKind::Destructive,
    ] {
        let button = Button {
            kind,
            loading: false,
            disabled: false,
        };
        let appearance = button.appearance(Interaction::default(), &theme);

        assert!(
            palette.contains(&appearance.foreground),
            "{kind:?}: foreground is not a theme token"
        );
        if let Some(background) = appearance.background {
            assert!(
                palette.contains(&background),
                "{kind:?}: background is not a theme token"
            );
        }
        if let Some(border) = appearance.border {
            assert!(
                palette.contains(&border),
                "{kind:?}: border is not a theme token"
            );
        }
    }
}

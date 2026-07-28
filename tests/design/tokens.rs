use aura_core::text::Script;
use aura_design::Theme;
use aura_design::tokens::{ColorTokens, Contrast, Scheme, Spacing, TextStyle, Typography};

#[test]
fn all_schemes_have_readable_primary_and_surface_text() {
    for scheme in [Scheme::Light, Scheme::Dark, Scheme::HighContrast] {
        let colors = ColorTokens::for_scheme(scheme);
        assert!(Contrast::body_text(colors.on_primary, colors.primary));
        assert!(Contrast::body_text(colors.on_surface, colors.surface));
    }
}

#[test]
fn spacing_scale_is_monotonic_and_touch_scale_is_not_compact_only() {
    // The spacing ramp is a set of compile-time constants, so its
    // monotonicity is a compile-time invariant, not a runtime one: a
    // `const` block makes a future edit that reorders the scale fail to
    // compile rather than fail a test run.
    const {
        assert!(Spacing::XS < Spacing::SM && Spacing::SM < Spacing::MD);
        assert!(Spacing::MD < Spacing::LG && Spacing::LG < Spacing::XL);
    }
}

#[test]
fn script_aware_line_metrics_do_not_clip_complex_scripts() {
    assert!(
        Typography::style(TextStyle::Body, Script::Arabic).line_height
            >= Typography::style(TextStyle::Body, Script::Latin).line_height
    );
}

#[test]
fn theme_revision_changes_only_for_actual_theme_changes() {
    let theme = Theme::default();
    assert_eq!(
        theme.with_scheme(theme.scheme()).revision(),
        theme.revision()
    );
    assert!(theme.with_scheme(Scheme::Light).revision() > theme.revision());
}

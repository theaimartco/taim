//! Every widget `Catalog` must have an `aura_design::Theme` implementation.
//!
//! This gap is not hypothetical. 17 of 41 catalogues had no implementation
//! for months -- and they were exactly the mobile set (`bottom_sheet`,
//! `drawer`, `tabs`, `chip`, `badge`, `avatar`, `date_picker`, and the rest),
//! the part of the widget set built to differentiate Aura. They compiled and
//! ran the whole time because each falls back to its own `aura_core::Theme`
//! catalogue, so nothing failed; the design system simply did not reach them.
//! Meanwhile `widget/src/aura.rs` documented itself as covering everything.
//!
//! Two mechanisms, because they catch different failures:
//!
//! 1. [`static_assertions`] fails to **compile** if an implementation that
//!    exists today is removed or its signature drifts.
//! 2. [`every_catalog_has_a_theme_implementation`] reads the source and fails
//!    if a *newly added* catalogue has no implementation -- which a
//!    hand-written assertion list cannot notice by construction.

/// Compile-time proof for the catalogues that exist today.
///
/// Each line is a type-check, not a runtime call: if `Theme` stops
/// implementing one of these, this module stops compiling.
mod static_assertions {
    use aura_design::theme::Theme;

    macro_rules! assert_catalog {
        ($($module:ident),* $(,)?) => {
            $(
                const _: fn() = || {
                    fn assert_impl<T: aura_widget::$module::Catalog>() {}
                    assert_impl::<Theme>();
                };
            )*
        };
    }

    assert_catalog!(
        accordion,
        app_bar,
        avatar,
        badge,
        bottom_nav_bar,
        bottom_sheet,
        button,
        carousel,
        checkbox,
        chip,
        container,
        date_picker,
        dialog,
        drawer,
        float,
        icon_button,
        list_tile,
        pane_grid,
        pick_list,
        progress_bar,
        pull_to_refresh,
        radio,
        rule,
        scrollable,
        search_bar,
        segmented_control,
        skeleton,
        slider,
        snackbar,
        stepper,
        table,
        tabs,
        text_editor,
        text_input,
        time_picker,
        toggler,
    );

    // `combo_box` blanket-implements through `pick_list` rather than
    // declaring its own bound shape.
    const _: fn() = || {
        fn assert_impl<T: aura_widget::combo_box::Catalog>() {}
        assert_impl::<Theme>();
    };

    // These three are behind features, so their assertions have to be too --
    // otherwise a default-feature build fails to compile a test about
    // coverage rather than reporting one.
    #[cfg(feature = "markdown")]
    const _: fn() = || {
        fn assert_impl<T: aura_widget::markdown::Catalog>() {}
        assert_impl::<Theme>();
    };

    #[cfg(feature = "svg")]
    const _: fn() = || {
        fn assert_impl<T: aura_widget::svg::Catalog>() {}
        assert_impl::<Theme>();
    };

    #[cfg(feature = "qr_code")]
    const _: fn() = || {
        fn assert_impl<T: aura_widget::qr_code::Catalog>() {}
        assert_impl::<Theme>();
    };
}

/// Fails when a catalogue is added without an implementation.
///
/// Reads the tree rather than a list someone has to remember to update: a
/// hand-maintained list is exactly what let 17 catalogues go uncovered.
#[test]
fn every_catalog_has_a_theme_implementation() {
    use std::collections::BTreeSet;
    use std::path::Path;

    let widget_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("widget/src");

    let mut declared = BTreeSet::new();

    for entry in std::fs::read_dir(&widget_src).expect("widget/src should be readable") {
        let path = entry.expect("a readable directory entry").path();

        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }

        let source = std::fs::read_to_string(&path).expect("a readable widget module");

        if source.contains("pub trait Catalog") {
            let module = path
                .file_stem()
                .expect("a named module")
                .to_string_lossy()
                .into_owned();

            let _ = declared.insert(module);
        }
    }

    assert!(
        declared.len() > 30,
        "expected to discover the widget catalogues; found {} -- has widget/src moved?",
        declared.len()
    );

    let bridge = std::fs::read_to_string(widget_src.join("aura.rs"))
        .expect("widget/src/aura.rs should be readable");

    let missing: Vec<&String> = declared
        .iter()
        .filter(|module| {
            let bare = format!("impl {module}::Catalog for Theme");
            let qualified = format!("impl crate::{module}::Catalog for Theme");

            !bridge.contains(&bare) && !bridge.contains(&qualified)
        })
        .collect();

    assert!(
        missing.is_empty(),
        "these widgets declare a `Catalog` with no `aura_design::Theme` \
         implementation in widget/src/aura.rs: {missing:?}\n\
         They will still compile and run -- each falls back to its own \
         `aura_core::Theme` catalogue -- but the design system does not reach \
         them, which is the drift this test exists to prevent."
    );
}

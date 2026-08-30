//! Proves the design system actually reaches real widgets.
//!
//! `phase.md` Phase 11's whole point is a coherent visual language for
//! *production components*. Until these tests existed, `taim-design` was
//! an island: its tokens, state resolution, and component contracts were
//! tested only against themselves, and no `taim_widget` rendered from
//! them. A design system nothing renders is a specification, not a design
//! system.
//!
//! These drive the real `Catalog` impls (`taim_widget::taim`) that a
//! running application uses, and assert the resulting per-widget `Style`
//! carries Taim's semantic tokens -- so the state matrix
//! `components.rs` proves is demonstrably the one pixels come from.

use taim_design::components::button::ButtonKind;
use taim_design::theme::Theme;
use taim_design::tokens::{Contrast, MaterialTokens, Scheme};

use taim_core::material::{Material, SceneLight};
use taim_core::{Background, Color};
use taim_widget::taim;
use taim_widget::{
    button, checkbox, container, icon_button, list_tile, progress_bar, radio, text_input,
};

const EVERY_SCHEME: [Scheme; 3] = [Scheme::Light, Scheme::Dark, Scheme::HighContrast];

fn color_of(background: Background) -> Color {
    match background {
        Background::Color(color) => color,
        Background::Gradient(_) => {
            panic!("Taim only ever produces solid colour backgrounds")
        }
    }
}

// ---------------------------------------------------------------------
// The theme actually drives real widgets
// ---------------------------------------------------------------------

#[test]
fn a_real_button_renders_taims_primary_token() {
    // The end-to-end claim: an `taim_widget::button` themed with
    // `taim_design::Theme` paints Taim's `primary`, not taim's palette.
    for scheme in EVERY_SCHEME {
        let theme = Theme::new(scheme);
        let class = <Theme as button::Catalog>::default();

        let style = button::Catalog::style(&theme, &class, button::Status::Active);

        assert_eq!(
            style.background.map(color_of),
            Some(theme.colors().primary),
            "{scheme:?}: a filled button must paint the primary token"
        );
        assert_eq!(style.text_color, theme.colors().on_primary);
    }
}

#[test]
fn a_real_button_is_readable_in_every_scheme() {
    // The contrast guarantee has to survive the trip through the widget's
    // own Style, not just hold inside taim-design.
    for scheme in EVERY_SCHEME {
        let theme = Theme::new(scheme);
        let class = <Theme as button::Catalog>::default();
        let style = button::Catalog::style(&theme, &class, button::Status::Active);

        let background = style.background.map(color_of).expect("filled button");

        assert!(
            Contrast::body_text(style.text_color, background),
            "{scheme:?}: rendered button text fails contrast (ratio {:.2})",
            Contrast::ratio(style.text_color, background)
        );
    }
}

#[test]
fn a_real_buttons_state_layer_deepens_from_rest_to_hover_to_press() {
    // Taim resolves feedback as one overlay alpha; the bridge turns it
    // into a real colour. Each step must be visibly distinct, or the
    // control feels dead.
    let theme = Theme::new(Scheme::Dark);
    let class = <Theme as button::Catalog>::default();

    let rest = button::Catalog::style(&theme, &class, button::Status::Active);
    let hovered = button::Catalog::style(&theme, &class, button::Status::Hovered);
    let pressed = button::Catalog::style(&theme, &class, button::Status::Pressed);

    let rest_bg = rest.background.map(color_of).unwrap();
    let hovered_bg = hovered.background.map(color_of).unwrap();
    let pressed_bg = pressed.background.map(color_of).unwrap();

    assert_ne!(rest_bg, hovered_bg, "hover must be visible");
    assert_ne!(hovered_bg, pressed_bg, "press must differ from hover");
}

#[test]
fn a_disabled_real_button_fades_rather_than_changing_hue() {
    // Disabled is an alpha reduction, not a different colour: the control
    // must still read as *itself*, only inert.
    let theme = Theme::new(Scheme::Dark);
    let class = <Theme as button::Catalog>::default();

    let active = button::Catalog::style(&theme, &class, button::Status::Active);
    let disabled = button::Catalog::style(&theme, &class, button::Status::Disabled);

    let active_bg = active.background.map(color_of).unwrap();
    let disabled_bg = disabled.background.map(color_of).unwrap();

    assert!(
        disabled_bg.a < active_bg.a,
        "a disabled button must fade: {} vs {}",
        disabled_bg.a,
        active_bg.a
    );
    assert_eq!(
        (disabled_bg.r, disabled_bg.g, disabled_bg.b),
        (active_bg.r, active_bg.g, active_bg.b),
        "a disabled button must not change hue"
    );
}

#[test]
fn every_button_variant_reaches_the_widget_with_its_own_tokens() {
    let theme = Theme::new(Scheme::Dark);
    let colors = theme.colors();

    let filled = taim::button_style(ButtonKind::Filled)(&theme, button::Status::Active);
    let destructive = taim::button_style(ButtonKind::Destructive)(&theme, button::Status::Active);
    let outlined = taim::button_style(ButtonKind::Outlined)(&theme, button::Status::Active);
    let text_only = taim::button_style(ButtonKind::Text)(&theme, button::Status::Active);

    assert_eq!(filled.background.map(color_of), Some(colors.primary));
    assert_eq!(destructive.background.map(color_of), Some(colors.error));

    // An outlined button draws a real border and no fill; a text button
    // draws neither. That difference is what distinguishes them.
    assert_eq!(outlined.background, None);
    assert!(outlined.border.width > 0.0);
    assert_eq!(outlined.border.color, colors.primary);

    assert_eq!(text_only.background, None);
    assert_eq!(text_only.border.width, 0.0);
}

#[test]
fn a_real_text_input_renders_taim_tokens_and_shows_focus() {
    for scheme in EVERY_SCHEME {
        let theme = Theme::new(scheme);
        let colors = theme.colors();
        let class = <Theme as text_input::Catalog>::default();

        let active = text_input::Catalog::style(&theme, &class, text_input::Status::Active);
        let focused = text_input::Catalog::style(
            &theme,
            &class,
            text_input::Status::Focused { is_hovered: false },
        );

        assert_eq!(color_of(active.background), colors.surface);
        assert_eq!(active.value, colors.on_surface);
        assert_eq!(active.selection, colors.primary);

        // Focus must be visible on the control itself, via the focus
        // token -- a keyboard user has nothing else to go on.
        assert_eq!(
            focused.border.color, colors.focus,
            "{scheme:?}: a focused field must border with the focus token"
        );
        assert_ne!(active.border.color, focused.border.color);
    }
}

#[test]
fn a_real_text_inputs_value_and_placeholder_are_both_readable() {
    for scheme in EVERY_SCHEME {
        let theme = Theme::new(scheme);
        let class = <Theme as text_input::Catalog>::default();
        let style = text_input::Catalog::style(&theme, &class, text_input::Status::Active);

        let background = color_of(style.background);

        assert!(
            Contrast::body_text(style.value, background),
            "{scheme:?}: field value fails contrast"
        );
        assert!(
            Contrast::body_text(style.placeholder, background),
            "{scheme:?}: placeholder fails contrast (ratio {:.2}) -- a placeholder \
             must stay legible while reading as secondary",
            Contrast::ratio(style.placeholder, background)
        );
    }
}

#[test]
fn a_real_checkbox_carries_selection_in_its_fill() {
    let theme = Theme::new(Scheme::Dark);
    let colors = theme.colors();
    let class = <Theme as checkbox::Catalog>::default();

    let unchecked = checkbox::Catalog::style(
        &theme,
        &class,
        checkbox::Status::Active { is_checked: false },
    );
    let checked = checkbox::Catalog::style(
        &theme,
        &class,
        checkbox::Status::Active { is_checked: true },
    );

    assert_eq!(
        color_of(checked.background),
        colors.primary,
        "a checked box fills with primary"
    );
    assert_ne!(
        color_of(unchecked.background),
        color_of(checked.background),
        "checked and unchecked must be distinguishable"
    );

    // Unchecked is carried by the border, not by colour alone.
    assert!(unchecked.border.width > 0.0);
}

#[test]
fn a_real_radio_is_distinguishable_when_selected() {
    let theme = Theme::new(Scheme::Dark);
    let colors = theme.colors();
    let class = <Theme as radio::Catalog>::default();

    let unselected =
        radio::Catalog::style(&theme, &class, radio::Status::Active { is_selected: false });
    let selected =
        radio::Catalog::style(&theme, &class, radio::Status::Active { is_selected: true });

    assert_eq!(selected.dot_color, colors.primary);
    assert_ne!(
        unselected.border_color, selected.border_color,
        "selection must change the radio's outline"
    );
}

#[test]
fn a_real_progress_bar_uses_the_primary_token_for_its_bar() {
    let theme = Theme::new(Scheme::Dark);
    let colors = theme.colors();
    let class = <Theme as progress_bar::Catalog>::default();

    let style = progress_bar::Catalog::style(&theme, &class);

    assert_eq!(color_of(style.bar), colors.primary);
    assert_eq!(color_of(style.background), colors.surface);
}

#[test]
fn a_card_container_paints_a_surface_but_a_bare_container_does_not() {
    let theme = Theme::new(Scheme::Dark);
    let colors = theme.colors();

    let card = taim::card_style()(&theme);
    assert_eq!(card.background.map(color_of), Some(colors.surface));
    assert_eq!(card.text_color, Some(colors.on_surface));

    // A bare container must stay transparent: giving every container a
    // fill would paint the whole hierarchy into mush.
    let class = <Theme as container::Catalog>::default();
    let bare = container::Catalog::style(&theme, &class);
    assert_eq!(bare.background, None);
}

// ---------------------------------------------------------------------
// Materials, from the token set to a widget's Style
// ---------------------------------------------------------------------

#[test]
fn a_widget_is_opaque_until_a_theme_opts_into_materials() {
    // `MaterialTokens::FLAT` is the default, and it must leave every
    // widget exactly as it was before materials existed -- otherwise
    // "opt-in" is not what this is.
    let theme = Theme::new(Scheme::Dark);

    assert_eq!(theme.materials(), MaterialTokens::FLAT);
    assert_eq!(taim::card_style()(&theme).material, Material::OPAQUE);
    assert_eq!(taim::glass_style()(&theme).material, Material::OPAQUE);

    let class = <Theme as container::Catalog>::default();
    let bare = container::Catalog::style(&theme, &class);
    assert_eq!(bare.material, Material::OPAQUE);
}

#[test]
fn a_physical_theme_reaches_a_widgets_material() {
    // The whole chain in one assertion: token set -> Theme -> the style
    // function a real application calls -> the `Style` a widget draws
    // from. If any link is missing, the material never reaches a pixel.
    let theme = Theme::new(Scheme::Dark).with_materials(MaterialTokens::PHYSICAL);

    assert_eq!(
        taim::glass_style()(&theme).material,
        MaterialTokens::PHYSICAL.overlay,
        "a glass surface must take the theme's overlay material"
    );
    assert_eq!(
        taim::card_style()(&theme).material,
        MaterialTokens::PHYSICAL.surface,
        "a card must take the theme's surface material"
    );

    // And the one that makes it worth doing: an overlay transmits, so it
    // has something behind it to bend.
    assert!(taim::glass_style()(&theme).material.needs_backdrop());
}

#[test]
fn swapping_materials_bumps_the_theme_revision() {
    // Widgets cache against `revision`; a material change that did not
    // move it would leave stale styles on screen.
    let flat = Theme::new(Scheme::Dark);
    let physical = flat.with_materials(MaterialTokens::PHYSICAL);

    assert_ne!(physical.revision(), flat.revision());

    // But an identical set must not: a revision bump invalidates caches,
    // and doing that for a no-op change is a per-frame cost for nothing.
    assert_eq!(flat.with_materials(MaterialTokens::FLAT).revision(), flat.revision());
}

#[test]
fn the_scene_light_reaches_the_renderer_trait_through_the_theme() {
    // `theme::Base::light` is what `winit` calls to drive
    // `Renderer::set_light`. If it did not follow the token set, every
    // surface in the interface would be lit by a constant no designer
    // could move.
    use taim_core::theme::Base as _;

    let flat = Theme::new(Scheme::Dark);
    assert_eq!(flat.light(), SceneLight::KEY);

    let moved = SceneLight {
        direction: [0.6, -0.5, 0.62],
        ..SceneLight::KEY
    };
    let relit = flat.with_materials(MaterialTokens {
        light: moved,
        ..MaterialTokens::PHYSICAL
    });

    assert_eq!(relit.light(), moved);
}

// ---------------------------------------------------------------------
// The "no effects required" promise, at the pixel level
// ---------------------------------------------------------------------

#[test]
fn no_taim_widget_depends_on_a_shadow_to_be_seen() {
    // "Effects are never load-bearing": with every shadow removed, the
    // controls must still be fully distinguishable. Taim enforces this by
    // simply never emitting one -- elevation is an overlay tint.
    let theme = Theme::new(Scheme::Dark);

    let button_class = <Theme as button::Catalog>::default();
    let button_style = button::Catalog::style(&theme, &button_class, button::Status::Active);
    assert_eq!(
        button_style.shadow,
        taim_core::Shadow::default(),
        "a button must not need a shadow"
    );

    let card = taim::card_style()(&theme);
    assert_eq!(
        card.shadow,
        taim_core::Shadow::default(),
        "a card must not need a shadow"
    );
}

// ---------------------------------------------------------------------
// Theme switching reaches the widgets
// ---------------------------------------------------------------------

#[test]
fn switching_the_scheme_changes_what_a_real_widget_paints() {
    // The coherence payoff: one token switch restyles every control, with
    // no per-widget work.
    let class = <Theme as button::Catalog>::default();

    let dark = Theme::new(Scheme::Dark);
    let light = dark.with_scheme(Scheme::Light);

    let dark_style = button::Catalog::style(&dark, &class, button::Status::Active);
    let light_style = button::Catalog::style(&light, &class, button::Status::Active);

    assert_ne!(
        dark_style.background.map(color_of),
        light_style.background.map(color_of),
        "a scheme switch must change what the button paints"
    );
    assert!(light.revision() > dark.revision());
}

// ---------------------------------------------------------------------
// The Taim-native widgets
// ---------------------------------------------------------------------

#[test]
fn a_real_icon_button_renders_taim_tokens() {
    for scheme in EVERY_SCHEME {
        let theme = Theme::new(scheme);
        let colors = theme.colors();
        let class = <Theme as icon_button::Catalog>::default();

        let rest = icon_button::Catalog::style(&theme, &class, icon_button::Status::Active);
        let hovered = icon_button::Catalog::style(&theme, &class, icon_button::Status::Hovered);

        assert_eq!(
            rest.icon, colors.on_surface,
            "{scheme:?}: an icon button's glyph is on_surface"
        );
        assert_eq!(
            rest.background, None,
            "{scheme:?}: at rest an icon button is content, not chrome -- it draws no fill"
        );
        assert!(
            hovered.background.is_some(),
            "{scheme:?}: hover must give it a surface"
        );
    }
}

#[test]
fn a_disabled_icon_button_dims_and_draws_no_fill() {
    let theme = Theme::default();
    let class = <Theme as icon_button::Catalog>::default();

    let rest = icon_button::Catalog::style(&theme, &class, icon_button::Status::Active);
    let disabled = icon_button::Catalog::style(&theme, &class, icon_button::Status::Disabled);

    assert_eq!(
        disabled.background, None,
        "a disabled control must not look pressable"
    );
    // The state layer dims the glyph rather than recolouring it.
    assert_eq!(disabled.icon, rest.icon);
}

#[test]
fn a_real_list_tile_only_takes_a_surface_when_it_needs_one() {
    // A resting list must read as a column of text, not a stack of slabs.
    let theme = Theme::default();
    let class = <Theme as list_tile::Catalog>::default();

    let rest = list_tile::Catalog::style(&theme, &class, list_tile::Status::Active, false);
    let hovered = list_tile::Catalog::style(&theme, &class, list_tile::Status::Hovered, false);
    let selected = list_tile::Catalog::style(&theme, &class, list_tile::Status::Active, true);

    assert_eq!(rest.background, None, "a resting row draws no fill");
    assert!(hovered.background.is_some(), "hover must be visible");
    assert!(
        selected.background.is_some(),
        "a selected row must be distinguishable at rest"
    );
}

#[test]
fn a_selected_list_tile_tints_its_text_with_primary() {
    let theme = Theme::default();
    let colors = theme.colors();
    let class = <Theme as list_tile::Catalog>::default();

    let unselected = list_tile::Catalog::style(&theme, &class, list_tile::Status::Active, false);
    let selected = list_tile::Catalog::style(&theme, &class, list_tile::Status::Active, true);

    assert_eq!(selected.text_color, colors.primary);
    assert_ne!(unselected.text_color, selected.text_color);
}

#[test]
fn a_list_tile_stays_flush_so_a_list_reads_as_a_column() {
    // Rounding a row would break the continuous edge a scannable list
    // depends on. Radius::None resolves to a zero border radius.
    let theme = Theme::default();
    let class = <Theme as list_tile::Catalog>::default();

    let style = list_tile::Catalog::style(&theme, &class, list_tile::Status::Active, false);

    assert_eq!(style.border.radius, 0.0.into());
}

#[test]
fn every_taim_native_widget_is_readable_on_the_surface_it_sits_on() {
    for scheme in EVERY_SCHEME {
        let theme = Theme::new(scheme);
        let surface = theme.colors().surface;

        let icon_class = <Theme as icon_button::Catalog>::default();
        let icon = icon_button::Catalog::style(&theme, &icon_class, icon_button::Status::Active);
        assert!(
            Contrast::body_text(icon.icon, surface),
            "{scheme:?}: an icon button's glyph must be legible on the surface"
        );

        let tile_class = <Theme as list_tile::Catalog>::default();
        for selected in [false, true] {
            let tile =
                list_tile::Catalog::style(&theme, &tile_class, list_tile::Status::Active, selected);
            let background = tile.background.map(color_of).unwrap_or(surface);

            assert!(
                Contrast::body_text(tile.text_color, background),
                "{scheme:?}/selected={selected}: tile text fails contrast (ratio {:.2})",
                Contrast::ratio(tile.text_color, background)
            );
        }
    }
}

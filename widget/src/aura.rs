//! Renders the built-in widgets from Aura's design system.
//!
//! This is the bridge that makes [`aura_design::Theme`] a first-class
//! aura theme: with the `aura` feature enabled, an application builds its
//! `Element`s over `aura_design::Theme` and every widget below resolves
//! its style from Aura's semantic tokens instead of aura's built-in
//! palette.
//!
//! ```ignore
//! use aura_design::Theme;
//! use aura_widget::{button, text};
//!
//! let element: aura_core::Element<'_, (), Theme, Renderer> =
//!     button(text("Save")).into();
//! ```
//!
//! # How a style is resolved
//!
//! Each widget's `Status` (aura's per-widget notion of "what is happening
//! to this control right now") is translated into an
//! [`aura_design::state::Interaction`], handed to the corresponding
//! [`ComponentSpec`]'s `appearance`, and the resulting
//! [`Appearance`](aura_design::components::Appearance) is mapped onto that
//! widget's own `Style` struct. So the *same* state matrix that
//! `tests/design/components.rs` proves is the one real pixels come from --
//! there is no second, parallel styling path to drift out of sync.
//!
//! # Why the direction is `aura_widget` -> `aura-design`
//!
//! `aura-design` is renderer-neutral by design and must stay that way, so
//! it cannot depend on `aura_widget`. The `Catalog` traits are local to
//! this crate and `aura_design::Theme` is a foreign type, which is
//! exactly the orphan-rule shape that lets the impls live here.
//!
//! # Scope
//!
//! All catalogued widgets now have Aura design system implementations:
//! button, text input, checkbox, radio, container, progress bar, icon
//! button, list tile, dialog, app bar, toggler, slider, rule, scrollable,
//! overlay menu, pick list, combo box, text editor, float, SVG, QR code,
//! table, pane grid, and markdown. Every control resolves its style from
//! Aura's semantic tokens — there is no second, parallel styling path.

use crate::core::border;
use crate::core::{Background, Border, Color, Shadow, Vector};

use aura_design::components::button::{Button as AuraButton, ButtonKind};
use aura_design::components::card::Card;
use aura_design::components::dialog::Dialog as AuraDialog;
use aura_design::components::icon_button::IconButton as AuraIconButton;
use aura_design::components::list_tile::{ListLines, ListTile};
use aura_design::components::menu::Menu as AuraMenu;
use aura_design::components::progress::Progress;
use aura_design::components::selection::{Selection, SelectionKind};
use aura_design::components::slider::Slider as AuraSlider;
use aura_design::components::text_field::TextField;
use aura_design::components::{Appearance, ComponentSpec};
use aura_design::icon::Icon;
use aura_design::state::Interaction;
use aura_design::theme::Theme;
use aura_design::tokens::Radius;

use crate::overlay::menu;
use crate::{
    app_bar, button, checkbox, container, dialog, float, icon_button, list_tile, pane_grid,
    pick_list, progress_bar, radio, rule, scrollable, slider, table, text_editor, text_input,
    toggler,
};

/// Converts an Aura [`Radius`] into an aura border radius.
///
/// [`Radius::Full`] has no fixed pixel value (it is a pill, resolved from
/// the control's own height), so it maps to a radius large enough that
/// any realistic control renders fully rounded.
fn border_radius(radius: Radius) -> crate::core::border::Radius {
    radius.pixels().unwrap_or(f32::from(u8::MAX)).into()
}

/// Builds an aura [`Border`] from an [`Appearance`].
fn border_of(appearance: &Appearance) -> Border {
    Border {
        color: appearance.border.unwrap_or(Color::TRANSPARENT),
        width: if appearance.border.is_some() {
            1.0
        } else {
            0.0
        },
        radius: border_radius(appearance.radius),
    }
}

/// Applies an [`Appearance`]'s state layer to `base`.
///
/// Aura expresses hover/press/disabled feedback as one alpha over the
/// control's own colour rather than a separate palette entry, so this is
/// where that single number becomes a real colour.
fn with_state_layer(base: Color, appearance: &Appearance) -> Color {
    if appearance.overlay_alpha == 0.0 {
        return base;
    }

    // A disabled control fades out; hover/press deepen toward the
    // foreground. Both are the same alpha applied in the direction the
    // state means.
    if !appearance.enabled {
        return Color {
            a: base.a * (1.0 - appearance.overlay_alpha),
            ..base
        };
    }

    let overlay = appearance.foreground;
    let amount = appearance.overlay_alpha;

    Color {
        r: base.r + (overlay.r - base.r) * amount,
        g: base.g + (overlay.g - base.g) * amount,
        b: base.b + (overlay.b - base.b) * amount,
        a: base.a,
    }
}

// ---------------------------------------------------------------------
// Button
// ---------------------------------------------------------------------

/// Maps aura's button [`Status`](button::Status) onto an Aura
/// [`Interaction`].
fn button_interaction(status: button::Status) -> Interaction {
    match status {
        button::Status::Active => Interaction::default(),
        button::Status::Hovered => Interaction {
            hovered: true,
            ..Interaction::default()
        },
        button::Status::Pressed => Interaction {
            pressed: true,
            ..Interaction::default()
        },
        button::Status::Disabled => Interaction {
            disabled: true,
            ..Interaction::default()
        },
    }
}

/// The style function for a button of the given [`ButtonKind`].
pub fn button_style(kind: ButtonKind) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let spec = AuraButton {
            kind,
            loading: false,
            disabled: matches!(status, button::Status::Disabled),
        };

        let appearance = spec.appearance(button_interaction(status), theme);

        button::Style {
            background: appearance
                .background
                .map(|color| Background::Color(with_state_layer(color, &appearance))),
            text_color: appearance.foreground,
            border: border_of(&appearance),
            // Aura never uses a shadow to distinguish a control: elevation
            // is an overlay tint, so a control still reads at the Recovery
            // quality tier with every effect disabled.
            shadow: Shadow::default(),
            snap: crate::core::renderer::CRISP,
        }
    }
}

impl button::Catalog for Theme {
    type Class<'a> = button::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(button_style(ButtonKind::Filled))
    }

    fn style(&self, class: &Self::Class<'_>, status: button::Status) -> button::Style {
        class(self, status)
    }
}

// ---------------------------------------------------------------------
// Text input
// ---------------------------------------------------------------------

fn text_input_interaction(status: text_input::Status) -> Interaction {
    match status {
        text_input::Status::Active => Interaction::default(),
        text_input::Status::Hovered => Interaction {
            hovered: true,
            ..Interaction::default()
        },
        text_input::Status::Focused { is_hovered } => Interaction {
            focus_visible: true,
            hovered: is_hovered,
            ..Interaction::default()
        },
        text_input::Status::Disabled => Interaction {
            disabled: true,
            ..Interaction::default()
        },
    }
}

impl text_input::Catalog for Theme {
    type Class<'a> = text_input::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme: &Theme, status| {
            let spec = TextField {
                secure: false,
                multiline: false,
                error: false,
                disabled: matches!(status, text_input::Status::Disabled),
            };

            let interaction = text_input_interaction(status);
            let appearance = spec.appearance(interaction, theme);
            let colors = theme.colors();

            text_input::Style {
                background: Background::Color(appearance.background.unwrap_or(colors.surface)),
                border: border_of(&appearance),
                icon: colors.on_surface_variant,
                // The placeholder is deliberately the *variant* token, not
                // a faded copy of the value colour: it must stay legible
                // while reading as secondary.
                placeholder: colors.on_surface_variant,
                value: appearance.foreground,
                selection: colors.primary,
            }
        })
    }

    fn style(&self, class: &Self::Class<'_>, status: text_input::Status) -> text_input::Style {
        class(self, status)
    }
}

// ---------------------------------------------------------------------
// Checkbox
// ---------------------------------------------------------------------

fn selection_interaction(hovered: bool, checked: bool, disabled: bool) -> Interaction {
    Interaction {
        hovered,
        selected: checked,
        disabled,
        ..Interaction::default()
    }
}

impl checkbox::Catalog for Theme {
    type Class<'a> = checkbox::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme: &Theme, status| {
            let (hovered, checked, disabled) = match status {
                checkbox::Status::Active { is_checked } => (false, is_checked, false),
                checkbox::Status::Hovered { is_checked } => (true, is_checked, false),
                checkbox::Status::Disabled { is_checked } => (false, is_checked, true),
            };

            let spec = Selection {
                kind: SelectionKind::Checkbox,
                checked,
                disabled,
            };

            let interaction = selection_interaction(hovered, checked, disabled);
            let appearance = spec.appearance(interaction, theme);
            let colors = theme.colors();

            checkbox::Style {
                background: Background::Color(
                    appearance
                        .background
                        .map(|color| with_state_layer(color, &appearance))
                        .unwrap_or(Color::TRANSPARENT),
                ),
                icon_color: appearance.foreground,
                border: border_of(&appearance),
                text_color: Some(colors.on_surface),
            }
        })
    }

    fn style(&self, class: &Self::Class<'_>, status: checkbox::Status) -> checkbox::Style {
        class(self, status)
    }
}

// ---------------------------------------------------------------------
// Radio
// ---------------------------------------------------------------------

impl radio::Catalog for Theme {
    type Class<'a> = radio::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme: &Theme, status| {
            // Radio has no Disabled status of its own upstream.
            let (hovered, selected) = match status {
                radio::Status::Active { is_selected } => (false, is_selected),
                radio::Status::Hovered { is_selected } => (true, is_selected),
            };

            let spec = Selection {
                kind: SelectionKind::Radio,
                checked: selected,
                disabled: false,
            };

            let interaction = selection_interaction(hovered, selected, false);
            let appearance = spec.appearance(interaction, theme);
            let colors = theme.colors();

            radio::Style {
                background: Background::Color(Color::TRANSPARENT),
                dot_color: colors.primary,
                border_width: 1.0,
                border_color: appearance.border.unwrap_or(colors.on_surface_variant),
                text_color: Some(colors.on_surface),
            }
        })
    }

    fn style(&self, class: &Self::Class<'_>, status: radio::Status) -> radio::Style {
        class(self, status)
    }
}

// ---------------------------------------------------------------------
// Container (Aura's Card)
// ---------------------------------------------------------------------

/// The style function for a card surface.
pub fn card_style() -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let spec = Card {
            interactive: false,
            selected: false,
        };
        let appearance = spec.appearance(Interaction::default(), theme);

        container::Style {
            text_color: Some(appearance.foreground),
            background: appearance.background.map(Background::Color),
            border: border_of(&appearance),
            // See `button`: elevation is a tint, never a shadow that
            // content depends on.
            shadow: Shadow::default(),
            snap: crate::core::renderer::CRISP,
        }
    }
}

impl container::Catalog for Theme {
    type Class<'a> = container::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        // A bare container is a transparent grouping box, not a card:
        // giving every container a surface fill would paint the whole
        // hierarchy.
        Box::new(|_theme: &Theme| container::Style::default())
    }

    fn style(&self, class: &Self::Class<'_>) -> container::Style {
        class(self)
    }
}

// ---------------------------------------------------------------------
// Progress bar
// ---------------------------------------------------------------------

impl progress_bar::Catalog for Theme {
    type Class<'a> = progress_bar::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme: &Theme| {
            let spec = Progress {
                value: Some(0.0),
                circular: false,
            };
            let appearance = spec.appearance(Interaction::default(), theme);
            let colors = theme.colors();

            progress_bar::Style {
                background: Background::Color(colors.surface),
                bar: Background::Color(colors.primary),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: border_radius(appearance.radius),
                },
            }
        })
    }

    fn style(&self, class: &Self::Class<'_>) -> progress_bar::Style {
        class(self)
    }
}

// ---------------------------------------------------------------------
// Icon button
// ---------------------------------------------------------------------

fn icon_button_interaction(status: icon_button::Status) -> Interaction {
    match status {
        icon_button::Status::Active => Interaction::default(),
        icon_button::Status::Hovered => Interaction {
            hovered: true,
            ..Interaction::default()
        },
        icon_button::Status::Pressed => Interaction {
            pressed: true,
            ..Interaction::default()
        },
        icon_button::Status::Disabled => Interaction {
            disabled: true,
            ..Interaction::default()
        },
    }
}

impl icon_button::Catalog for Theme {
    type Class<'a> = icon_button::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme: &Theme, status| {
            let spec = AuraIconButton {
                icon: Icon::new("icon", 24, false),
                // The widget owns the real label; the spec only needs a
                // non-empty one to resolve its appearance.
                tooltip: Some("icon"),
                disabled: matches!(status, icon_button::Status::Disabled),
            };

            let interaction = icon_button_interaction(status);
            let appearance = spec.appearance(interaction, theme);
            let colors = theme.colors();

            // At rest an icon button is content, not chrome: no fill at
            // all, so a row of them reads as icons rather than boxes. The
            // state layer is the only thing that gives it a surface.
            let background = (appearance.overlay_alpha > 0.0 && appearance.enabled)
                .then(|| Background::Color(with_state_layer(colors.surface, &appearance)));

            icon_button::Style {
                background,
                icon: appearance.foreground,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: border_radius(appearance.radius),
                },
            }
        })
    }

    fn style(&self, class: &Self::Class<'_>, status: icon_button::Status) -> icon_button::Style {
        class(self, status)
    }
}

// ---------------------------------------------------------------------
// List tile
// ---------------------------------------------------------------------

impl list_tile::Catalog for Theme {
    type Class<'a> = list_tile::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme: &Theme, status, selected| {
            let spec = ListTile {
                lines: ListLines::One,
                interactive: !matches!(status, list_tile::Status::Inert),
            };

            let interaction = Interaction {
                hovered: matches!(status, list_tile::Status::Hovered),
                pressed: matches!(status, list_tile::Status::Pressed),
                selected,
                ..Interaction::default()
            };

            let appearance = spec.appearance(interaction, theme);
            let colors = theme.colors();

            // A selected row is tinted with `primary`; an unselected one
            // only takes a surface while hovered or pressed, so a resting
            // list is a clean column of text rather than a stack of slabs.
            let background = if selected {
                Some(Background::Color(with_state_layer(
                    colors.surface,
                    &appearance,
                )))
            } else {
                (appearance.overlay_alpha > 0.0)
                    .then(|| Background::Color(with_state_layer(colors.surface, &appearance)))
            };

            list_tile::Style {
                background,
                text_color: appearance.foreground,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    // Tiles sit flush inside a list; rounding them would
                    // break the continuous edge a scannable list needs.
                    radius: border_radius(appearance.radius),
                },
            }
        })
    }

    fn style(
        &self,
        class: &Self::Class<'_>,
        status: list_tile::Status,
        selected: bool,
    ) -> list_tile::Style {
        class(self, status, selected)
    }
}

// ---------------------------------------------------------------------
// Dialog
// ---------------------------------------------------------------------

impl dialog::Catalog for Theme {
    type Class<'a> = dialog::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme: &Theme| {
            let spec = AuraDialog {
                dismissible: true,
                focus_trapped: true,
            };
            let appearance = spec.appearance(Interaction::default(), theme);
            let colors = theme.colors();

            dialog::Style {
                // A scrim is a dimming, not a blur: the page behind must
                // read as unavailable with every expensive effect
                // disabled, which is the Recovery tier's whole premise.
                scrim: Background::Color(Color {
                    a: 0.4,
                    ..Color::BLACK
                }),
                background: Background::Color(colors.surface),
                text_color: colors.on_surface,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: border_radius(appearance.radius),
                },
            }
        })
    }

    fn style(&self, class: &Self::Class<'_>) -> dialog::Style {
        class(self)
    }
}

// ---------------------------------------------------------------------
// App bar
// ---------------------------------------------------------------------

impl app_bar::Catalog for Theme {
    type Class<'a> = app_bar::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme: &Theme| {
            let colors = theme.colors();

            app_bar::Style {
                // The bar meets the window edge and reads as chrome, so it
                // takes the `background` token rather than a raised
                // `surface`: elevation here would be decoration.
                background: Some(Background::Color(colors.background)),
                text_color: colors.on_surface,
            }
        })
    }

    fn style(&self, class: &Self::Class<'_>) -> app_bar::Style {
        class(self)
    }
}

// theme::Base and text::Catalog impls live in aura_design::theme
// (orphan rule: Theme is defined there, not here).

// ---------------------------------------------------------------------
// Toggler
// ---------------------------------------------------------------------

impl toggler::Catalog for Theme {
    type Class<'a> = toggler::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme: &Theme, status| {
            let colors = theme.colors();

            let (hovered, toggled, disabled) = match status {
                toggler::Status::Active { is_toggled } => (false, is_toggled, false),
                toggler::Status::Hovered { is_toggled } => (true, is_toggled, false),
                toggler::Status::Disabled { is_toggled } => (false, is_toggled, true),
            };

            let spec = Selection {
                kind: SelectionKind::Checkbox,
                checked: toggled,
                disabled,
            };

            let interaction = Interaction {
                hovered,
                selected: toggled,
                disabled,
                ..Interaction::default()
            };

            let appearance = spec.appearance(interaction, theme);

            // A toggler is a switch: toggled-on gets the primary fill,
            // toggled-off gets a subdued surface, just like Material 3's
            // switch track.
            let background = if toggled {
                with_state_layer(colors.primary, &appearance)
            } else {
                with_state_layer(colors.on_surface_variant, &appearance)
            };

            let foreground = if toggled {
                colors.on_primary
            } else if disabled {
                Color {
                    a: 0.38,
                    ..colors.surface
                }
            } else {
                colors.surface
            };

            toggler::Style {
                background: background.into(),
                foreground: foreground.into(),
                foreground_border_width: 0.0,
                foreground_border_color: Color::TRANSPARENT,
                background_border_width: 0.0,
                background_border_color: Color::TRANSPARENT,
                text_color: Some(colors.on_surface),
                border_radius: None,
                padding_ratio: 0.1,
            }
        })
    }

    fn style(&self, class: &Self::Class<'_>, status: toggler::Status) -> toggler::Style {
        class(self, status)
    }
}

// ---------------------------------------------------------------------
// Slider
// ---------------------------------------------------------------------

impl slider::Catalog for Theme {
    type Class<'a> = slider::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme: &Theme, status| {
            let colors = theme.colors();

            let spec = AuraSlider {
                value: 50.0,
                range: (0.0, 100.0),
                rtl: false,
                disabled: false,
            };

            let interaction = match status {
                slider::Status::Active => Interaction::default(),
                slider::Status::Hovered => Interaction {
                    hovered: true,
                    ..Interaction::default()
                },
                slider::Status::Dragged => Interaction {
                    pressed: true,
                    ..Interaction::default()
                },
            };

            let appearance = spec.appearance(interaction, theme);

            // Active track uses primary, inactive track uses
            // on_surface_variant at low opacity — the slider's filled
            // vs. unfilled visual.
            let active_color = with_state_layer(colors.primary, &appearance);

            slider::Style {
                rail: slider::Rail {
                    backgrounds: (
                        active_color.into(),
                        Background::Color(Color {
                            a: 0.38,
                            ..colors.on_surface_variant
                        }),
                    ),
                    width: 4.0,
                    border: Border {
                        radius: border_radius(Radius::Full),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                },
                handle: slider::Handle {
                    shape: slider::HandleShape::Circle { radius: 10.0 },
                    background: active_color.into(),
                    border_color: Color::TRANSPARENT,
                    border_width: 0.0,
                },
            }
        })
    }

    fn style(&self, class: &Self::Class<'_>, status: slider::Status) -> slider::Style {
        class(self, status)
    }
}

// ---------------------------------------------------------------------
// Rule (divider / separator)
// ---------------------------------------------------------------------

impl rule::Catalog for Theme {
    type Class<'a> = rule::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme: &Theme| {
            let colors = theme.colors();

            rule::Style {
                // A separator is the on_surface_variant at reduced opacity:
                // visible enough to create hierarchy, quiet enough to not
                // compete with content.
                color: Color {
                    a: 0.12,
                    ..colors.on_surface_variant
                },
                radius: 0.0.into(),
                fill_mode: rule::FillMode::Full,
                snap: true,
            }
        })
    }

    fn style(&self, class: &Self::Class<'_>) -> rule::Style {
        class(self)
    }
}

// ---------------------------------------------------------------------
// Scrollable
// ---------------------------------------------------------------------

impl scrollable::Catalog for Theme {
    type Class<'a> = scrollable::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme: &Theme, _status| {
            let colors = theme.colors();

            let rail = scrollable::Rail {
                background: Some(Background::Color(Color {
                    a: 0.08,
                    ..colors.on_surface
                })),
                border: border::rounded(2),
                scroller: scrollable::Scroller {
                    background: Background::Color(Color {
                        a: 0.38,
                        ..colors.on_surface_variant
                    }),
                    border: border::rounded(2),
                },
            };

            scrollable::Style {
                container: container::Style::default(),
                vertical_rail: rail,
                horizontal_rail: rail,
                gap: None,
                auto_scroll: scrollable::AutoScroll {
                    background: Background::Color(Color {
                        a: 0.9,
                        ..colors.surface
                    }),
                    border: border::rounded(u32::MAX).width(1).color(Color {
                        a: 0.2,
                        ..colors.on_surface
                    }),
                    shadow: Shadow {
                        color: Color {
                            a: 0.3,
                            ..Color::BLACK
                        },
                        offset: Vector::ZERO,
                        blur_radius: 2.0,
                    },
                    icon: colors.on_surface_variant,
                },
            }
        })
    }

    fn style(&self, class: &Self::Class<'_>, status: scrollable::Status) -> scrollable::Style {
        class(self, status)
    }
}

// ---------------------------------------------------------------------
// Overlay Menu
// ---------------------------------------------------------------------

impl menu::Catalog for Theme {
    type Class<'a> = menu::StyleFn<'a, Self>;

    fn default<'a>() -> <Self as menu::Catalog>::Class<'a> {
        Box::new(|theme: &Theme| {
            let colors = theme.colors();

            let spec = AuraMenu {
                expanded: true,
                has_submenu: false,
            };

            let appearance = spec.appearance(Interaction::default(), theme);

            menu::Style {
                background: Background::Color(colors.surface),
                border: Border {
                    color: Color {
                        a: 0.12,
                        ..colors.on_surface_variant
                    },
                    width: 1.0,
                    radius: border_radius(appearance.radius),
                },
                text_color: colors.on_surface,
                selected_text_color: colors.on_primary,
                selected_background: Background::Color(colors.primary),
                // Aura never uses shadow to distinguish controls, but a
                // floating menu benefits from the subtle depth cue.
                shadow: Shadow::default(),
            }
        })
    }

    fn style(&self, class: &<Self as menu::Catalog>::Class<'_>) -> menu::Style {
        class(self)
    }
}

// ---------------------------------------------------------------------
// Pick List
// ---------------------------------------------------------------------

impl pick_list::Catalog for Theme {
    type Class<'a> = pick_list::StyleFn<'a, Self>;

    fn default<'a>() -> <Self as pick_list::Catalog>::Class<'a> {
        Box::new(|theme: &Theme, status| {
            let colors = theme.colors();

            let interaction = match status {
                pick_list::Status::Active => Interaction::default(),
                pick_list::Status::Hovered => Interaction {
                    hovered: true,
                    ..Interaction::default()
                },
                pick_list::Status::Opened { is_hovered } => Interaction {
                    focus_visible: true,
                    hovered: is_hovered,
                    ..Interaction::default()
                },
                pick_list::Status::Disabled => Interaction {
                    disabled: true,
                    ..Interaction::default()
                },
            };

            let spec = TextField {
                secure: false,
                multiline: false,
                error: false,
                disabled: matches!(status, pick_list::Status::Disabled),
            };

            let appearance = spec.appearance(interaction, theme);

            pick_list::Style {
                text_color: appearance.foreground,
                placeholder_color: colors.on_surface_variant,
                handle_color: colors.on_surface_variant,
                background: Background::Color(appearance.background.unwrap_or(colors.surface)),
                border: border_of(&appearance),
            }
        })
    }

    fn style(
        &self,
        class: &<Self as pick_list::Catalog>::Class<'_>,
        status: pick_list::Status,
    ) -> pick_list::Style {
        class(self, status)
    }
}

// ---------------------------------------------------------------------
// Combo Box (marker trait — supertraits text_input + menu are sufficient)
// ---------------------------------------------------------------------

impl crate::combo_box::Catalog for Theme {}

// ---------------------------------------------------------------------
// Text Editor (supertrait: theme::Base)
// ---------------------------------------------------------------------

impl text_editor::Catalog for Theme {
    type Class<'a> = text_editor::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme: &Theme, status| {
            let spec = TextField {
                secure: false,
                multiline: true,
                error: false,
                disabled: matches!(status, text_editor::Status::Disabled),
            };

            let interaction = match status {
                text_editor::Status::Active => Interaction::default(),
                text_editor::Status::Hovered => Interaction {
                    hovered: true,
                    ..Interaction::default()
                },
                text_editor::Status::Focused { is_hovered } => Interaction {
                    focus_visible: true,
                    hovered: is_hovered,
                    ..Interaction::default()
                },
                text_editor::Status::Disabled => Interaction {
                    disabled: true,
                    ..Interaction::default()
                },
            };

            let appearance = spec.appearance(interaction, theme);
            let colors = theme.colors();

            text_editor::Style {
                background: Background::Color(appearance.background.unwrap_or(colors.surface)),
                border: border_of(&appearance),
                placeholder: colors.on_surface_variant,
                value: appearance.foreground,
                selection: colors.primary,
            }
        })
    }

    fn style(&self, class: &Self::Class<'_>, status: text_editor::Status) -> text_editor::Style {
        class(self, status)
    }
}

// ---------------------------------------------------------------------
// Float
// ---------------------------------------------------------------------

impl float::Catalog for Theme {
    type Class<'a> = float::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        // A float is a transparent positioning layer — no visual style by
        // default, just like a bare container.
        Box::new(|_theme: &Theme| float::Style {
            shadow: Shadow::default(),
            shadow_border_radius: 0.0.into(),
        })
    }

    fn style(&self, class: &Self::Class<'_>) -> float::Style {
        class(self)
    }
}

// ---------------------------------------------------------------------
// SVG
// ---------------------------------------------------------------------

#[cfg(feature = "svg")]
impl crate::svg::Catalog for Theme {
    type Class<'a> = crate::svg::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme: &Theme, _status| {
            let colors = theme.colors();

            // Symbolic SVG icons are tinted with the surface content
            // colour — the same token icons and text use.
            crate::svg::Style {
                color: Some(colors.on_surface),
            }
        })
    }

    fn style(&self, class: &Self::Class<'_>, status: crate::svg::Status) -> crate::svg::Style {
        class(self, status)
    }
}

// ---------------------------------------------------------------------
// QR Code
// ---------------------------------------------------------------------

#[cfg(feature = "qr_code")]
impl crate::qr_code::Catalog for Theme {
    type Class<'a> = crate::qr_code::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme: &Theme| {
            let colors = theme.colors();

            crate::qr_code::Style {
                cell: colors.on_surface,
                background: colors.surface,
            }
        })
    }

    fn style(&self, class: &Self::Class<'_>) -> crate::qr_code::Style {
        class(self)
    }
}

// ---------------------------------------------------------------------
// Table
// ---------------------------------------------------------------------

impl table::Catalog for Theme {
    type Class<'a> = table::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme: &Theme| {
            let colors = theme.colors();

            // Table separators are the same on_surface_variant at low opacity
            // that rules use — visually consistent hierarchy dividers.
            let separator = Background::Color(Color {
                a: 0.12,
                ..colors.on_surface_variant
            });

            table::Style {
                separator_x: separator,
                separator_y: separator,
            }
        })
    }

    fn style(&self, class: &Self::Class<'_>) -> table::Style {
        class(self)
    }
}

// ---------------------------------------------------------------------
// Pane Grid (supertrait: container::Catalog)
// ---------------------------------------------------------------------

impl pane_grid::Catalog for Theme {
    type Class<'a> = pane_grid::StyleFn<'a, Self>;

    fn default<'a>() -> pane_grid::StyleFn<'a, Self> {
        Box::new(|theme: &Theme| {
            let colors = theme.colors();

            pane_grid::Style {
                hovered_region: pane_grid::Highlight {
                    background: Background::Color(Color {
                        a: 0.1,
                        ..colors.primary
                    }),
                    border: Border {
                        color: colors.primary,
                        width: 2.0,
                        radius: border_radius(Radius::Medium),
                    },
                },
                picked_split: pane_grid::Line {
                    color: colors.primary,
                    width: 2.0,
                },
                hovered_split: pane_grid::Line {
                    color: Color {
                        a: 0.6,
                        ..colors.primary
                    },
                    width: 2.0,
                },
            }
        })
    }

    fn style(&self, class: &<Self as pane_grid::Catalog>::Class<'_>) -> pane_grid::Style {
        class(self)
    }
}

// text::Catalog impl lives in aura_design::theme (orphan rule).

// ---------------------------------------------------------------------
// Markdown (supertrait: container + scrollable + text + rule +
//           checkbox + table — all now implemented)
// ---------------------------------------------------------------------

#[cfg(feature = "markdown")]
impl crate::markdown::Catalog for Theme {
    fn code_block<'a>() -> <Self as container::Catalog>::Class<'a> {
        Box::new(|theme: &Theme| {
            let colors = theme.colors();

            container::Style {
                text_color: Some(colors.on_surface),
                background: Some(Background::Color(Color {
                    a: 0.08,
                    ..colors.on_surface
                })),
                border: Border {
                    color: Color {
                        a: 0.12,
                        ..colors.on_surface_variant
                    },
                    width: 1.0,
                    radius: border_radius(Radius::Small),
                },
                shadow: Shadow::default(),
                snap: crate::core::renderer::CRISP,
            }
        })
    }
}

# The Aura visual language

> Status: Phase 11 baseline. This document describes the principles the
> design system commits to, the tokens that encode them, how the
> component contract enforces them, and what still needs human eyes.

## Principles

Aura's beauty comes from typography, hierarchy, spacing, colour,
material, and motion consistency — not from effects. Stated as rules the
system can actually hold itself to:

1. **Semantic tokens only.** A component never names a colour, a gap, or
   a duration; it names a *role* (`primary`, `on_surface`, `Spacing::MD`,
   `TransitionKind::Feedback`) and the theme decides. This is what makes
   a theme switch coherent instead of a patchwork.
2. **Effects are never load-bearing.** Blur, shadow, and noise may enrich
   a surface, but nothing — content, state, control boundaries, focus —
   may depend on them. The Recovery quality tier disables every expensive
   effect (see `docs/performance/effect-budgets.md`); the interface must
   remain fully usable there.
3. **State is visible without colour alone.** Selection is carried by
   fill *and* outline, checkedness by shape as well as tint, errors by a
   border rather than a glow.
4. **Every control is reachable by every input.** Keyboard, pointer,
   touch, and gamepad. A pointer-only affordance is a defect.
5. **Reduced motion is a substitution, not an off switch.** Spatial
   motion becomes a fade; a busy indicator keeps saying "busy" rather
   than freezing into "stalled".

## Tokens

| Family | Type | What it fixes |
|---|---|---|
| Colour | `tokens::ColorTokens` | Semantic roles (`primary`, `on_primary`, `surface`, `on_surface`, `on_surface_variant`, `error`, `focus`) per `Scheme::{Light, Dark, HighContrast}`. |
| Typography | `tokens::Typography` | A type ramp with script-aware line metrics, so Devanagari/Arabic are not clipped by Latin-derived leading. |
| Spacing | `tokens::Spacing` | Exactly six values: 4, 8, 12, 16, 24, 32. |
| Radius | `tokens::Radius` | `None`/`Small`/`Medium`/`Large`/`Full` — 0, 6, 12, 20, and pill. |
| Elevation | `tokens::Elevation` | `Flat`/`Raised`/`Floating`/`Modal`, expressed as an overlay tint *plus* an optional shadow, so hierarchy survives with shadows off. |
| Motion | `tokens::MotionTokens` | `Feedback` (120ms), `Hierarchy` (240ms), and a gesture spring, tied to `aura-motion`. |

The scales are pinned by `tests/golden/design_system.rs`: changing a
token value is a deliberate act that updates an approved oracle, exactly
like re-approving a golden image.

### Contrast

`tokens::Contrast` implements the WCAG relative-luminance ratio.
`Contrast::body_text` is the 4.5:1 bar. Every scheme's core pairings
(`on_surface`/`surface`, `on_primary`/`primary`,
`on_surface_variant`/`surface`, `on_surface`/`background`) are tested
against it, and `HighContrast` must additionally clear the stricter AAA
7.0 bar — otherwise it would be merely a different palette rather than a
more legible one.

## The component contract

Every component implements `components::ComponentSpec`, which turns
`phase.md`'s completion contract into types rather than prose:

| Contract clause | Encoded as |
|---|---|
| semantic role/actions | `role()`, `actions()` |
| state matrix | `supported_states()` + `appearance(Interaction, &Theme)` |
| keyboard/pointer/touch/gamepad | `input_methods() -> InputMethods` |
| minimum touch target | `min_size(Density)` vs `MIN_TOUCH_TARGET` (48px, re-exported from `aura_core::touch::MIN_TARGET`; `TouchTargetPolicy::PlatformCompatibility` is the only way to 44px) |
| focus ring | `Appearance::focus_ring` |
| compact/medium/expanded layout | `layout(ViewportClass, Density) -> LayoutSpec` |
| tokens only | every `Appearance`/`LayoutSpec` field is a theme token |
| readable with no effects | `readable_without_effects()` |
| RTL / large text | `supports_rtl()`, `supports_large_text()` |

`tests/design/components.rs` drives **every** component through **every**
state in **every** scheme generically. A new component is covered the
moment it joins the registry, and cannot quietly skip a clause:

- a disabled component must expose **no** actions (enforced in semantics,
  not merely painted — an inert control still advertising `ACTIVATE` is a
  trap for assistive technology);
- a focusable component must show a focus ring when focus is visible, and
  must **not** when it is not;
- an interactive component must meet the touch target at every density
  and accept every input method;
- a non-interactive component must declare no inputs and take no focus;
- padding/gap must literally be values from the spacing scale;
- a component claiming `SELECTED` must actually look different selected.

The sweeps carry vacuity guards: if a refactor ever made every case
skippable, the coverage test fails rather than passing while checking
nothing.

## Notable component rules

- **Button.** A *loading* button stays focusable but drops `ACTIVATE`: a
  busy control must not eject a keyboard user, nor accept a second
  activation.
- **App bar / window chrome.** These are containers, not controls. They
  expose no actions; their affordances are real `IconButton`s with their
  own roles, focus, and touch targets. (Modelling the bar itself as
  activatable would claim an action with no focusable target behind it.)
- **Window chrome.** Reserves a `SYSTEM_GESTURE_INSET` band where the
  platform's own gestures win. `may_own_drag_at` is the application-facing
  half of "system-edge gestures remain reserved by policy" — the other
  half is `aura_gesture::arena::Priority::SYSTEM` (see
  `docs/architecture/input-gestures.md`).
- **Bottom sheet.** Snaps to the nearest `Anchor`; a non-dismissible
  sheet never snaps closed, so a drag cannot dismiss what the caller said
  cannot be dismissed.
- **Notification.** `Privacy::{Public, Sensitive, Secret}` is a security
  boundary, not styling: it decides what a locked screen may reveal.
- **Toast.** Announces as a live region and never takes focus — which is
  exactly why an *actionable* toast gets the longer visible duration: the
  user must notice, read, and reach it.
- **Icon button.** An icon-only control has no visible label, so its
  accessible name is mandatory. `has_accessible_name()` rejects
  whitespace.
- **Tooltip.** Supplementary by definition; it is never the only home for
  information. `reachable_without_pointer()` distinguishes a tooltip that
  touch and keyboard users can actually get to.
- **Slider.** The one control whose *direction* mirrors in RTL, not just
  its layout: the maximum sits at the left.

## Anti-patterns

- Naming a literal colour, gap, or duration in a component.
- Using elevation/blur to distinguish a control from its background.
- Communicating state with colour alone.
- A focus ring that is conditional on the pointer being absent.
- A control that only a pointer can operate.
- Reduced motion implemented as `duration = 0` globally.
- A "disabled" look without disabled semantics.

## What Phase 11 does not close

## Reaching real widgets

`aura-design` is renderer-neutral and cannot depend on `aura_widget`. The
bridge therefore lives in `aura_widget::aura` (behind the `aura` feature,
re-exported as `aura`'s own `aura` feature), which implements aura's
`Catalog` traits for `aura_design::Theme`. `Catalog` is local to
`aura_widget` and `Theme` is foreign, which is exactly the orphan-rule
shape that permits it.

With the feature on, an application builds its `Element`s over
`aura_design::Theme` and every covered widget resolves its style from
Aura tokens:

```rust,ignore
let element: Element<'_, Message, aura_design::Theme, Renderer> =
    button(text("Save")).into();   // paints Aura's `primary`
```

Each widget's `Status` maps to an `Interaction`, goes through the same
`ComponentSpec::appearance` the state matrix tests, and the resulting
`Appearance` maps onto that widget's `Style`. **There is no second
styling path** — real pixels come from the resolver the tests prove.

Covered: `button`, `text_input`, `checkbox`, `radio`, `container`,
`progress_bar`. A widget with no Aura counterpart (`qr_code`,
`pane_grid`) is a *compile error* against `aura_design::Theme` rather
than a silently mis-styled control.

`tests/design/widgets.rs` proves this end-to-end: a real button paints
`primary` and passes contrast in every scheme; its state layer deepens
rest → hover → press; disabled fades without changing hue; a focused
field borders with the `focus` token; a scheme switch restyles every
control.

## What Phase 11 does not close

| Clause | Status | Evidence |
|---|---|---|
| "all production components pass the state matrix" | **Met** | `tests/design/components.rs` sweeps 17 components × 8 states × 3 schemes, plus per-component contracts. |
| "contrast and touch targets pass" | **Met** | 357 contrast assertions across the matrix; touch targets checked at every density; core pairings and the AAA bar pinned in the golden oracle. `tests/design/widgets.rs` re-checks contrast *after* the trip through each widget's own `Style`. |
| "performance remains within global gates" | **Met (by construction)** | `aura-design` is renderer-neutral data and pure functions: no draw calls, no per-frame allocation, no effects. |
| "design golden suite is reviewed at phone/tablet/desktop/watch profiles" | **Partially met** | The *contract* is pinned deterministically per viewport/density, and phone/tablet/desktop map to `ViewportClass::{Compact, Medium, Expanded}`. There is **no watch profile**, and no rendered output at any profile — these are policy oracles, not pixels. |
| "visual review finds one coherent language rather than individually styled widgets" | **Not met — requires human eyes** | Coherence is *structurally* enforced (one token set, one state resolver, one contract; tests reject off-token values; widgets now demonstrably render from it). But whether the result is beautiful is a judgement no test can make. Explicitly outstanding. |

## Follow-ups carried forward

1. **Extend widget coverage.** Six widgets render from Aura today
   (`button`, `text_input`, `checkbox`, `radio`, `container`,
   `progress_bar`). The remaining catalogued widgets — `toggler`,
   `slider`, `scrollable`, `pick_list`, `combo_box`, `text_editor`,
   `rule`, `table`, `pane_grid`, and the rest — have no
   `aura_design::Theme` impl yet, so an application using one must stay
   on `aura::Theme`.
2. **Components with no aura counterpart** (`dialog`, `bottom_sheet`,
   `menu`, `toast`, `notification`, `navigation`, `app_bar`,
   `list_tile`, `window_chrome`, `tooltip`, `icon_button`) are still
   contracts only: they specify role/state/layout but no widget renders
   them. These need real widget implementations, not just catalogs.
3. A watch/`ViewportClass` profile, if VYOM targets one.
4. Human visual review of the whole component set, at every profile and
   scheme, against the "one coherent language" bar.
5. Icon assets: `assets/icons/manifest.toml` and
   `scripts/design/check-icons.sh` validate names/grids/licences, but no
   SVG geometry is checked because no icons ship yet.

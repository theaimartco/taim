# The TAIM API

Status: **accepted direction, migration in progress** (2026-10-22).
Owner decision: replace the inherited Elm-style API with a TAIM-native one,
accepting a breaking change for every widget and application.

## Why

TAIM inherited its public API from its fork origin: an application is a
`Message` enum plus `update` and `view`, and a widget implements
`Widget<Message, Theme, Renderer>` with seven-argument methods (`tree`,
`layout`, `renderer`, `theme`, `style`, `cursor`, `viewport`…). Even with
every line rewritten, code written against that API reads like the
project it came from. TAIM already has the pieces for a different model
— fine-grained signals (`taim-signal`), explicit invalidation
(`StateDelta`, `FrameGate`), a retained semantic tree — but the API hides
them behind the old shape.

## Goals

1. **State is signals, not messages.** No `Message` type parameter
   anywhere. Event handlers are closures that change signals.
2. **Only what changed is rebuilt.** A view function re-runs when a signal
   it read changes; everything else keeps its layout and paint.
3. **One small widget contract.** A `View` trait with no generic
   parameters, whose methods take a context object instead of positional
   arguments.
4. **Styling through tokens.** Widgets read `taim-design` tokens from the
   theme; no per-widget `Catalog`/`StyleFn` generics.
5. **Accessibility and hit-testing are part of the contract,** not opt-in
   extras with silent defaults.

Non-goals: source compatibility with existing applications, and changing
renderer or windowing backends. The former compatibility host has been
removed; historical applications still require migration.

## Applications

```rust
use taim::prelude::*;

fn main() -> taim::Result {
    taim::launch("Counter", |cx| {
        let count = cx.signal(0);

        column(views![
            text(move || format!("Count: {}", count.get())),
            button("+1").on_press(move || count.update(|n| *n += 1)),
        ]
    })
}
```

- `taim::launch(title, root)` opens a window and calls `root` **once**.
  `root` builds the view tree and creates the signals it needs.
- `cx.signal(value)` creates a [`Signal`] owned by the window. Reading it
  inside a reactive closure (`move || …`) subscribes that closure.
- Event handlers are plain closures (`on_press(move || …)`). They change
  signals; the runtime turns those changes into the minimal
  `StateDelta` (layout, paint, or nothing).
- Async work: `cx.spawn(async move { … })` starts work owned by this
  window. Event handlers capture `let tasks = cx.tasks()` and call
  `tasks.spawn(async move { … })`. Closing the window cancels pending
  futures; a poll already executing retains the state until it returns.
  Cancellation is cooperative, so blocking work should still be bounded.
  The free `spawn` function is detached and should not capture window state.
- Stream subscriptions and multiple-window handles are planned API, not
  implemented `Scope` methods.

The old entry points (`taim::application`, `taim::run_app`,
`taim::native::app`) and the program crate have been removed. The remaining
compatibility discussion below is historical migration design, not an
available host. New applications use `launch` or `app(...).run()`.

## Views

```rust
pub trait View: 'static {
    /// How this view wants to be sized along each axis.
    fn sizing(&self) -> Sizing { Sizing::HUG }

    /// Measures the view under `constraints` and places its children.
    fn measure(&mut self, cx: &mut MeasureCx, constraints: Constraints) -> Size;

    /// Draws the view.
    fn paint(&self, cx: &mut PaintCx);

    /// Reacts to input. Returns whether the event was consumed.
    fn event(&mut self, cx: &mut EventCx, event: &Event) -> Handled { Handled::No }

    /// Describes the view to assistive technology.
    fn describe(&self, cx: &DescribeCx) -> Option<semantics::Node>;

    /// Visits child views (layout, paint, events, semantics, hit-testing
    /// all reach children through this one method).
    fn children(&mut self, visit: &mut dyn FnMut(&mut dyn View)) {}
}
```

Differences from the inherited `Widget` trait:

| Inherited `Widget<M, T, R>` | TAIM `View` |
|---|---|
| Three type parameters | None: the renderer is the compile-time selected `taim_renderer::Renderer`; the theme is `taim::Theme` |
| Messages published through `Shell<'_, Message>` | Handlers are closures captured by the view |
| State in a separate `Tree` with `Tag`/`State` downcasting | State lives in the view; the runtime keeps views alive across frames and reconciles by key |
| `size() -> Size<Length>` + `layout(tree, renderer, limits) -> Node` | `sizing()` + `measure(cx, constraints) -> Size`; child placement is recorded through `cx` |
| `draw(tree, renderer, theme, style, layout, cursor, viewport)` | `paint(cx)`; `cx` carries bounds, theme tokens, pointer, clip |
| `semantics` defaults to `None` (invisible to screen readers) | `describe` has **no default**: every view must answer, even if the answer is `None` |
| `visit_children` must be kept in step with five other traversals by hand | `children` is the only traversal a container writes |

`AnyView` is the boxed form (`Box<dyn View>`); every built-in view
converts into it with `.into()`.

### Context objects

- `MeasureCx`: text measurement, the theme's spacing tokens, and
  `place(child, origin)` to record where a child goes.
- `PaintCx`: the frame's painter (quads, text, images, layers), this
  view's bounds, theme tokens, pointer position and clip rectangle.
- `EventCx`: bounds, pointer, focus (`request_focus`, `has_focus`),
  pointer capture, clipboard, timers and `request_paint` /
  `request_layout`.
- `DescribeCx`: bounds and focus, for building the semantic node.

Contexts are the only way a view talks to the runtime, which is what lets
the runtime decide what to invalidate instead of each widget guessing.

### Layout

- `Sizing { width: Axis, height: Axis }` with `Axis::Hug`,
  `Axis::Fixed(px)`, `Axis::Fill { weight }`.
- `Constraints { min: Size, max: Size }`.
- Rows and columns use the TAIM flex solver already in
  `core::layout::flex` (water-filling, deterministic order).

### Reactivity

`text(move || …)`, `button(…).enabled(move || …)` and similar accept
either a value or a closure. A closure becomes a `taim_signal::Memo`; when
it changes, only the owning view is marked dirty with the delta the
property implies (text content: layout; color: paint).

### Styling

`taim::Theme` exposes `taim-design` tokens (`color.surface`,
`space.md`, `radius.sm`, `type.body` …). Views take optional overrides
(`.background(token_or_color)`); there are no style closures or catalog
traits.

## Migration plan

Each phase leaves `main` building with all tests passing.

| Phase | Work | Done when |
|---|---|---|
| 1. Foundation | New crate `taim-view`: `View`, `AnyView`, contexts, `Sizing`/`Constraints`, signal-backed props. A host that runs `View`s inside the existing runtime (an adapter implementing the inherited `Widget` trait). `taim::launch`. Views: `text`, `button`, `column`, `row`, `container`, `space`. | A counter written only against the new API runs under Xvfb and passes tests |
| 2. Core views | Port the input and layout views: `text_input`, `checkbox`, `toggler`, `slider`, `scrollable`, `stack`, `image`, `svg`, `rule`, `progress_bar` | Showcase pages for these use the new API only |
| 3. Mobile and advanced views | `tabs`, `segmented_control`, `bottom_sheet`, `drawer`, `snackbar`, `carousel`, `pull_to_refresh`, `pane_grid`, `text_editor`, `markdown`, `canvas`, `shader` | `mobile_shell` and `taim-showcase` use the new API only |
| 4. Runtime switch | The runtime drives `View` trees directly (no adapter); `Message`-based `App`/`Program`, `Element`, `Widget`, `Shell`, `Task`-returning `update` removed | No inherited API remains in `core`, `widget`, `runtime`, `program` |
| 5. Clean-up | Delete the compatibility host; rename crates/modules that still carry the old structure; final line-overlap and manual provenance audit | Audit recorded in `docs/status.md` |

## Risks

- **Scale.** About 100 `Widget` implementations and every example change.
  Mitigated by the adapter in phase 1: old and new views coexist, so the
  work can stop safely after any phase.
- **Performance of `dyn View`.** One virtual call per view per pass;
  measured with `taim_bench` against the current path before phase 4.
- **Signal lifetimes.** Signals are owned by their window scope and
  dropped with it; handlers capture copies of `Signal` handles (cheap
  `Arc`s), so there are no borrowed-state lifetimes in views.

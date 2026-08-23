# Aura

**A GPU-first UI framework in Rust.**

Aura is a fork of [iced](https://github.com/iced-rs/iced), extended with a
damage-driven renderer, a gesture arena, a retained semantic tree, a
token-based design system, and a mobile component set. The upstream
architecture and a large part of the widget and renderer code originate
there; see `CREDITS.md`.

```bash
cargo run -p aura-showcase     # every widget, both themes
cargo run -p mobile_shell      # a mobile app shell
```

---

## Measured performance

Every number below is read out of a file in `benchmarks/baseline/`, captured
on 2026-08-23 from a clean tree at a named commit on a registered device.
Nothing here is estimated, and nothing is quoted from a run that is not
committed.

| Metric | Value | Scene |
|---|---|---|
| Framework frame cost, p95 — **best** | 6.51 µs | `idle_shell` |
| Framework frame cost, p95 — **worst** | **57.59 µs** | `virtual_list` |
| Framework frame cost, p99 — **worst** | **74.40 µs** | `virtual_list` |
| Margin against the 60 Hz budget, worst p99 | **~224×** | 16.67 ms available |
| Missed frames | **0.000%** | all 13 scenes |
| Allocations, 5 runs of 10k rows | 199,286 alloc / 199,284 free | net delta 2 |
| Peak RSS | 196.7 MiB | `text_heavy` |
| Release binary, `mobile_shell` | 20.53 MiB unstripped · 15.76 MiB stripped | — |

All thirteen scenes, ordered by p95, cheapest first. Times in microseconds,
RSS in MiB:

| Scene | p50 | p95 | p99 | update p95 | draw p95 | RSS |
|---|---:|---:|---:|---:|---:|---:|
| `idle_shell` | 4.01 | **6.51** | 7.22 | 1.73 | 4.74 | 136.9 |
| `effect_ladder` | 4.46 | **8.75** | 11.42 | 2.21 | 6.55 | 138.5 |
| `ten_thousand_rows` | 6.24 | **10.76** | 14.35 | 2.97 | 7.60 | 138.9 |
| `multitouch` | 6.63 | **12.02** | 16.81 | 4.24 | 7.93 | 138.2 |
| `animated_cards` | 7.88 | **14.66** | 25.08 | 3.66 | 10.90 | 135.8 |
| `text_heavy` | 8.50 | **14.86** | 24.77 | 4.06 | 10.02 | 196.7 |
| `blur_layers` | 8.35 | **15.95** | 22.12 | 2.60 | 13.10 | 135.8 |
| `motion_stress` | 4.55 | **23.21** | 36.08 | 15.97 | 8.55 | 136.1 |
| `text_edit` | 8.79 | **23.38** | 33.79 | 5.97 | 17.63 | 143.9 |
| `damage_churn` | 10.49 | **24.02** | 26.47 | 7.59 | 15.90 | 137.8 |
| `image_grid` | 10.81 | **29.95** | 36.32 | 6.39 | 22.19 | 142.3 |
| `virtual_list_variable` | 15.72 | **33.70** | 42.33 | 16.10 | 15.85 | 137.5 |
| `virtual_list` | 27.49 | **57.59** | 74.40 | 30.08 | 32.42 | 137.9 |

```bash
scripts/perf/run-baseline.sh --device dev-workstation-01   # all 13, as committed
cargo run -p aura_bench --release --features profiling -- --scene ten_thousand_rows
```

**Frame cost is `update + draw`** — the work the framework does to turn
application state into a submitted frame. It deliberately excludes the
harness's `Renderer::screenshot` readback, which costs 6.8–9.6 ms on this
machine regardless of scene content and is reported separately. A benchmark
that folded that readback into its headline would be measuring itself rather
than the framework, and two regression tests exist to stop it coming back.

**`virtual_list` is the number to look at.** It is the only scene that drives
the real `VirtualList` widget, over 100,000 rows, and at 57.59 µs p95 it is
8.8× the cheapest scene, with more than half its cost in `update` rather than
paint. It is the closest thing here to a list-heavy application, so it is the
honest headline — not `idle_shell`.

Numbers come from `dev-workstation-01` (see `benchmarks/devices.toml`): a
development laptop with an integrated Intel Iris Xe, running headless with the
CPU governor at `powersave`. That is not locked reference hardware, and P95 on
an unpinned governor is known to drift — see `docs/performance/methodology.md`.
Treat these as evidence the harness is reproducible, not as a hardware claim.

### What has *not* been measured

**There is no Flutter or React Native comparison in this repository.**
`benchmarks/competitors/` states the rules such a comparison must follow and
holds no results. The comparison is not claimed until the same application is
built three times and measured on identical hardware.

Also unmeasured: **cold start**, **energy draw**, and **steady-state
application memory** — the three axes on which a framework with no garbage
collector should win. The RSS figures above are the benchmark harness, not an
application.

Two subsystems the architecture advertises are **not on the live path**: GPU
visual effects (the effect pipeline is a stub, so `blur_layers` and
`effect_ladder` measure a cost model, not pixels) and GPU partial present
(damage is computed and then discarded before the surface is presented).
`Capstone.md` documents both, with the evidence.

---

## Components

75 public constructors. The mobile set below is the part built on top of
the fork, each with a theme catalog, semantics, and unit tests.

**Navigation and structure** — `scaffold`, `drawer`, `bottom_sheet`,
`bottom_nav_bar`, `tabs`, `app_bar`, `navigator`, `carousel`, `accordion`

**Input** — `button`, `checkbox`, `radio`, `toggler`, `slider`,
`vertical_slider`, `text_input`, `text_editor`, `pick_list`, `combo_box`,
`search_bar`, `stepper`, `segmented_control`, `chip`, `date_picker`,
`time_picker`

**Display** — `text`, `rich_text`, `markdown`, `image`, `svg`, `canvas`,
`qr_code`, `table`, `avatar`, `badge`, `skeleton`, `progress_bar`,
`tooltip`, `snackbar`

**Layout** — `column`, `row`, `grid`, `stack`, `container`, `scrollable`,
`pane_grid`, `virtual_list`, `sliver`, `responsive`, `adaptive`,
`safe_area`, `float`, `pin`, `constrain`

**Gestures** — `pull_to_refresh`, `mouse_area`, `sensor`

### Not yet built

Calendar range selection, tree view, pagination, breadcrumb, rating,
video, web view, and map. Per-day accessibility semantics inside
`date_picker` — the grid reports one node naming the chosen date, not one
node per day.

---

## Architecture

```text
┌─────────────────────────────────────────────────┐
│                  Application                     │
│         boot / update / view (Elm-style)         │
├──────────┬──────────┬───────────┬───────────────┤
│  widget  │ runtime  │ graphics  │   renderer    │
│ controls │ reconcile│ damage    │  wgpu + skia  │
│          │ schedule │ layers    │  compositors  │
├──────────┴──────────┴───────────┴───────────────┤
│                    core                          │
│  types · layout · text · input · semantics       │
├─────────────────────────────────────────────────┤
│  aura-design · aura-motion · aura-gesture        │
│  aura-accessibility · aura-profiler              │
└─────────────────────────────────────────────────┘
```

153,983 lines of Rust across 23 workspace members. Measured on 2026-08-23
with `find <crate> -name '*.rs' | xargs cat | wc -l`:

| Crate | Purpose | LOC |
|---|---|---|
| `widget` | The control set | 51,089 |
| `core` | Portable types, layout, text, input, semantics, damage | 19,686 |
| `wgpu` | GPU renderer: batching, pipeline cache, atlas | 10,215 |
| `runtime` | Reconciliation, frame scheduling, hit testing, routing | 8,012 |
| `graphics` | Renderer-neutral primitives, damage tracking, caches | 7,881 |
| `aura-accessibility` | Retained semantic tree, focus, AT-SPI, live regions | 4,904 |
| `winit` | Desktop shell integration | 4,293 |
| `aura-gesture` | Arena, tap, pan, pinch, rotate, swipe, nested scroll | 3,844 |
| `tiny_skia` | Software renderer fallback | 3,326 |
| `aura-design` | Token and component specs driving widget appearance | 2,501 |
| `aura-highlighter` | Dependency-free syntax highlighting | 1,868 |
| `aura-profiler` | Low-overhead telemetry, Chrome trace export | 1,640 |
| `aura-platform-vyom` | Native Wayland/Smithay backend (not the default path) | 1,608 |
| `aura-motion` | Springs, tweens, decay, keyframes, transitions | 1,081 |

`aura-design` is a **specification** layer: each component declares its
role, actions, minimum size, and appearance from semantic tokens, and
`widget/src/aura.rs` maps those onto real widget styles.

That mapping is **incomplete**: 41 widgets declare a `Catalog`, and 24 have an
implementation for the design-system theme. The 17 that do not are the mobile
set — `bottom_sheet`, `drawer`, `tabs`, `chip`, `badge`, `avatar`,
`date_picker`, and the rest — which fall back to the built-in theme. Closing
that gap is tracked in `Capstone.md` F-13.

---

## Quick start

```toml
[dependencies]
aura = { version = "0.15.0-dev", features = ["wgpu"] }
```

```rust
use aura::widget::{button, column, text};
use aura::{Element, Task};

#[derive(Debug, Clone)]
enum Message {
    Incremented,
}

struct Counter {
    value: i32,
}

impl Counter {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Incremented => self.value += 1,
        }

        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        column![
            text(self.value).size(32),
            button(text("Increment")).on_press(Message::Incremented),
        ]
        .spacing(12)
        .into()
    }
}

fn main() -> aura::Result {
    aura::application(|| (Counter { value: 0 }, Task::none()), Counter::update, Counter::view)
        .run()
}
```

---

## How Aura compares

Honest positioning, since the temptation is to overclaim.

**Where Aura is genuinely ahead**

- **No garbage collector.** Rust's ownership model, not a GC pause budget.
- **Zero idle work.** No frames are produced when nothing changes. The
  allocation delta of 3 across five benchmark runs above is that property
  showing up as evidence.
- **Damage-driven repaint.** Unchanged subtrees skip paint entirely.
- **Deterministic testing.** Fixed-clock, headless, golden-oracle suites
  that need neither a GPU nor a device farm.
- **Accessibility by construction.** Roles and actions come from the
  component spec, so a widget cannot ship without declaring them.
- **A software fallback.** `tiny_skia` renders the same tree without a GPU.

**Where Flutter and React Native are ahead**

- **Component breadth.** Ten years and several hundred widgets each.
- **Ecosystem.** pub.dev and npm dwarf what exists for Rust GUI.
- **Tooling.** DevTools, stateful hot reload, mature inspectors.
- **Hiring.** Dart and JavaScript developer pools are far larger.

**Unproven either way.** Memory footprint, cold start, and energy draw
are the axes where a no-GC framework should win, and Aura measures its
own side of all three. None of them has been compared against Flutter or
React Native on identical hardware, so no comparison is claimed.

---

## Testing

```bash
cargo test --workspace          # 1,705 tests, 0 failures
./scripts/ci/check-budgets.sh   # dependency count, binary size, unsafe, metric guards
```

`unsafe_code = "deny"` is the workspace default. Nine files opt out, each
on an explicit allow list the budget script enforces.

---

## Licence

MIT. See `LICENSE` and `CREDITS.md`.

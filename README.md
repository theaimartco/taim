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

Every number below comes from `tools/aura_bench` and can be reproduced
with the command beneath it. Nothing here is estimated.

| Metric | Value | Scene |
|---|---|---|
| Framework frame cost, p95 | **8.9 µs** | `ten_thousand_rows` |
| Framework frame cost, p95 | **4.4 µs** | `idle_shell` |
| Margin against the 60 Hz budget | **~1,800×** | 16.67 ms available |
| Missed frames | **0.000%** | all 13 scenes |
| Allocations, 5 runs of 10k rows | 197,846 alloc / 197,843 free | net delta 3 |
| Release binary, `mobile_shell` | **10.3 MiB** | unstripped |

```bash
cargo run -p aura_bench --release -- --scene ten_thousand_rows
cargo run -p aura_bench --release --features profiling -- --scene ten_thousand_rows
```

**Frame cost is `update + draw`** — the work the framework does to turn
application state into a submitted frame. It deliberately excludes the
harness's `Renderer::screenshot` readback, which costs a near-constant
~2.3 ms regardless of scene content and is reported separately. A
benchmark that folded that readback into its headline would be measuring
itself rather than the framework, and two regression tests exist to stop
it coming back.

Numbers were captured on a development machine, not on locked reference
hardware. `scripts/perf/run-baseline.sh` and `compare-baseline.sh` are
the tools for a trustworthy baseline; `benchmarks/devices.toml` records
what a device name means.

### What has *not* been measured

**There is no Flutter or React Native comparison in this repository.**
`benchmarks/competitors/` is empty. Aura's own numbers are above; the
comparison is not claimed until the same application is built three times
and measured on identical hardware. Cold start and energy draw are also
unmeasured.

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

| Crate | Purpose | LOC |
|---|---|---|
| `core` | Portable types, layout, text, input, semantics, damage | 19,686 |
| `widget` | The control set | 38,663 |
| `wgpu` | GPU renderer: batching, pipeline cache, effects, atlas | 10,434 |
| `runtime` | Reconciliation, frame scheduling, hit testing, routing | 8,012 |
| `graphics` | Renderer-neutral primitives, damage tracking, caches | 7,881 |
| `aura-accessibility` | Retained semantic tree, focus, AT-SPI, live regions | 4,904 |
| `aura-gesture` | Arena, tap, pan, pinch, rotate, swipe, nested scroll | 3,844 |
| `aura-design` | Token and component specs driving widget appearance | 2,501 |
| `aura-profiler` | Low-overhead telemetry, Chrome trace export | 1,640 |
| `aura-motion` | Springs, tweens, decay, keyframes, transitions | 1,081 |
| `tiny_skia` | Software renderer fallback | — |

`aura-design` is a **specification** layer: each component declares its
role, actions, minimum size, and appearance from semantic tokens, and
`widget/src/aura.rs` maps those onto real widget styles. There is no
second styling path to drift out of sync.

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
cargo test --workspace          # 1,600+ tests
./scripts/ci/check-budgets.sh   # dependency count, binary size, unsafe, metric guards
```

`unsafe_code = "deny"` is the workspace default. Nine files opt out, each
on an explicit allow list the budget script enforces.

---

## Licence

MIT. See `LICENSE` and `CREDITS.md`.

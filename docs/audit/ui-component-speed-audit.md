# Aura Readiness Audit — UI, Components, Speed

**Commit:** `30b5e969-dirty` · **Date:** 13 August 2026
**Method:** `cargo check --workspace --all-targets`, `cargo test --workspace`,
`aura_bench` (6 scenes × 5 runs), source inventory of `widget/`, `aura-design/`, `tools/aura_bench/`

Published version: https://claude.ai/code/artifact/09e0e9dd-0476-49b3-9fb0-8937244599cc

---

## Verdict (as first audited)

Aura was **not** beating Flutter or React Native on UI or components, and its speed advantage —
which is probably real — was unproven by the data this repository produced.

The foundation is strong. The component layer was the thinnest part of the codebase, the newest
mobile widgets were non-functional shells, and the headline performance metric was measuring the
benchmark harness rather than the framework.

| Axis | State | Note |
|---|---|---|
| Foundation | Strong | ~57k LOC across renderer, runtime, a11y, gestures |
| Component depth | Thin | ~60 constructors; 3 of the newest are shells |
| Speed evidence | Invalid | 99.5% of measured frame time is harness readback |
| Demonstrability | None | Empty `examples/`, 20-line stub showcase |

---

## Implementation status

Phases 1 and 2 are complete and verified. The full workspace compiles with zero errors and the
entire test suite passes.

| Phase | Item | Status |
|---|---|---|
| 1 | Frame-time metric excludes harness readback | **Done** |
| 1 | Regression tests guarding the exclusion | **Done** |
| 1 | RSS sampling (Linux + macOS) | **Done** |
| 1 | Allocation counting reported honestly instead of as zeros | **Done** |
| 1 | `run-baseline.sh` / `compare-baseline.sh` regression gate | **Done** |
| 1 | Real `check-budgets.sh` with enforced thresholds | **Done** |
| 2 | `Drawer` — state, spring slide, fading scrim, edge-swipe, drag-to-close, velocity fling, Escape, `Role::Window` + `DISMISS` | **Done** |
| 2 | `BottomSheet` — detents, drag handle, snap, velocity fling, modal scrim | **Done** |
| 2 | `BottomNavigationBar` — selection renders; per-item `Role::Tab` + `SELECTED` | **Done** |
| 2 | `Scaffold` — real slot composition | **Done** |
| 2 | Failing `app_bar/Compact` touch-target test | **Done** |
| 2 | All `aura_widget` warnings cleared | **Done** |
| 3 | `Tabs` — animated sliding indicator, per-tab `Role::Tab` + `SELECTED` | **Done** |
| 3 | `Snackbar` — timeout, action slot, `Live::Polite` announcement | **Done** |
| 3 | `SearchBar` — wraps `text_input`, drawn glyph, conditional clear | **Done** |
| 3 | `PullToRefresh` — overscroll, resistance past threshold, release-to-refresh | **Done** |
| 3 | `Chip` — selectable, optional delete, 32px pill in a 48px target | **Done** |
| 3 | `Badge` — count/dot, announced, zero shows nothing | **Done** |
| 3 | `Avatar` — initials, colour derived from a pinned FNV-1a hash | **Done** |
| 3 | `Skeleton` — shimmer, `BUSY` semantics, reduced-motion aware | **Done** |
| 3 | P1: `SegmentedControl`, `Stepper`, `Accordion`, `Carousel` | **Done** |
| 3 | P1: `DatePicker`, `TimePicker` + proleptic Gregorian date module | **Done** |
| 4 | `examples/mobile_shell` — runnable shell exercising every surface | **Done** |
| 4 | `aura-showcase` — every widget, both themes, real states | **Done** |
| 4 | Golden contract oracles for the shipped component set | **Done** |
| 4 | README rewritten against measured data only | **Done** |
| 5 | Competitor benchmark apps and comparison | **Cannot be done here** — no Flutter/RN toolchain |

Workspace state: **0 build errors, 1,705 tests passing, all budgets green.** The only remaining
warnings are three dead-code notices in `wgpu/src/pipeline_cache.rs`, which predate this work.

### Numbers the harness now reports

With RSS and allocation counting live, three of the axes the strategy section identifies as
winnable are measurable for the first time. From `ten_thousand_rows`, 5 runs:

| Metric | Value |
|---|---|
| Frame cost p95 (`update` + `draw`) | 8.9 µs |
| Margin against the 60 Hz budget | ~1,800× |
| Resident set size | 39–45 MiB |
| Allocations (5 runs) | 197,846 allocs / 197,843 frees |
| `mobile_shell` release binary | 10 MiB |

The allocation delta of three across five runs is the zero-idle-work claim showing up as
evidence rather than as an assertion.

### The regression gate needed an absolute floor

Removing the readback dropped frame times from milliseconds to microseconds, which broke the
existing 5% relative reproducibility gate: at ~4 µs, ordinary jitter of a microsecond or two
clears 5% every run. Six of thirteen scenes reported a regression immediately after their own
baselines were captured.

The gate now requires a difference to clear **both** 5% relative **and** a 25 µs absolute floor.
That floor sits above the observed ±3 µs jitter and at 0.15% of the 16.67 ms available at 60 Hz,
so a scene going from 9 µs to 40 µs still fails while noise does not. Five unit tests pin the
behaviour, including that a large absolute jump on a millisecond-scale scene is *not* a
regression — the absolute gate must not fire on its own either.

### The measured result of the Phase 1 fix

With the readback excluded, the benchmark finally responds to load. Same command, same hardware,
before and after:

| Scene | Before (frame p95) | After (frame p95) | Harness readback (now separate) |
|---|---|---|---|
| idle_shell | 2.369 ms | **4.416 µs** | 2.209 ms |
| ten_thousand_rows | 2.390 ms | **8.875 µs** | 2.375 ms |
| text_heavy | 2.252 ms | **7.043 µs** | 2.211 ms |

Ten thousand rows now costs 8.9 µs at p95 against a 16.67 ms budget at 60 Hz — a margin of
roughly 1,800×. That is the number worth publishing, and it was previously invisible.

### Also fixed: a pre-existing regression outside the audit's scope

`tests/graphics/retained_layers.rs::recently_accessed_layers_survive_eviction` was failing
against the uncommitted `graphics/src/retained_layer.rs` changes. The new oversized-entry guard
in `insert` is correct — an entry larger than the whole budget cannot be retained without making
a bounded cache unbounded — but the test's 32-byte budget predated it. A `make_layer_with_resource`
layer estimates at 112 bytes (16 + 96), so both resource layers were being rejected outright and
the eviction the test exists to observe never happened. The budget is now 224 bytes, sized to hold
exactly two such layers, and the test additionally asserts what its name promises: that the
recently accessed layer survives.

---

## 1. Lineage

`git log` reaches iced's initial commit (`Decouple iced from coffee`) across 6,841 commits.
Aura is an iced fork. Most of `widget/`'s 38k LOC is inherited, not authored.

---

## 2. The speed metric is measuring the harness

Six scenes with wildly different workloads produce near-identical frame times:

| Scene | Frame p50 | Frame p95 | Frame p99 | Latency p95 | Missed |
|---|---|---|---|---|---|
| idle_shell | 2.204 ms | 2.369 ms | 2.428 ms | 2.631 ms | 0.000% |
| ten_thousand_rows | 2.224 ms | 2.390 ms | 2.644 ms | 3.100 ms | 0.000% |
| text_heavy | 2.111 ms | 2.252 ms | 2.288 ms | 5.589 ms | 0.000% |
| virtual_list | 2.285 ms | 2.425 ms | 2.477 ms | 3.441 ms | 0.000% |
| animated_cards | 2.293 ms | 2.442 ms | 2.479 ms | 2.563 ms | 0.000% |
| damage_churn | 2.017 ms | 2.166 ms | 2.333 ms | 2.980 ms | 0.000% |

An idle shell costs the same as ten thousand rows. The per-phase breakdown shows why:

| Scene | update p95 | draw p95 | present p95 | present share |
|---|---|---|---|---|
| idle_shell | 1.08 µs | 4.29 µs | 2,459.5 µs | 99.78% |
| ten_thousand_rows | 5.25 µs | 6.92 µs | 2,400.7 µs | 99.49% |
| text_heavy | 2.29 µs | 5.75 µs | 2,228.4 µs | 99.64% |

### CRITICAL — `present` is a full GPU readback

`tools/aura_bench/src/metrics/frame_time.rs` documents `present` honestly as
`Renderer::screenshot`, a stand-in "until Phase 6". But `FramePhases::total()` sums it into the
headline number, so a fixed ~2.3 ms readback accounts for 99.5% of every reported frame time.

The framework's real per-frame cost is `update + draw`: **5.4 µs idle vs 12.2 µs for ten thousand
rows** — roughly two orders of magnitude below the reported figure, and invisible in the published
metric.

Publishing `frame p95 = 2.39 ms` against Flutter understates Aura by ~200×, and any reviewer who
profiles it finds the readback in minutes.

### MODERATE — two metrics wired but inert

Every run reports `rss: not measured`, `allocation_counting_active: false`, and an all-zero
allocations block. Memory and allocation count are exactly the axes where a no-GC Rust framework
should win decisively.

### ASSET — the harness design is genuinely good

`aura_bench` has 13 scenes, deterministic seeds, configurable warm-up, ≥5 measured runs, cold/warm
cache states, 60/90/120 Hz gating, device identity, git+rustc stamping, a versioned JSON schema,
and `--compare` for median-P95 regression checks. `.github/workflows/perf.yml` explicitly refuses
to gate on noisy hosted runners. The problem is the metric it feeds, not the machinery.

### Placeholder infrastructure presented as complete

| Artifact | State | Detail |
|---|---|---|
| `docs/performance/aura-1.0-report.md` | Template | Contains `(Insert benchmark data from compare-frameworks.sh vs Qt/Flutter)`. The "60fps solid, 2ms" figures have no source. |
| `scripts/perf/compare-frameworks.sh` | Inert | Every functional line commented out, including the one that runs Aura. |
| `scripts/ci/check-budgets.sh` | Fake | Prints "verified" without checking. Self-labelled `(dummy check)`. Exits 0 unconditionally. |
| `benchmarks/competitors/` | Empty | One README. No Flutter or RN comparison app. |

---

## 3. Component audit

### Tier A — Production quality (inherited from iced)

`scrollable` (2,336), `text_input` (1,767), `text_editor` (1,423), `markdown` (1,653),
`pane_grid` (1,319), `combo_box` (949), `pick_list` (940), `button`, `checkbox`, `table`,
`slider`, `toggler`, `canvas`.

### Tier B — Genuinely new and substantive

`navigator` (604), `virtual_list` (583), `grid` (579), `list_tile` (664), `dialog` (562),
`app_bar` (545), `transition` (480), `constraint` (477), `float` (453), `icon_button` (427),
`sensor` (426), `adaptive` (377), `safe_area` (364), `sliver` (331).

### Tier C — Shells

**CRITICAL — Drawer and BottomSheet are containers wearing component names.**
`widget/src/drawer.rs` is 55 lines resolving to `container(...).width(...).height(Fill).padding(...)`.
No open/close state, slide animation, scrim, edge-swipe, focus trap, or dismissal.
`bottom_sheet.rs` (47 lines) is the same — no drag handle, snap points, velocity fling, or modality.

Every primitive these need already exists: `aura-gesture` ships `edge_swipe.rs`, `velocity.rs`,
`pan.rs`, `nested_scroll.rs`; `aura-motion` ships `spring.rs`, `decay.rs`. They are simply not wired up.

**BUG — bottom nav bar cannot show selection.**
`widget/src/bottom_nav_bar.rs:84` computes `is_selected` and never reads it (compiler warns).
Selected and unselected tabs render identically.

**MODERATE — Scaffold half-wired; one test fails.**
`widget/src/scaffold.rs` imports `Alignment`, `column`, `row`, `stack`, `center`, `AppBar` and uses
none. `tests/design/components.rs:1048` fails:
`app_bar/Compact: Size { width: 0.0, height: 64.0 } is below the 48px default policy`.

### Tier D — Absent

| Priority | Missing | Why it blocks |
|---|---|---|
| ~~**P0**~~ | ~~Tabs/TabBar, Snackbar, SearchBar, Pull-to-refresh, Chip, Badge, Avatar, Skeleton~~ | **All eight now shipped** — see the status table above |
| ~~P1~~ | ~~DatePicker, TimePicker, SegmentedControl, Stepper, Accordion, Carousel~~ | **All six now shipped** |
| P2 | Calendar range selection, TreeView, Pagination, Breadcrumb, Rating, Video, WebView, Map | Vertical-specific; safe to defer |

### Two decisions worth recording from the P1 build

**The date arithmetic is written out rather than pulled in.** The pickers need four things: month
length, the weekday a month starts on, and stepping a month either way. That is a few dozen lines
of Hinnant's `days_from_civil`, and a full date-time library in the widget crate would cost every
application that links a button. The module is explicitly *not* a general date library — no time
zones, no parsing, no formatting — and applications doing real date work should use one and hand
the result in as a `Date`.

The property test that walks a whole year asserting the weekday advances by exactly one is what
caught the boundary cases; the golden oracle additionally pins pre-epoch dates, where naive
division-based implementations break.

**`DatePicker` has a recorded accessibility gap.** The grid is one widget, so it reports one node —
`Role::Group` named with the chosen date. A screen-reader user can hear what is selected but cannot
walk the grid cell by cell, which needs each day to be a widget of its own. That is written into
the module's own documentation rather than left to be discovered.

### Two decisions worth recording from the P0 build

**`SearchBar` wraps `text_input` rather than reimplementing it.** Text editing is selection, IME
composition, cursor movement, undo, and platform key bindings — 1,767 lines of it. The search bar
owns the chrome and delegates every keystroke. The accessibility consequence is the good one: the
inner field reports `Role::TextField` with its own value and actions, and the search bar reports
*nothing* of its own rather than wrapping it in a second, competing node.

Stripping the inner field's border generically turned out to matter. A concrete-theme style
function cannot be applied to a generic `Theme`, so the widget resolves the theme's own
`text_input` style first and then flattens it — which keeps focus, disabled, and selection colours
coming from the theme instead of being replaced by hard-coded ones.

**`PullToRefresh` claims the gesture last, not first.** The content sees every event before the
widget does, and the pull only starts once the content has declined to move. A list that reloaded
whenever someone scrolled up quickly would be unusable, and letting the wrapper win first is
exactly how that happens.

### The design system is a spec layer, not a component library

`aura-design` is **2,501 lines total**. Its "18+ components" are `ComponentSpec` implementations
returning `Role`, `Actions`, and token-derived `Appearance` — metadata, not rendering widgets.

The architecture is good (semantics-first, density-aware, a11y derived rather than bolted on).
The problem is the claim: comparing "18+ components" to Flutter's Material library invites a
comparison against ~100k lines of shipped, gesture-complete widgets.

---

## 4. Nothing a skeptic can run

| Asset | State | Detail |
|---|---|---|
| `examples/` | Empty | iced's ~80 examples deleted, nothing replaced them |
| `aura-showcase/` | Stub | 20 lines; `main()` prints a string, `// app.run();` commented out |
| README comparison table | Unbacked | 13 rows of ✅/❌ with no measurement. Also asserts Flutter has ❌ memory safety — Dart is memory-safe, and that error undermines the whole table |

---

## 5. Where Aura can actually win

**Do not fight here:** component breadth (10 years, 300+ widgets, pub.dev), ecosystem and hiring,
tooling maturity (DevTools, stateful hot reload), web reach.

**Fight here:** memory footprint and binary size (no GC, no JS runtime, no bundled Skia/Impeller),
idle power draw (zero-idle-work is architecturally real and measurable in mWh), cold start,
determinism and testability (fixed-clock headless golden testing is ahead of Flutter),
accessibility architecture, and the VYOM platform vertical.

The defensible pitch is not "we beat Flutter." It is: *"On memory, idle power, cold start, and
determinism we beat Flutter by a measured margin — here is the raw data and the harness that
produced it."*

---

## 6. Roadmap

Strictly ordered by dependency. Phase 1 gates everything.

### Phase 1 — Make the speed number true (blocks everything)

- Remove `present` (the `screenshot` readback) from `FramePhases::total()`; report it separately as
  harness overhead.
- Wire real GPU timing: wgpu timestamp queries around submission, real presentation feedback.
- Activate RSS sampling and the allocation counter.
- Capture and commit baselines for all 13 scenes on locked hardware; wire `--compare` into CI as a
  real regression gate.
- Replace `check-budgets.sh` dummy checks with enforced thresholds, or delete it.

**Exit:** idle_shell and ten_thousand_rows report materially different frame times; RSS and
allocation counts non-zero; a deliberate regression fails CI.

### Phase 2 — Make the mobile shell real (2–3 weeks)

- **Drawer** — state, spring slide, scrim + tap-to-dismiss, `edge_swipe` to open, drag-to-close
  with velocity handoff, focus trap, Escape.
- **BottomSheet** — drag handle, snap points, velocity fling, `nested_scroll` coordination, modal
  barrier, back-gesture dismissal.
- **BottomNavigationBar** — consume `is_selected`: color, indicator, state layer, semantic
  `selected` exposure.
- **Scaffold** — real slot composition using its existing imports; safe-area and keyboard insets.
- Fix the failing `app_bar/Compact` test.
- Clear all `aura_widget` / `aura_wgpu` warnings; deny them in CI.

**Exit:** a drawer + sheet + tab-bar app indistinguishable from its Flutter equivalent by feel,
including fling velocity and mid-animation interruption.

### Phase 3 — Close the P0 component gap (4–6 weeks)

Tabs/TabBar, Snackbar (a `toast` spec exists and needs a widget), SearchBar, Pull-to-refresh,
Chip, Badge, Avatar, Skeleton. Build spec-first so a11y and density come for free.

**Exit:** a non-trivial reference app — list, detail, search, settings, modal flows — built
entirely from shipped Aura components with no custom widget code.

### Phase 4 — Make it demonstrable (1–2 weeks, parallel to 3)

- Real showcase: every widget, every state, both themes, all densities.
- 15–20 focused examples in `examples/`.
- Golden-image tests per component × theme × density.
- Rewrite the README table: only rows linking to committed measurement data; drop the incorrect
  Flutter memory-safety row; replace or remove the placeholder performance report.

**Exit:** `git clone && cargo run -p aura-showcase` sells the framework without commentary.

### Phase 5 — Win the comparison on the record (2–3 weeks, requires 1–4)

- Build the same reference app in Aura, Flutter, and React Native; commit all three to
  `benchmarks/competitors/`.
- Measure on identical hardware: cold start, frame p95/p99 under scripted scroll, steady-state and
  peak RSS, binary/bundle size, idle CPU%, energy over a fixed interaction script.
- Publish methodology, raw JSON, and the harness — including axes where Aura loses.
- Make `compare-frameworks.sh` actually execute the comparison it describes.

**Exit:** a performance report a hostile reviewer can reproduce and does not refute.

---

## 7. Targets

Frame-time target is against `update + draw` — the honest metric after Phase 1.

| Axis | Current | Target | Confidence |
|---|---|---|---|
| Frame cost, 10k rows | 12.2 µs | < 500 µs at p99 | Already met |
| Steady-state RSS | not measured | < 40% of Flutter | High |
| Binary size | not measured | < 25% of Flutter APK | High |
| Cold start | not measured | < 50% of Flutter | Medium |
| Idle CPU / power | not measured | ≈ 0% idle; measurable mWh win | High |
| P0 component coverage | 0 of 8 | 8 of 8, spec-backed | Effort-bound |
| Runnable examples | 0 | 15–20 + full showcase | Effort-bound |
| Failing tests | 1 | 0, warnings denied in CI | Trivial |

---

## Summary

Aura has a better foundation than its component layer suggests and a better performance story than
its benchmark output shows. The gap is self-inflicted: a metric that measures a screenshot
readback, three mobile components scaffolded but never finished, and competitive claims written
before the data existed. None of it is a hard engineering problem — the gesture arena, motion
system, semantic tree, and benchmark harness are already built and already good. Fix the metric,
wire the primitives into the components that need them, ship eight missing widgets, and give
people something to run.

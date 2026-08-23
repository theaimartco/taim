# Aura Capstone — Competitive Audit and the Path Past Flutter and React Native

**Repository:** `vyom-aura` · **Audited at:** `bf065be7` ("ui components added") · **Date:** 2026-08-23
**Toolchain:** rustc 1.95.0 (59807616e 2026-04-14), cargo 1.95.0 · **Host:** Linux 7.0.0-30-generic, 20 cores, 14 GiB

---

## Status — M0 complete (2026-08-23)

Section 10's **M0 — Restore truth** has been executed. All eight tasks are
done and the exit gate passes:

```
cargo fmt --all -- --check                          PASS
cargo clippy --workspace --all-targets -D warnings  PASS
cargo test --workspace                              PASS  (1,705 passed, 0 failed)
bash scripts/ci/check-budgets.sh                    PASS  (4/4)
```

**Closed by M0** — F-01, F-02, F-03, F-04, F-05, F-06, F-12, F-20, F-21,
F-22, F-23, and the missing `CREDITS.md` (§9.7). F-11's dead code is gone;
the *capability* it represented (off-thread image decode) remains unbuilt and
is still M3 work.

**Two things M0 found that this audit had wrong:**

1. **The clippy error count was higher than 15.** `cargo clippy` aborted at
   `aura_wgpu`, so six further lints in `aura_widget` and `tests/design` were
   never reported. F-21's count reflected what the tool printed, not what was
   there.
2. **`aura_bench` recorded stale commits.** `build.rs` watched `.git/HEAD`,
   whose contents do not change when you commit on a branch, so the commit
   baked into the binary went stale until a branch switch. The new
   dirty-tree guard surfaced this on its first run: a clean tree still
   produced a `-dirty` report. Fixed in `5e2d7896`.

**The re-captured baselines are worse than the numbers this audit quoted**,
because they come from a different machine (Linux/Iris Xe, `powersave`
governor) than the unattributable macOS reports. Worst-case p95 moved from
33.79 µs to **57.59 µs** (`virtual_list`), and the 60 Hz margin from 493× to
**224×** on worst p99. Section 4's tables record the *old* reports as they
stood when audited; `benchmarks/baseline/` now holds the honest set, and
`README.md` quotes those.

**Everything else in this document stands as written.** The remaining P0s —
F-07 (effect pipeline is a stub), F-08 (GPU damage discarded), F-10 (no
visual truth set), F-13 (17 unthemed catalogs), F-14 (no icons or font),
F-15 (accessibility never reaches a screen reader), F-17 (no mobile
platform) — are untouched by M0 and unchanged.

---

## 0. How to read this document

Every number in this document was produced by a command run against this working tree on the
date above, or read out of a file committed to this repository. Where a claim could not be
verified, it is marked **UNVERIFIED** and the reason is given. Where a claim made elsewhere in
the repository disagrees with what the tree actually does, both numbers appear side by side.

There are three documents in this repository that overlap with this one:

| File | What it is | Status |
|---|---|---|
| `audit.md` | Competitive audit written at `b753d3e0` | Partly stale — P0-6, P0-10 have since been closed |
| `phase.md` | 18-phase execution roadmap | Still the authoritative build order |
| `docs/audit/ui-component-speed-audit.md` | UI/component/speed audit at `bf065be7` | Current, narrower scope |

This document supersedes the *findings* in `audit.md` (which were re-tested, not assumed) and
extends `docs/audit/ui-component-speed-audit.md` with the head-to-head framework analysis, the
full component gap table, and the framework-completeness checklist. It does not replace
`phase.md`, which remains the sequencing authority.

The short version, before the detail: **the engineering in this repository is genuinely good,
and the claims made about it are not.** Section 6.1 is the most important section here. Fix
that first, because every other advantage is worthless if the numbers can't be trusted.

---

## 1. Executive verdict

### 1.1 Rating

| Axis | Rating | One-line justification |
|---|---|---|
| Core architecture | **A−** | Damage model, gesture arena, semantic tree, keyed reconciliation are real and tested |
| Code quality | **A−** | 1,705 tests green, `unsafe_code = "deny"` with 9 audited exceptions, dense doc comments |
| Rendering completeness | **C** | Effect pipeline is an empty stub; GPU partial present is a no-op; retention is whole-frame |
| Component breadth | **C+** | 75 constructors, but 17 of 41 catalogs unthemed and no icon or font assets exist |
| Performance evidence | **D** | Headline numbers do not match the committed baselines; baselines are unattributable |
| Platform reach | **D** | Desktop Linux/Wayland/X11 only. Zero Android, zero iOS. |
| Ecosystem / DX | **D** | One example, no package published, no inspector app, no starter template |
| Process discipline | **C** | CI exists and is well written — and is red on `main` right now |
| **Overall readiness to challenge Flutter/RN** | **~20%** | Strong engine, no vehicle |

### 1.2 The one-sentence verdict

Aura is a **well-architected desktop-Linux GUI engine with a mobile-shaped widget set**, roughly
20% of the way to being a framework that can be *compared* to Flutter or React Native — and 0%
of the way to being one that can be *deployed* against them, because it does not run on a phone.

### 1.3 Can Aura beat Flutter and React Native?

**Not on their terms, and not in this decade.** Flutter has ~10 years, several hundred widgets,
2,000+ bundled icons, six platform targets, and a DevTools suite. React Native has the npm
ecosystem and renders real `UIView`/`android.view.View` instances so it inherits every platform
behaviour for free. Aura has 75 constructors, zero icons, and one platform.

**Yes, on a battlefield Aura chooses.** There is a real, defensible thesis here, and Section 8
develops it: Aura should not be "Flutter, but in Rust." It should be **the system UI layer for
VYOM OS and for memory- and power-constrained Linux devices** — a domain where a no-GC,
zero-idle-work, damage-driven, software-fallback-capable framework has structural advantages
that Flutter and React Native cannot copy without abandoning their own runtime models.

The three things that make that thesis winnable are:
1. **No GC.** Flutter's UI thread and RN's JS thread both have collector pauses in the frame path. Aura has none. This is not a tuning difference; it is a runtime-model difference.
2. **Zero idle work.** Aura produces no frames when nothing changes. Both competitors have paths that keep the pipeline warm.
3. **A software renderer that renders the same tree.** `tiny_skia` is a real fallback. Flutter's Impeller requires Vulkan/Metal; RN requires the platform view system.

None of those three advantages is *measured against a competitor* today. That is the work.

### 1.4 The five things blocking everything else

| # | Blocker | Evidence | Cost to fix |
|---|---|---|---|
| B1 | Public performance claims contradict the committed baselines | §6.1, F-01 | 1 day |
| B2 | CI is red on `main` — `fmt` and `clippy` both fail | §6.6, F-20/F-21 | 2 days |
| B3 | No mobile platform backend at all | §6.4, F-14 | 6–10 weeks |
| B4 | GPU effect pipeline is a literal empty stub | §6.2, F-07 | 4–6 weeks |
| B5 | No visual truth set — zero golden images, zero pixel comparisons | §6.2, F-10 | 2–3 weeks |

---

## 2. Verification log

Everything below was executed against this tree on 2026-08-23. This is the evidence base for
the rest of the document.

| Command | Result | Time |
|---|---|---|
| `cargo check --workspace --all-targets` | **PASS** (exit 0), 11 dead-code warnings in `aura_wgpu` | 19.6 s |
| `cargo test --workspace` | **PASS** (exit 0) — **1,705 passed, 0 failed, 3 ignored**, 63 binaries | ~7 min |
| `cargo build -p mobile_shell --release` | **PASS** (exit 0) | 44.9 s |
| `cargo fmt --all -- --check` | **FAIL** — 68 diff sites across ~30 files | 3 s |
| `cargo clippy --workspace --all-targets -- -D warnings` | **FAIL** — 15 errors, all in `aura_wgpu` | ~2 min |
| `bash scripts/ci/check-budgets.sh` | **FAIL** — 2 of 4 budgets breached | 6 s |
| `bash scripts/ci/check-workspace.sh` | **PASS** — 23 members resolved, workspace consistent | <1 s |
| `ls -l target/release/mobile_shell` | **21,536,248 bytes (20.54 MiB)** unstripped | — |
| `strip` on the same binary | **16,533,088 bytes (15.77 MiB)** | — |
| Parsed all 13 files in `benchmarks/baseline/*.json` | See §4 | — |

### What was **not** verified

- **No frame was ever presented to a screen.** This is a headless audit. No visual quality
  assessment was made, because there is nothing committed to compare a render against (F-10).
- **No Flutter or React Native application was built or measured.** Every competitive statement
  in this document is architectural, not empirical. `benchmarks/competitors/` contains a README
  and nothing else. No head-to-head number is claimed anywhere below, and none should be
  claimed publicly until §11 is satisfied.
- **No GPU work was executed.** No Vulkan device was opened; the `wgpu` paths were compiled and
  their tests run, not exercised against real hardware.
- **No mobile device, emulator, or ARM target was involved.**
- **`aura-showcase` and `mobile_shell` were compiled, not run.** They build; whether they look
  good is unknown and unknowable from this repository.

---

## 3. Verified inventory

### 3.1 Scale

**154,107 lines of Rust** across **23 workspace members**.

| Crate | Purpose | Actual LOC | README claims | Δ |
|---|---|---:|---:|---|
| `widget` | The control set | **51,094** | 38,663 | **+32%** |
| `core` | Portable types, layout, text, input, semantics, damage | 19,686 | 19,686 | ✓ |
| `wgpu` | GPU renderer: batching, pipeline cache, effects, atlas | 10,434 | 10,434 | ✓ |
| `runtime` | Reconciliation, scheduling, hit testing, routing | 8,012 | 8,012 | ✓ |
| `graphics` | Renderer-neutral primitives, damage, caches | 7,881 | 7,881 | ✓ |
| `aura-accessibility` | Retained semantic tree, focus, AT-SPI, live regions | 4,904 | 4,904 | ✓ |
| `winit` | Desktop shell integration | 4,287 | — | — |
| `aura-gesture` | Arena, tap, pan, pinch, rotate, swipe, nested scroll | 3,844 | 3,844 | ✓ |
| `tiny_skia` | Software renderer fallback | 3,326 | "—" | now measurable |
| `aura-design` | Token and component specs | 2,501 | 2,501 | ✓ |
| `src` (facade) | The `aura` crate itself | 2,086 | — | — |
| `aura-highlighter` | Dependency-free syntax highlighting | 1,868 | — | — |
| `aura-profiler` | Telemetry, Chrome trace export | 1,640 | 1,640 | ✓ |
| `aura-platform-vyom` | Native Wayland/Smithay backend | 1,608 | — | — |
| `futures` | Executors and subscriptions | 1,344 | — | — |
| `aura-motion` | Springs, tweens, decay, keyframes, transitions | 1,081 | 1,081 | ✓ |
| `renderer` / `program` / `debug` | Glue | 1,705 | — | — |
| `tools` (`aura_bench`, `aura_lab`) | Harnesses | 8,410 | — | — |
| `tests` | Integration suites | 17,238 | — | — |

**Finding:** the README's crate table understates `widget` by 12,431 lines and omits six crates
entirely. It was written before `bf065be7` and not refreshed. Minor on its own; part of a
pattern (§6.1).

### 3.2 Test suite — the strongest thing in this repository

```
cargo test --workspace  →  1,705 passed · 0 failed · 3 ignored · 63 binaries · exit 0
```

`#[test]` attribute count in-tree: 1,566 (the delta is doc-tests and generated cases). The
README says "1,600+ tests"; the real number is higher, and it is green. No test in the entire
workspace carries `#[ignore]`. There are **zero** `todo!()` or `unimplemented!()` calls in
production code — the 18 matches are all inside doc-comment examples plus one deliberate
`futures/src/backend/null.rs` null executor.

This is a genuinely high-discipline codebase. Say so plainly, because most of this document is
about what is wrong.

### 3.3 Safety posture

`unsafe_code = "deny"` is the workspace default. **9 files** opt out, every one of them on the
explicit allow list that `scripts/ci/check-budgets.sh` enforces, and the check passes:

```
core/src/image.rs                          wgpu/src/image/cache.rs
core/src/renderer/null.rs                  wgpu/src/quad/gradient.rs
tiny_skia/src/lib.rs                       wgpu/src/window/compositor.rs
tiny_skia/src/raster.rs                    tools/aura_bench/src/metrics/allocations.rs
tiny_skia/src/window/compositor.rs
```

15 `unsafe` blocks total. For a GPU framework this is exceptionally tight — Flutter's engine is
C++ throughout, and React Native's Fabric core is C++ with manual lifetime management. **This is
a real, defensible, marketable advantage and it is currently not marketed at all.**

### 3.4 Documentation

36 markdown files under `docs/`, ~45,900 words, including 10 full user-facing guides
(`getting-started`, `widgets`, `layout`, `styling`, `animations`, `gestures`, `accessibility`,
`canvas-drawing`, `design-system`, `scaling`, `tasks-and-subscriptions`). The architecture notes
(`docs/architecture/*.md`) are unusually good — they explain *why*, not just *what*.

Two documents undercut the rest and are dealt with in §6.1: `docs/performance/aura-1.0-report.md`
and the stale unit scale in `docs/performance/methodology.md`.

---

## 4. The measured baseline, re-read

### 4.1 What the committed baselines actually say

All 13 files in `benchmarks/baseline/` parsed. Values are the median-of-5-runs summary each
file records. Frame-cost columns are microseconds; `present` is milliseconds; RSS is MiB.

| Scene | p50 µs | **p95 µs** | p99 µs | update p95 | draw p95 | present p95 (ms) | missed | RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `effect_ladder` | 2.71 | **7.62** | 9.04 | 1.79 | 5.33 | 2.922 | 0.000 | 137.9 |
| `text_heavy` | 4.29 | **8.04** | 15.54 | 2.29 | 5.79 | 2.494 | 0.000 | 336.2 |
| `ten_thousand_rows` | 3.88 | **10.25** | 11.33 | 2.42 | 7.79 | 2.631 | 0.000 | 283.3 |
| `text_edit` | 6.12 | **11.00** | 12.50 | 2.67 | 8.38 | 2.492 | 0.000 | 290.8 |
| `blur_layers` | 6.92 | **11.08** | 12.58 | 1.33 | 9.79 | 2.919 | 0.000 | 280.5 |
| `damage_churn` | 5.42 | **11.96** | 16.42 | 6.29 | 8.08 | 2.663 | 0.000 | 283.7 |
| `animated_cards` | 7.75 | **12.04** | 15.17 | 2.38 | 10.12 | 2.847 | 0.000 | 283.0 |
| `motion_stress` | 2.46 | **13.88** | 15.79 | 6.79 | 7.33 | 2.681 | 0.000 | 283.6 |
| `multitouch` | 6.46 | **15.29** | 19.25 | 8.96 | 8.71 | 2.833 | 0.000 | 284.0 |
| `image_grid` | 10.21 | **15.71** | 17.79 | 2.96 | 12.96 | 2.925 | 0.000 | 288.0 |
| `idle_shell` | 3.38 | **6.38** | 8.25 | 1.79 | 4.83 | 2.906 | 0.000 | 283.8 |
| `virtual_list_variable` | 17.75 | **22.71** | 24.33 | 13.17 | 9.04 | 2.603 | 0.000 | 284.8 |
| `virtual_list` | 28.00 | **33.79** | 38.25 | 21.21 | 13.71 | 2.625 | 0.000 | 286.2 |

### 4.2 What is genuinely excellent here

- **Zero missed frames in all 13 scenes.** Not one. That is a real result.
- **Worst-case framework frame cost is 33.79 µs** against a 16,670 µs budget at 60 Hz — a
  **~493× margin on the worst scene**. Even the pessimistic reading is enormous headroom.
- **Update/draw split is recorded per scene**, which makes regressions attributable rather than
  merely detectable. Most frameworks cannot tell you which half got slower.
- **`effect_ladder` at 137.9 MiB RSS** versus ~283 MiB for everything else is a real signal the
  harness is measuring something scene-dependent, not a constant.

### 4.3 Six problems with using these numbers publicly

**P-1 — The README's headline numbers are not these numbers.**

| Claim in `README.md` | Committed baseline | Understated by |
|---|---|---|
| `ten_thousand_rows` p95 = **8.9 µs** (line 25) | **10.25 µs** | 15% |
| `idle_shell` p95 = **4.4 µs** (line 26) | **6.38 µs** | 45% |
| "Margin against the 60 Hz budget ~1,800×" (line 27) | 1,627× on `ten_thousand_rows`; **493× worst-case** | 3.7× overstated |

The README publishes the two fastest-sounding scenes and omits `virtual_list` — which is
**5.3× slower than the headline** and is the scene that drives the real `VirtualList` widget
that a real application would use. The README's own claim that "nothing here is estimated" is
therefore not accurate: something was estimated, or a different run was used and not committed.

**P-2 — The baselines cannot be attributed to a registered device.** Every one of the 13 files
records `environment.os = "macos"` and `environment.device = "dev-workstation-01"`. But
`benchmarks/devices.toml` registers `dev-workstation-01` as *"Linux (Ubuntu-based), kernel 7.0"*
with an *"Intel Iris Xe / Vulkan via Mesa"* GPU. The report and the device registry describe two
different machines. `docs/performance/methodology.md` explicitly forbids comparing across
unlike environments; these reports cannot be placed in any environment at all.

**P-3 — The baselines were captured from a dirty tree, two commits behind HEAD.** Every file
records `git_commit = "30b5e969f3db0123721597dbbc894aca63a69af9-dirty"`. HEAD is `bf065be7`.
The `-dirty` suffix means uncommitted changes were present, so the tree that produced these
numbers cannot be reconstructed by anyone, including its author.

**P-4 — Allocation counting was off in every run.** `allocation_counting_active: false` in all
13 files, which is why every `allocations` field reads zero. The README's "197,846 alloc /
197,843 free, net delta 3" therefore comes from a run that is **not committed anywhere in this
repository**. The harness is honest about this — `tools/aura_bench/src/report.rs:70-76`
documents precisely this trap — but the README quotes the number without the caveat.

**P-5 — Two metric families are wired but inert.** Across all 13 scenes, `pacing.jitter`,
`pacing.mean_target_error`, `pacing.max_queue_depth`, and `pacing.mean_queue_depth` are
**uniformly zero**. Real pacing measurement does not produce identical zeros in thirteen
different workloads. These fields are plumbed, not measured.

**P-6 — `docs/performance/methodology.md` is off by three orders of magnitude.** It analyses
`text_heavy` P95 as *"ranging from ~2.44 ms to ~3.56 ms"*. The committed `text_heavy` baseline
says **8.04 µs**. The document was written when `present` (the ~2.5 ms screenshot readback) was
still folded into the headline, and was not updated when the metric was redefined. Its
*conclusion* — that P95 is unstable on an unpinned governor — is still correct and still
important. Its numbers now describe a metric that no longer exists.

### 4.4 The one number nobody is looking at

**RSS sits between 280 and 336 MiB in twelve of thirteen scenes, including `idle_shell`.**

`idle_shell` is the do-nothing scene. It costs 283.8 MiB resident. Some of that is the benchmark
harness's own readback buffers and a wgpu device, so it is not the framework's footprint — but
nobody has separated the two, and **memory is the axis where the "no GC" argument is supposed to
win.** Publishing a frame-cost victory while an unexplained 283 MiB idle RSS sits in the repo is
how a competitive claim gets destroyed in one reply. Measure the app, not the harness, before
anyone says the word "memory" in public.

### 4.5 `virtual_list` deserves its own paragraph

At **33.79 µs p95 with 21.21 µs of that in `update` alone**, `virtual_list` is 5.3× the
headline scene and its cost is dominated by the update half, not paint. Every other scene is
draw-dominated. This is the one scene driving the real `aura_widget::VirtualList` over 100,000
rows, and it is the closest thing in the suite to "what a real list-heavy app costs."

In absolute terms it is still 493× inside budget and it misses zero frames — this is not a
crisis. But it is the profile that matters most for the mobile workloads Aura is aiming at
(feeds, settings lists, launchers), it is the least flattering number in the suite, and it is
the one the README omits. Publishing the other twelve and hiding this one is the single fastest
way to lose credibility with the exact audience being courted.

---

## 5. Architectural comparison: Aura vs Flutter vs React Native

> Everything in this section is **architectural**, derived from each framework's design. No
> head-to-head measurement exists (§2). Nothing here should be quoted as a benchmark result.

### 5.1 Rendering model

| | **Aura** | **Flutter** | **React Native** |
|---|---|---|---|
| Draws with | Own `wgpu` renderer + `tiny_skia` software fallback | Impeller (default: iOS 3.10+, Android 3.27+); Skia elsewhere | Nothing — delegates to platform views |
| Pixel identity across platforms | Yes (own rasteriser) | Yes (own rasteriser) | **No** — inherits platform look |
| Shader compilation jank | N/A (no dynamic effect shaders exist yet) | Solved by Impeller's build-time shader precompilation | N/A |
| Software fallback | **Yes — `tiny_skia` renders the same tree** | No (needs Vulkan/Metal/GL) | No (needs the platform view system) |
| Partial present / damage | **Software: yes. GPU: no — logged then discarded** | Layer-based caching + `RepaintBoundary`; raster-side dirty regions | Platform's own view invalidation |
| Retained layers | Implemented, but **granularity is the whole frame** | `RepaintBoundary`, per-subtree, developer-placed | Platform view tree is inherently retained |
| Visual effects (blur, backdrop, colour matrix) | **Cost model only — pipeline is an empty stub** | Full: `BackdropFilter`, `ImageFilter`, `ShaderMask`, `ColorFiltered` | Platform-native (`UIVisualEffectView`, `RenderEffect`) + community libs |

**Read:** Aura's rendering *architecture* is closer to Flutter's than to React Native's — own
renderer, own widget tree, pixel-identical output. Its rendering *implementation* is behind
both. The software fallback is the one axis where Aura is architecturally ahead of Flutter, and
it is a decisive advantage on low-end and headless devices where Flutter simply will not run.

### 5.2 Threading and the frame pipeline

| | **Aura** | **Flutter** | **React Native (New Arch)** |
|---|---|---|---|
| Threads in the frame path | UI thread; `thread-pool`/`tokio`/`smol` executors for tasks | Platform, **UI (Dart)**, **Raster**, IO | **JS**, native/UI, background (Yoga/shadow) |
| Raster decoupled from build/layout | **No — single-threaded frame** | **Yes** — layer tree handed to raster thread | Platform compositor handles raster |
| Image decode | **Synchronous on the UI thread** — the async `Decoder` exists and is dead code | Background thread pool via `instantiateImageCodec` | Platform-native, off-thread |
| Idle behaviour | **Zero frames produced when nothing changes** | Vsync-driven; frames scheduled when dirty | JS event loop + platform vsync |
| GC pauses in the frame path | **None — no GC exists** | Generational; scavenger is fast but pauses | Hermes (Hades, concurrent generational) |

**Read:** This is Aura's strongest and weakest comparison in one table.

*Strongest:* no GC, and zero idle work. Both competitors have a managed runtime somewhere in
the frame path. That is not a tuning gap they can close; it is what they are.

*Weakest:* Aura's frame is single-threaded. Flutter's raster thread means a slow paint does not
block the next build. Aura has no such separation, so any expensive paint directly costs the
next frame's budget. With 493× of headroom today this is invisible — and it will stop being
invisible the moment real effects, real image decode, and real blur land, because those are
exactly the costs the raster thread exists to absorb. **Aura's future perf ceiling is set here,
not in micro-optimisation.**

### 5.3 Layout

| | **Aura** | **Flutter** | **React Native** |
|---|---|---|---|
| Algorithm | Constraint-based, iced-derived; `Tree::layout_with_cache` for revision-based reuse | Single-pass: constraints down, sizes up, parent positions | Yoga (Flexbox) in C++ |
| Complexity | Linear in visited nodes; cached subtrees skipped | O(n), one pass; intrinsics cost extra passes | O(n) Flexbox |
| Incremental / cached | **Yes — constraint+revision keyed cache** | No general cache; relayout boundaries limit propagation | Shadow-tree diffing |
| Virtualisation | `virtual_list`, `sliver`, `scrollable` | `ListView.builder`, slivers, `CustomScrollView` | `FlatList`, `SectionList` (+ `FlashList` in the ecosystem) |

**Read:** Roughly at parity conceptually; Aura's `layout_with_cache` is arguably ahead of
Flutter's model, which relies on relayout boundaries rather than memoisation. But §4.5 shows
`virtual_list` update cost at 21.21 µs, which suggests the cache is not yet paying off on the
one path where it matters most. That is a profiling task, not a redesign.

### 5.4 State, reconciliation, and the programming model

| | **Aura** | **Flutter** | **React Native** |
|---|---|---|---|
| Model | Elm: `boot` / `update` / `view`, messages | `StatefulWidget` + `setState`; Provider/Riverpod/Bloc externally | React function components + hooks |
| Identity/keying | **Keyed structural reconciliation** (`runtime/src/reconcile.rs`, `09d0c9c2`) | `Key`, `GlobalKey` | React `key` |
| Type safety | **Compile-time exhaustive** — messages are enums, `match` is checked | Sound null safety; runtime widget-type errors possible | TypeScript optional; runtime errors common |
| Learning curve | Steep — Rust ownership + Elm | Moderate | Low for React developers |
| Hiring pool | **Very small** | Large | **Very large** |

**Read:** Aura's model is the most correct and the least hireable. That is an acceptable trade
for a systems-UI framework owned by one organisation (VYOM OS) and a fatal one for a framework
seeking third-party app developers. This directly constrains the strategy in §8.1.

### 5.5 Text

| | **Aura** | **Flutter** | **React Native** |
|---|---|---|---|
| Shaping | `cosmic-text` 0.19 (rustybuzz/swash lineage) | HarfBuzz | Platform (CoreText / Android `Paragraph`) |
| Line breaking | `core/src/text/line_break.rs` + `unicode-segmentation` | ICU | Platform |
| Editing | `text_editor`, selection, `core/src/text/editor.rs` | `EditableText`, full | Platform `TextInput` |
| IME / preedit | **Yes** — real preedit overlay with selection spans (`winit/src/window.rs:363-390`) | Full | Platform-native |
| Bundled font | **None — `assets/fonts/` contains one README** | Roboto bundled | Platform system fonts |
| Rich text | `rich_text`, `markdown`, `aura-highlighter` (1,868 LOC, dependency-free) | `TextSpan`, `RichText` | Nested `<Text>` |

**Read:** The text stack is a genuine strength — `cosmic-text` is the right choice, IME works,
and `aura-highlighter` being dependency-free is a nice piece of engineering. The missing bundled
font is a real shipping problem: an app built with Aura today renders with whatever the system
provides, which means it looks different on every machine. Flutter bundles Roboto precisely to
avoid that.

### 5.6 Input and gestures

| | **Aura** | **Flutter** | **React Native** |
|---|---|---|---|
| Disambiguation | **Gesture arena** — `aura-gesture` (3,844 LOC), wired into `runtime/src/pointer_router.rs` and `winit/src/lib.rs:1182` | Gesture arena (the model Aura's is named after) | `PanResponder` / `react-native-gesture-handler` |
| Recognisers | tap, pan, pinch, rotate, swipe, long-press, nested scroll | Full set | Full set (via RNGH) |
| Nested scroll | Yes | Yes (slivers, `NestedScrollView`) | Yes |
| Pointer model | Unified `core/src/pointer.rs` across mouse/touch/stylus | Unified `PointerEvent` | Platform events, normalised |

**Read:** **At parity, and this is the most complete of Aura's differentiating subsystems.**
`audit.md`'s P0-6 ("gesture arena is not the default widget interaction path") is **now closed** —
the arena is constructed per-pointer-down in the winit event loop and routed through
`Router`'s capture table. Verified at `winit/src/lib.rs:1176-1184`.

### 5.7 Accessibility

| | **Aura** | **Flutter** | **React Native** |
|---|---|---|---|
| Model | **Retained semantic tree, incrementally diffed** (`aura-accessibility`, 4,904 LOC) | `SemanticsNode` tree, also diffed | **Free** — real platform views carry native a11y |
| Declared where | **In the component spec** — a widget cannot ship without a role | `Semantics` widget, opt-in per widget | Platform + `accessibilityLabel` props |
| Reaches a screen reader | **No on the default path** — see F-15 | Yes: TalkBack, VoiceOver, and desktop ATs | Yes, natively |
| Focus management | `focus.rs` (744 LOC), scopes, traps | `FocusNode`, `FocusScope` | Platform |
| Live regions | `live_region.rs` (394 LOC) with a privacy policy | `liveRegion` | `accessibilityLiveRegion` |

**Read:** Aura's a11y *design* is the best of the three — roles come from the component
specification, so omission is a compile-time impossibility rather than a code-review miss. React
Native's a11y is the best in *practice*, because it gets the platform's for free. Aura's design
advantage is currently worth nothing on the default path, because the D-Bus transport that would
carry it to Orca is behind a non-default feature flag (F-15). This is a small fix with a large
payoff — it converts a design claim into a demonstrable one.

### 5.8 Platform reach — the decisive table

| Target | **Aura** | **Flutter** | **React Native** |
|---|---|---|---|
| Android | ❌ **none** | ✅ first-class | ✅ first-class |
| iOS | ❌ **none** | ✅ first-class | ✅ first-class |
| Linux desktop (X11) | ✅ | ✅ | ⚠️ community |
| Linux desktop (Wayland) | ✅ | ✅ | ⚠️ community |
| VYOM OS (Smithay/Wayland) | ✅ **unique** | ❌ | ❌ |
| Windows | ⚠️ winit supports it; untested here | ✅ | ⚠️ Microsoft, out-of-tree |
| macOS | ⚠️ winit supports it; untested here | ✅ | ⚠️ Microsoft, out-of-tree |
| Web / WASM | ⚠️ `webgl` feature + `web-sys` inherited from iced; unexercised | ✅ CanvasKit / WASM | ⚠️ `react-native-web` |
| Embedded / no-GPU | ✅ **`tiny_skia`, unique among the three** | ❌ | ❌ |

**This is the table that decides the strategy.** A grep of the entire tree for `android`,
`target_os = "ios"`, `ndk`, or `UIKit` returns **exactly one hit** —
`aura-design/src/platform.rs:33`, a `cfg!` used to pick a visual style. There is no mobile
platform code anywhere in this repository.

Aura ships `scaffold`, `drawer`, `bottom_sheet`, `bottom_nav_bar`, `app_bar`, `safe_area`,
`pull_to_refresh`, and a `mobile_shell` example — **a mobile component vocabulary with no mobile
platform underneath it.** Every one of those widgets is currently exercised only by a mouse on
a desktop. `safe_area` in particular cannot be correct, because nothing in the tree reads a real
device's display cutout or home indicator inset.

### 5.9 Memory and binary

| | **Aura (measured)** | **Flutter** | **React Native** |
|---|---|---|---|
| Runtime | **None — native, no VM** | Dart VM + AOT snapshot | Hermes VM + bytecode |
| GC | **None** | Generational | Hades (concurrent generational) |
| Trivial-app release binary | **20.54 MiB unstripped / 15.77 MiB stripped** (Linux x86-64, `mobile_shell`) | UNVERIFIED here | UNVERIFIED here |
| Idle RSS in benchmark harness | **283.8 MiB** (`idle_shell`; harness included) | UNVERIFIED here | UNVERIFIED here |
| Dependency count (normal graph) | **244** | — | npm graph typically far larger |

**Read:** The runtime model is Aura's structural advantage and it is **completely unproven**.
20.54 MiB for a trivial Linux desktop binary is not obviously better than a Flutter APK, and the
comparison has never been made. Until §11 is executed, "no GC, therefore smaller and faster" is
a hypothesis with a plausible mechanism and zero evidence.

### 5.10 Developer experience

| | **Aura** | **Flutter** | **React Native** |
|---|---|---|---|
| Hot reload | `hot` feature via `cargo-hot`, "very experimental" per `Cargo.toml:56` | **Stateful hot reload, sub-second, the category benchmark** | Fast Refresh, near-instant |
| Inspector | ❌ `aura_lab` has 11 modules of overlays but **no application** — `docs/tools/aura-lab.md` says so outright | DevTools: widget inspector, layout explorer, timeline, memory, CPU | React DevTools + Hermes debugger |
| Profiler | ✅ **Real** — `aura-profiler`, closure-based zero-cost-when-off, Chrome trace export | DevTools timeline | Systrace / Hermes profiler |
| Examples | **1** (`mobile_shell`) + 1 showcase | Hundreds, plus samples repo, plus codelabs | Thousands |
| Package published | ❌ not on crates.io | ✅ pub.dev | ✅ npm |
| Project scaffolding | ❌ none | `flutter create` | `npx react-native init` / Expo |
| Compile/iterate time | **44.9 s release build of one example** (cold, 20 cores) | Sub-second hot reload | Sub-second Fast Refresh |

**Read:** **This is the widest gap of any row in this document, and it is the one least
discussed in the repository.** Iteration speed is why Flutter won mindshare. A Rust GUI
framework starts with a structural disadvantage here — compile times — and Aura currently adds
to it by having no inspector, no template, and one example. The profiler is genuinely good and
is the seed of the answer.

### 5.11 Scorecard

Scored 1–5 on architecture and current implementation. "Arch" is the ceiling the design allows;
"Impl" is what the tree does today.

| Dimension | Aura arch | Aura impl | Flutter | React Native |
|---|:--:|:--:|:--:|:--:|
| Rendering correctness & breadth | 4 | **2** | 5 | 4 |
| Frame-path performance | 5 | **4** | 4 | 3 |
| Memory model | 5 | **?** | 3 | 3 |
| Idle / power behaviour | 5 | **4** | 3 | 3 |
| Layout | 4 | 4 | 5 | 4 |
| Text & i18n | 4 | 3 | 5 | 4 |
| Gestures | 5 | **4** | 5 | 4 |
| Accessibility design | **5** | **1** | 4 | 5 |
| Component breadth | 3 | **2** | 5 | 4 |
| Visual polish & effects | 4 | **1** | 5 | 4 |
| Platform reach | 3 | **1** | 5 | 5 |
| Developer experience | 3 | **1** | 5 | 5 |
| Ecosystem | 2 | **1** | 5 | 5 |
| Type safety / correctness | **5** | **5** | 3 | 2 |
| Test discipline | **5** | **5** | 4 | 3 |
| **Mean** | 4.1 | **2.5** | 4.4 | 3.9 |

The gap between *Aura arch 4.1* and *Aura impl 2.5* is the entire content of this document. The
design is competitive. The build is 60% of the way to the design.

---

## 6. Detailed audit

24 findings. Severity: **P0** blocks any competitive claim or public release · **P1** blocks
production use · **P2** blocks polish/adoption · **P3** hygiene.

Every finding carries the evidence that produced it. Anything not directly observable in the
tree is marked as such.

### 6.1 Integrity of claims — *fix this first*

> This is the highest-priority group in the document. A framework that overstates its own
> measurements loses the argument the first time somebody checks, and every genuine advantage in
> §5 goes down with it. All six findings here are cheap to fix and none require engineering.

---

**F-01 · P0 · README performance numbers contradict the committed baselines**

*Evidence:* `README.md:25-27` claims 8.9 µs / 4.4 µs / ~1,800×.
`benchmarks/baseline/ten_thousand_rows.json` → `median.frame_time.p95 = 10,251 ns`.
`benchmarks/baseline/idle_shell.json` → `6,376 ns`. Worst scene (`virtual_list`) → 33,792 ns,
a 493× margin, not 1,800×.

*Impact:* The README states "Every number below comes from `tools/aura_bench` and can be
reproduced with the command beneath it. Nothing here is estimated." The commands beneath it do
not produce those numbers. This is the single most damaging fact in this repository.

*Fix:* Regenerate every number from committed baselines, or commit the runs that produced them.
Publish the **worst** scene alongside the best. Recompute the margin from the worst case.

---

**F-02 · P0 · Baselines cannot be attributed to a registered device**

*Evidence:* all 13 baselines record `environment.os = "macos"`, `device = "dev-workstation-01"`.
`benchmarks/devices.toml:15-27` registers that name as Linux/Ubuntu, kernel 7.0, Intel Iris Xe.

*Impact:* `docs/performance/methodology.md` forbids comparing across unlike environments. These
reports belong to no environment. Every baseline in the repository is unusable as a reference
point until re-captured.

*Fix:* Re-capture all 13 on one machine, from a clean tree, and register that machine truthfully.

---

**F-03 · P0 · Baselines were captured from a dirty tree two commits behind HEAD**

*Evidence:* `git_commit = "30b5e969…-dirty"` in all 13 files; HEAD is `bf065be7`.

*Fix:* Teach `--save` to refuse a `-dirty` commit outright. The harness already refuses reports
with unattributable identity fields (`tools/aura_bench/src/report.rs:119-124`); extend that
guard to cover tree cleanliness.

---

**F-04 · P1 · README quotes allocation figures from an uncommitted run**

*Evidence:* `README.md:29` claims "197,846 alloc / 197,843 free, net delta 3". All 13 baselines
record `allocation_counting_active: false` and every allocation field reads zero.

*Note:* the harness documents this exact trap in `tools/aura_bench/src/report.rs:70-76`. The
tool is more honest than the README.

*Fix:* Commit a `--features profiling` run, or drop the claim.

---

**F-05 · P1 · `docs/performance/aura-1.0-report.md` presents numbers with no provenance**

*Evidence:* the file states "Launcher Scroll: 60fps solid, 2ms frame time", "Settings
Navigation: 60fps solid, 1.5ms", "Complex Grid: 60fps, 3ms", on "VYOM Reference Hardware,
8-core ARM64, 8GB". No such device exists in `benchmarks/devices.toml`, no ARM64 run exists in
`benchmarks/baseline/`, and no scene by those names exists in `tools/aura_bench/src/scenes/`.
The file also contains the literal line `(Insert benchmark data from compare-frameworks.sh vs
Qt/Flutter)`.

*Impact:* A document titled "Aura 1.0 Performance Report" containing invented device names and
unsourced frame times is the most quotable thing in the repository and the least defensible.

*Fix:* Delete it, or replace its body with a single line: *"Not yet measured. See
`benchmarks/baseline/`."*

---

**F-06 · P2 · `docs/performance/methodology.md` analyses a metric that no longer exists**

*Evidence:* it discusses `text_heavy` P95 "~2.44 ms to ~3.56 ms"; the committed baseline says
8.04 µs. The document predates the removal of the ~2.5 ms `present` readback from the headline.

*Impact:* Its conclusion — governor-dependent P95 instability — is still valid and still
matters. Its numbers mislead by three orders of magnitude.

*Fix:* Rewrite the numeric sections against `frame_time`; keep the analysis.

---

### 6.2 Rendering pipeline

---

**F-07 · P0 · The GPU effect pipeline is a literal empty stub**

*Evidence:* `wgpu/src/effects/pipeline.rs` in full is a `pub struct Pipeline {}` with
`prepare()` (`// Stub`, line 14) and `render()` (`// Stub implementation for rendering effects`,
line 18) doing nothing. There are **zero WGSL shaders** for any effect: `find . -name '*.wgsl'`
returns 11 files, all inherited quad/triangle/image/blit pipelines. The only `blur` in any
shader is the quad drop-shadow inherited from iced.

Meanwhile `wgpu/src/effects/` contains **nine** fully-written modules — `blur.rs`,
`backdrop_blur.rs`, `color_matrix.rs`, `saturation.rs`, `noise.rs`, `mask.rs`, `shadow.rs`,
`pool.rs`, `pipeline.rs` — implementing `Effect::bounds()`, tier gating, sampling expansion, and
a resource pool. All of it computes *what an effect would cost and how far it would sample*.
None of it draws.

*Impact:* Blur, backdrop blur, colour matrix, saturation, and noise **do not render**. The
`blur_layers` and `effect_ladder` benchmark scenes measure a cost model, not pixels — which is
why `effect_ladder` is the *cheapest* scene in the suite at 7.62 µs. Against Flutter's
`BackdropFilter` / `ImageFilter` / `ShaderMask` and React Native's platform blur views, Aura
today renders no effects at all. Modern mobile UI is built out of exactly these.

*Fix:* Author the WGSL. Two-pass separable Gaussian for blur; backdrop capture → downsample →
blur → upsample for backdrop blur; a 5×4 matrix fragment shader covers colour matrix and
saturation together. The bounds/tier/pool machinery is already correct and already tested —
this is shader work against a finished interface, not a redesign. **4–6 weeks.**

---

**F-08 · P0 · GPU partial present is a no-op; damage is computed, logged, and discarded**

*Evidence:* `wgpu/src/window/compositor.rs:396-436`. `present_with_damage` computes rect counts
and surface coverage percentages, emits them at `log::debug!`, then at line 427 comments *"For
now, wgpu does not natively expose partial-present / damage hints in its surface API. We fall
back to a full present"* and calls plain `present()`.

By contrast the software path is real: `tiny_skia/src/window/compositor.rs:163-183` diffs and
groups damage and genuinely redraws only the damaged region.

*Impact:* **"Damage-driven repaint" — the README's second headline advantage — is not true on
the GPU path**, which is the default path (`default = ["wgpu", …]`). Every frame that reaches a
screen is a full-surface present. The entire damage subsystem (`graphics/src/damage.rs`,
`runtime/src/invalidation.rs`, `core/src/damage.rs`) currently only avoids *CPU* paint work; it
saves no GPU bandwidth and no power.

*Fix:* Two routes, both real. (a) Wayland: `wl_surface.damage_buffer` via the VYOM backend and
`raw-window-handle`, which is where the power win actually lives on the target platform.
(b) `VK_KHR_incremental_present` / `EGL_KHR_swap_buffers_with_damage` beneath wgpu, or a
scissored render pass that skips undamaged tiles. **Route (a) first** — it is the platform Aura
actually ships on. Until one of them lands, remove the claim from the README.

---

**F-09 · P1 · Retained layers exist, but the granularity is the entire frame**

*Evidence:* the only caller of the retained-layer API in the whole tree is
`runtime/src/user_interface.rs:636-655`, which keys **one** layer on
`hash(root_state_id) + root_revision` covering the full viewport. `wgpu/src/lib.rs:806-836`
holds exactly one `retained_frame` and evicts it whenever key, revision, bounds, or scale
factor differ. No widget anywhere calls `start_retained_layer`.

*Impact:* Retention is all-or-nothing. Change one pixel of state anywhere in the tree and the
entire display list re-records. Flutter's `RepaintBoundary` is per-subtree and developer-placed;
Aura's is per-application. The cache's hit rate in any interactive application is approximately
zero, and `RetainedLayerCache`'s budget, LRU, generation-purge, and telemetry machinery
(`graphics/src/retained_layer.rs:208-380`) is unreachable — the wgpu backend stores a single
`Option`, not the cache.

*Fix:* Push retention down to subtree boundaries. `container`, `scaffold`, `scrollable`
viewports, and list items are the natural boundaries. Reuse the existing dirty-flag propagation
(`8b39ad99`) to decide which subtrees kept their revision. Then wire `RetainedLayerCache` in
place of the single-slot `Option`.

---

**F-10 · P0 · There is no visual truth set — zero golden images, zero pixel comparisons**

*Evidence:* `tests/golden/harness.rs` is 600+ lines of genuinely excellent design: it refuses to
auto-adopt a missing golden, treats `Outcome::Missing` as a failure, separates `Exact` from
`Approximate` tolerance, and requires `AURA_UPDATE_GOLDEN` as a deliberate act. And:

- `tests/golden/images/` **does not exist**. Zero reference images are committed.
- `grep -c 'compare(' tests/golden/*.rs` excluding the harness → **0**. Nothing calls it.
- The seven "golden" suites (`components.rs`, `design_system.rs`, `effects.rs`,
  `gpu_primitives.rs`, `damage_motion.rs`, `multilingual_text.rs`, `transitions.rs`) are
  **property oracles**, not pixel captures. `components.rs:1-12` says so explicitly.

The property oracles are good — leap years, month lengths, touch-target floors, gesture
thresholds, badge growth bounds. They are not a visual regression suite.

*Impact:* **Nothing in this repository can detect a visual regression.** A change that renders
every button as a black rectangle passes all 1,705 tests. There is no evidence that Aura's UI
looks like anything at all. For a project whose stated goal is beating Flutter *on UI*, this is
the most consequential gap after the missing mobile platform.

*Fix:* The harness is done; only the cases are missing. Add real render-and-compare cases for
every widget × theme × state, run them under `xvfb` with a pinned software adapter (`llvmpipe`)
so they are hardware-independent, and commit the `.rgba` files. `.github/workflows/golden.yml`
already exists and already uploads diffs on failure. **2–3 weeks.**

---

**F-11 · P1 · Image decoding is synchronous on the UI thread; the async decoder is dead code**

*Evidence:* `wgpu/src/image/decode.rs` implements `Decoder`, `DecodeRequest`, `DecodeHandle`,
`DecodeResult`, `PendingRequest`, a thread count, a bounded queue, `submit`, `poll`, `cancel`,
and `decode_image`. `grep -rn 'decode::' wgpu/src --include='*.rs'` excluding the file itself
returns **nothing**. `cargo clippy` reports every one of these as never constructed / never used.

*Impact:* Decoding a JPEG blocks the frame. `image_grid` is the second-slowest scene (15.71 µs
p95, draw-dominated at 12.96 µs) and it will get much worse with real assets. Flutter decodes on
a background pool; React Native gets platform-native off-thread decode for free. This is the
concrete instance of the single-threaded-frame ceiling identified in §5.2.

*Fix:* Wire `Decoder` into `wgpu/src/image/cache.rs`. Return a placeholder on miss, upload on
`poll`, invalidate the region when the decode lands. The code is written; it needs a caller.

---

**F-12 · P2 · The pipeline cache is a one-shot builder, not a cache**

*Evidence:* `wgpu/src/engine.rs:39-105` constructs `PipelineCache::new(32)`, uses it four times
to build the quad/text/triangle/image pipelines, stores it, and never reads it again. Clippy:
*"field `pipeline_cache` is never read"*, *"methods `invalidate_all`, `telemetry`, and
`resource_generation` are never used"*.

*Impact:* Minor today — Aura has four static pipelines. It becomes load-bearing the moment F-07
lands and effects need pipeline variants per format/tier.

*Fix:* Either consult it from the effect pipeline (after F-07) or reduce it to a builder and
delete the dead surface. Do not leave it as decoration.

---

### 6.3 Components and the design system

---

**F-13 · P0 · 17 of 41 widget catalogs have no design-system theme — and they are exactly the mobile set**

*Evidence:* 41 `pub trait Catalog` declarations across `widget/src/`. `widget/src/aura.rs`
implements **24** of them for `aura_design::Theme`. The 17 with no implementation:

```
accordion   avatar        badge      bottom_nav_bar   bottom_sheet
carousel    chip          date_picker  drawer         pull_to_refresh
search_bar  segmented_control  skeleton  snackbar     stepper
tabs        time_picker
```

*Impact:* These are **precisely the components built to differentiate Aura** — the mobile set
the README leads with. Under the `aura` feature they fall back to `aura_core::Theme` catalogs
declared in their own modules, so `mobile_shell` and `aura-showcase` compile and run; but the
design system does not reach them. The README's claim that *"there is no second styling path to
drift out of sync"* describes the intent, not the tree: there are two paths today, and the new
components are all on the wrong one.

*Fix:* 17 `impl Catalog for Theme` blocks in `widget/src/aura.rs` against the tokens in
`aura-design/src/components/`. Mechanical, high-value, roughly one week. Then add a compile-time
test that fails when a `Catalog` exists without a `Theme` impl, so this cannot recur.

---

**F-14 · P0 · There are no icons and no bundled font**

*Evidence:* `assets/icons/` contains `manifest.toml` and nothing else — two entries (`add`,
`back`), both with `license = "TBD-before-asset-import"`, and **zero SVG files**.
`assets/fonts/` contains one `README.md`.

`examples/mobile_shell/src/main.rs:36-40` works around it with Unicode glyphs — `\u{2302}` (⌂)
for Home, `\u{2315}` (⌕) for Search, `\u{263A}` (☺) for Profile.

*Impact:* Flutter bundles 2,000+ Material icons and Roboto. React Native has
`react-native-vector-icons` with a dozen icon families. Aura has three approximate Unicode
glyphs whose appearance depends entirely on which fonts the host happens to have installed. **No
application built on Aura today can look finished**, and no two machines will render it alike.

*Fix:* Adopt an MIT/Apache-licensed icon set (Lucide, Feather, or Material Symbols under Apache
2.0), commit the SVGs with their licence records, generate a typed `icon::` module, and bundle
one variable font. This is a licensing and build-script task, not a rendering one, and it does
more for perceived UI quality per hour than anything else in this document.

---

**F-15 · P0 · Accessibility never reaches assistive technology on the default path**

*Evidence:* `aura-accessibility/src/atspi.rs:23` gates the entire D-Bus transport behind
`#[cfg(feature = "atspi")]`. `aura-accessibility/Cargo.toml:16-21` declares that feature
**off by default** (`default = []`). The only manifest in the tree that enables it is
`aura-platform-vyom/Cargo.toml:15` — itself an optional, non-default backend.

The default `winit` path *does* construct a session:
`winit/src/window.rs:93-95` → `Session::new(atspi::adapter::Adapter::new())`, and
`winit/src/lib.rs:939-947` publishes semantic diffs and focus changes into it every frame. But
without the `atspi` feature there is no `server` module, no zbus connection, and no bus task
draining the queue. The semantic tree is diffed, translated, and pushed into an in-memory
`VecDeque` that nothing reads.

*Impact:* Aura's most distinctive design claim — *"accessibility by construction"* — is
currently a claim about an in-memory data structure. **Orca will not read an Aura application.**
The tree, the diffing, the focus engine, the live-region privacy policy, the AT-SPI vocabulary
(451 LOC), and the D-Bus server (804 LOC) are all written and tested; they are simply not
connected on the path any user takes.

*Fix:* Add an `accessibility` feature to the `aura` facade that turns on
`aura-accessibility/atspi`, spawn the bus task from `winit`'s event loop, and **verify against a
real Orca session** — the only test that counts. Small change, very large credibility payoff.

---

**F-16 · P1 · Component gap against both competitors**

Not built, per `README.md` and confirmed by absence from `widget/src/`: calendar range
selection, tree view, pagination, breadcrumb, rating, video, web view, map. Also absent and
unlisted: data grid with virtualised columns, autocomplete with async sources, rich-text editor
(as distinct from `text_editor`), file picker, colour picker, charting primitives.

Also: `date_picker` reports **one semantic node naming the chosen date, not one node per day**.
A screen-reader user cannot navigate the grid.

Full gap table in §7.

---

### 6.4 Platform

---

**F-17 · P0 · No mobile platform exists**

*Evidence:* a tree-wide grep for `android`, `target_os = "ios"`, `ndk`, `UIKit` returns exactly
one hit, `aura-design/src/platform.rs:33`, a `cfg!` used to choose a visual style. No NDK glue,
no `NativeActivity`, no JNI, no `UIViewController`, no touch-from-platform path, no display
cutout or safe-area inset source, no `Cross.toml` mobile targets (it lists only
`aarch64-unknown-linux-gnu` and `armv7-unknown-linux-gnueabihf`).

*Impact:* **Aura cannot run on a phone.** Flutter and React Native are mobile-first frameworks;
mobile is the ground they are defended on. Aura ships a mobile *vocabulary* — `scaffold`,
`drawer`, `bottom_sheet`, `bottom_nav_bar`, `safe_area`, `pull_to_refresh` — validated only by a
mouse on a desktop. `safe_area` cannot be correct, because nothing in this tree can tell it
where a notch is.

*Fix:* This is a strategic fork, not a bug. See §8.1 — the recommendation is **not** to chase
Android/iOS, but to own VYOM/Linux and *stop describing Aura in mobile terms* until a mobile
backend exists.

---

**F-18 · P1 · The VYOM backend is a side crate, not the application path**

*Evidence:* `aura-platform-vyom` is 1,608 LOC across 13 modules — `event_loop`, `surface`,
`input`, `ime`, `clipboard`, `drag_drop`, `frame_clock`, `power`, `gesture_bridge`,
`accessibility`, `capabilities`, `window_protocol`. It is an **optional dependency** of the
`aura` facade behind a non-default `vyom` feature (`Cargo.toml:93`), and the `application()`
entry point runs on `aura_winit`.

*Impact:* The one platform where Aura has a structural advantage — direct Smithay/Wayland with
capability-gated privileged protocols — is not the platform `aura::application()` targets. It
is also the only path where `atspi` is enabled (F-15) and the only realistic route to real
damage-based partial present (F-08). **Three separate P0s converge on this one crate.**

*Fix:* Make `vyom` the default backend on VYOM targets and route `application()` through it.
`docs/platform/vyom-protocol.md` already specifies the protocol.

---

**F-19 · P2 · Web/WASM is inherited and unexercised**

*Evidence:* `webgl` feature, `web-sys 0.3.85`, `wasm32` target blocks in `futures/` and
`winit/`, and `deny.toml` lists `wasm32-unknown-unknown` — all inherited from iced. No CI job
builds for wasm32, no example targets it, no test exercises it.

*Impact:* Unknown whether it works. Should not be listed as a supported target.

---

### 6.5 CI and process

---

**F-20 · P0 · `cargo fmt --check` fails — CI on `main` is red**

*Evidence:* 68 diff sites across ~30 files, including `wgpu/src/effects/pipeline.rs`,
`wgpu/src/image/decode.rs`, `winit/src/lib.rs`, `aura-showcase/src/main.rs`,
`examples/mobile_shell/src/main.rs`, and 20 widget modules.

*Impact:* `.github/workflows/ci.yml` gates `test` and `release-build` on `needs: [fmt, check,
clippy]`. With `fmt` failing, **the test and release jobs never run on `main`**. CI exists, is
well written, and is currently reporting nothing.

*Fix:* `cargo fmt --all`. One commit.

---

**F-21 · P0 · `cargo clippy -- -D warnings` fails with 15 errors**

*Evidence:* all 15 in `aura_wgpu` — 3 × `manual_let_else` (`wgpu/src/engine.rs:44, 57, 70`,
denied by workspace lints) and 12 dead-code warnings promoted to errors by `-D warnings`. The
dead code is F-11's image decoder (8) and F-12's pipeline cache (4).

*Impact:* Same as F-20 — the `clippy` job is a `needs:` dependency of `test` and
`release-build`.

*Fix:* Rewrite the three matches as `let…else` (clippy prints the exact replacement). For the
dead code, wire it (F-11, F-12) rather than `#[allow]` it — an `#[allow(dead_code)]` here would
suppress the only signal that a whole subsystem is disconnected.

---

**F-22 · P1 · The budget script fails and is never run by CI**

*Evidence:* `bash scripts/ci/check-budgets.sh` → 2 of 4 fail:

```
FAIL: dependency count 244 exceeds budget 190
FAIL: mobile_shell is 21536248 bytes, over the 16777216 byte budget
```

And `grep -rn 'check-budgets' .github/workflows/` returns **nothing**. Only
`check-workspace.sh` is invoked (`ci.yml:25`).

*Impact:* The script's own header says an earlier version *"printed 'Binary size limits
verified' … without measuring anything, which is worse than having no script at all."* The
current situation is worse still: it measures correctly, fails correctly, and no one is
listening. Dependencies have grown from a measured 175 to 244 (+39%) and the example binary from
10 MiB to 20.54 MiB (+105%) with nothing to stop either.

*Fix:* Add a `budgets` job to `ci.yml`. Then either bring the numbers under budget or raise the
thresholds deliberately, in a commit that states why — which is what the script's header asks for.

---

**F-23 · P0 · The release binary is 2× what the README claims and over the project's own budget**

*Evidence:* measured today, `cargo build -p mobile_shell --release`:

| Measurement | Bytes | MiB |
|---|---:|---:|
| **Actual, unstripped** | 21,536,248 | **20.54** |
| **Actual, stripped** | 16,533,088 | **15.77** |
| README claim (`README.md:30`, "unstripped") | ~10,800,000 | 10.3 |
| Project budget (`check-budgets.sh:33`) | 16,777,216 | 16.0 |

*Impact:* The README understates the shipped size by **2×**. `check-budgets.sh:29-33` calls this
*"the number a Flutter/React Native comparison will be held to"* — and it is over budget even
before F-07 (effects), F-14 (icons and a bundled font) add to it. Binary size is one of the
three axes where a no-GC framework should win; right now it is unmeasured against competitors
and moving in the wrong direction.

*Fix:* Update the README to the measured number today. Then reduce it: `panic = "abort"`,
`lto = "fat"`, `codegen-units = 1`, `strip = true`, `opt-level = "z"` for the release profile;
audit the 244-crate graph (F-22) for the 20 heaviest; make `image` codecs opt-in. Re-measure and
budget the stripped number, since that is what ships.

---

**F-24 · P2 · One example, no package, no template**

*Evidence:* `examples/` contains exactly one member, `mobile_shell`. Upstream iced ships ~80.
The crate is not published (version `0.15.0-dev`). There is no `cargo generate` template and no
scaffolding command. `CHANGELOG.md`'s `[1.0.0] - Unreleased` section is prose about features
("Advanced flexbox-based layout system"), not a changelog.

*Impact:* Nobody outside this repository can start an Aura project. `aura = { version =
"0.15.0-dev" }` in the README's quick start does not resolve from crates.io.

---

### 6.6 Findings closed since `audit.md`

Credit where it is due — these were open in `audit.md` and are **now closed**, verified today:

| Was | Now |
|---|---|
| **P0-6** Gesture arena not the default interaction path | ✅ Closed — arena constructed per pointer-down at `winit/src/lib.rs:1176-1184`, routed via `runtime/src/pointer_router.rs` |
| **P0-10** CI completely disabled | ✅ Closed — 5 workflows exist (`ci`, `perf`, `golden`, `fuzz`, `licenses`) and are well designed (though red, F-20/F-21) |
| **P0-1** Retained rendering implemented but not integrated | ⚠️ Partly — now called from `user_interface.rs:642`, but at whole-frame granularity (F-09) |
| **P1** Reviewability and roadmap discipline | ✅ Closed — `phase.md`, `docs/architecture/*`, per-phase commit splits are exemplary |

---

## 7. Component inventory and gap analysis

### 7.1 What ships today

`widget/src/helpers.rs` exposes **75 `pub fn` constructors**, verified. Read honestly, that
number is **61 actual widgets plus 14 alignment and utility helpers** (`center`, `center_x`,
`center_y`, `bottom`, `bottom_center`, `bottom_right`, `right`, `right_center`, `hover`,
`opaque`, `void`, `value`, `span`, `aura`). A handful more live in their own modules —
`table::table`, `rule::horizontal`/`vertical`, `markdown::view` — and `sliver` and `navigator`
are type-based rather than constructor-based.

| Family | Components | Design-system themed? |
|---|---|---|
| **Navigation & structure** | `scaffold`, `drawer`, `bottom_sheet`, `bottom_nav_bar`, `tabs`, `app_bar`, `navigator`, `carousel`, `accordion` | ⚠️ only `app_bar` |
| **Input** | `button`, `checkbox`, `radio`, `toggler`, `slider`, `vertical_slider`, `text_input`, `text_editor`, `pick_list`, `combo_box`, `search_bar`, `stepper`, `segmented_control`, `chip`, `date_picker`, `time_picker`, `icon_button` | ⚠️ 11 of 17 |
| **Display** | `text`, `rich_text`, `markdown`, `image`, `svg`, `canvas`, `qr_code`, `table`, `avatar`, `badge`, `skeleton`, `progress_bar`, `tooltip`, `snackbar`, `list_tile`, `dialog`, `rule` | ⚠️ 12 of 17 |
| **Layout** | `column`, `row`, `grid`, `stack`, `container`, `scrollable`, `pane_grid`, `virtual_list`, `sliver`, `responsive`, `adaptive`, `safe_area`, `float`, `pin`, `constrain`, `space`, `keyed_column` | n/a (unstyled) |
| **Gestures & platform** | `pull_to_refresh`, `mouse_area`, `sensor`, `transition`, `themer`, `shader` | ⚠️ only `float` |

**Verified quality signals:** every widget carries unit tests; every interactive control is
pinned against the 48 px `aura_core::touch::MIN_TARGET` floor by
`tests/golden/components.rs:140`; and roles/actions come from `aura-design`'s component specs
rather than from each widget's own opinion.

**Verified quality gaps:** 17 of 41 catalogs unthemed (F-13); no icons (F-14); no pixel test
anywhere (F-10); `date_picker` exposes one semantic node instead of one per day.

### 7.2 Gap against Flutter (Material 3 + Cupertino)

Flutter ships several hundred documented widgets. This table lists what an ordinary application
reaches for and Aura does not have.

| Category | Missing in Aura | Priority |
|---|---|---|
| **Iconography** | Any icon set at all; any bundled font | **P0** |
| **Effects** | Blur, backdrop blur, colour matrix, saturation, shader mask — *stubbed, F-07* | **P0** |
| **Lists** | Reorderable list, swipe-to-dismiss, sticky headers, grouped/sectioned lists, infinite scroll with async paging | **P0** |
| **Navigation** | Nested/declarative router, deep links, route guards, predictive back, `navigator` has no URL model | **P0** |
| **Overlays** | Menu/context menu constructor, popover, banner, bottom action sheet, modal barrier scrim policy | **P1** |
| **Data** | Data grid with virtualised + resizable + sortable columns, tree view, pagination, breadcrumb | **P1** |
| **Feedback** | Rating, refresh indicator variants, determinate/indeterminate spinner set, shimmer beyond `skeleton` | **P1** |
| **Media** | Video player, camera preview, audio, image cropping, gallery | **P2** (platform-bound) |
| **Web** | Web view | **P2** (platform-bound) |
| **Maps** | Map view, markers, tiles | **P2** (platform-bound) |
| **Pickers** | Calendar **range** selection, colour picker, file picker, multi-select | **P1** |
| **Text** | Rich-text *editor* (WYSIWYG), autocomplete with async sources, mentions, spell check | **P1** |
| **Charts** | Any charting primitive (`canvas` exists; nothing built on it) | **P2** |
| **Adaptive** | Cupertino-equivalent platform-idiomatic variants | **P2** |

### 7.3 Gap against React Native core + first-tier ecosystem

RN's own core component set is *smaller* than Aura's — `View`, `Text`, `Image`, `ScrollView`,
`TextInput`, `Pressable`, `FlatList`, `SectionList`, `Modal`, `Switch`, and a handful more. Its
advantage is not core breadth; it is that everything else is one `npm install` away, and that
its components *are* platform views.

| What RN gets from the platform / ecosystem | Aura's position |
|---|---|
| Native accessibility, free, everywhere | Designed better; **not connected** (F-15) |
| Native text selection, magnifier, autofill, spell check | Not built |
| Native keyboard avoidance & inset handling | Not built (no mobile platform, F-17) |
| Native share sheet, permissions, camera roll, biometrics | Not built |
| Native scroll physics matching the OS exactly | Own physics; unvalidated against any OS |
| `react-native-reanimated` (UI-thread animation) | **`aura-motion` is arguably better** — springs/decay/keyframes, no JS bridge to cross |
| `react-native-gesture-handler` | **`aura-gesture` is at parity** |
| ~2M npm packages | ~0 Aura-specific crates |

**The honest read:** Aura's *core component set is already competitive with React Native's core
component set*. It is not competitive with the ecosystem, and it never will be by building
components — that is a distribution problem, not an engineering one, and it argues directly for
the strategy in §8.1.

### 7.4 Build order for components

Ranked by *perceived UI quality gained per engineering week*, which is the metric that matters
for a framework judged on looks.

| Rank | Work | Why first | Est. |
|---|---|---|---|
| 1 | **Icon set + bundled font** (F-14) | Nothing else makes an app look finished; nothing else is this cheap | 1 wk |
| 2 | **17 missing catalogs** (F-13) | Makes the mobile set themeable — it already exists, it just isn't wired | 1 wk |
| 3 | **Effect shaders** (F-07) | Blur/backdrop is the visual language of modern mobile UI | 4–6 wks |
| 4 | **Golden image suite** (F-10) | Everything above is unverifiable without it; do it *in parallel* with 1–3 | 2–3 wks |
| 5 | **List family** — reorder, swipe-dismiss, sticky headers, sections | The most-used missing components in real apps | 3 wks |
| 6 | **Declarative router** with deep links and guards | `navigator` is a transition, not a routing model | 3 wks |
| 7 | **Menu / popover / context menu** constructors | Catalogs exist (`menu::Catalog` is themed); constructors do not | 1 wk |
| 8 | **`date_picker` per-day semantics** | An accessibility framework shipping an inaccessible calendar is a self-inflicted headline | 3 days |
| 9 | **Data grid** | Unlocks desktop/tooling use cases where Aura's perf actually shows | 4 wks |
| 10 | Rating, breadcrumb, pagination, tree view | Long tail | 3 wks |

---

## 8. How to actually beat Flutter and React Native

### 8.1 First: choose the battlefield

**Do not position Aura as "Flutter, but in Rust."** That framing invites exactly the comparison
Aura loses — breadth, ecosystem, tooling, hiring — and hides the three places it wins. §5.4
makes the hiring constraint concrete: an Elm-in-Rust programming model is a poor fit for
recruiting third-party app developers and an excellent fit for a systems team that owns its
whole stack.

**Position Aura as the system UI layer for VYOM OS and constrained Linux devices.** On that
battlefield the scorecard inverts:

| Requirement of that domain | Aura | Flutter | React Native |
|---|---|---|---|
| No GC pauses in the frame path | ✅ | ❌ | ❌ |
| Runs with no GPU | ✅ `tiny_skia` | ❌ | ❌ |
| Zero frames when idle (battery) | ✅ | ⚠️ | ⚠️ |
| Direct Wayland/Smithay with privileged protocols | ✅ | ❌ | ❌ |
| Memory-safe with 15 `unsafe` blocks | ✅ | ❌ C++ | ❌ C++ |
| No second language runtime to ship | ✅ | ❌ Dart VM | ❌ Hermes |
| Owned end-to-end by one org | ✅ | ❌ | ❌ |

Win that domain completely. Then, if a mobile backend is ever funded, expand from a position of
proven strength instead of arriving as the 4th-best option in someone else's market.

### 8.2 Beating them on **UI**

"Better UI" is not an opinion if it is defined as measurable properties. Four, in order:

**1. Ship the visual primitives that make modern UI look modern.** Blur, backdrop blur,
elevation with correct penumbra, colour matrix, saturation, gradient meshes. Today Aura renders
**none** of these (F-07). This is the whole gap and it is bounded shader work against a
finished, tested interface.

**2. Ship a coherent visual identity.** An icon set, a bundled variable font, motion curves,
and elevation tokens that agree with each other. `aura-design` already specifies the tokens;
`assets/` is empty (F-14). A framework whose demo app draws its navigation with `⌂ ⌕ ☺` cannot
credibly claim a UI advantage.

**3. Make correctness visible.** The 48 px touch floor pinned in a test, roles that cannot be
omitted, `MotionPolicy` honouring reduced-motion — these are real UI-quality advantages Flutter
enforces only by convention. **Nobody knows they exist.** Put them in the showcase as a visible,
switchable panel: turn on reduced motion, turn on a large text scale, turn on high contrast, and
watch every widget respond. That demo is more persuasive than any frame-time chart.

**4. Prove it never regresses.** F-10. Golden images per widget × theme × state, run in CI
against a pinned software adapter. Flutter has this. Aura has the harness and none of the cases.

**Target:** the `aura-showcase` gallery, rendered side by side against the Flutter Gallery and an
RN Paper demo, where a designer cannot tell which is which — except that Aura's runs on a device
with no GPU.

### 8.3 Beating them on **speed**

Aura's frame cost is already excellent and is **already 493× inside the 60 Hz budget on its
worst scene**. More micro-optimisation is not the win; the win is (a) making the numbers
defensible and (b) removing the ceilings that will bite when the renderer becomes complete.

| Priority | Work | Why |
|---|---|---|
| **1** | **Fix the claims** (F-01…F-06) | Currently the fastest way to *lose* a speed argument |
| **2** | **Real GPU damage** (F-08) | Frame-cost wins are CPU-side only; power wins need partial present |
| **3** | **Subtree retention** (F-09) | Whole-frame retention has ~0% hit rate in an interactive app |
| **4** | **Off-thread image decode** (F-11) | The code exists and is dead; `image_grid` is already 2nd slowest |
| **5** | **Profile `virtual_list`** | 33.79 µs p95, 21.21 µs in `update` — the one scene mobile workloads look like |
| **6** | **Split raster from build/layout** | §5.2's structural ceiling; only matters after F-07 lands, but decide the architecture *before* |
| **7** | **Make pacing metrics real** (P-5) | Uniform zeros across 13 scenes is not a measurement |

**The claim to aim for, once F-01…F-06 and §11 are done:** *"Same application, same device,
same scenes: Aura's p99 frame time is lower than Flutter's and React Native's, and Aura's p99
minus p50 is dramatically lower — because there is no collector in the frame path."*

That second half is the interesting one. **Aura's real speed story is not the mean, it is the
tail.** A GC-free frame path should show a much tighter p99/p50 ratio than either competitor.
That is a claim about a mechanism, it is falsifiable, and it is the kind of result nobody can
dismiss as benchmark-rigging. Measure the tail, not the average.

### 8.4 Beating them on **performance** (memory, startup, power, size)

These are the axes where a no-GC framework *should* structurally win, and **Aura currently
measures its own side of exactly one of them.**

| Axis | Aura today | Required to claim a win |
|---|---|---|
| **Memory** | 283.8 MiB idle RSS in the harness (§4.4) — harness-inflated, never isolated | Measure the *app*, not the harness. Steady-state RSS of `mobile_shell` vs equivalents. Report peak, not just steady. |
| **Startup** | **Never measured** | Cold-start to first frame, 20 runs, median + p95, killed between runs |
| **Power** | **Never measured** | Joules over a fixed 60 s interaction script; the zero-idle-work claim lives or dies here and it is Aura's best structural advantage |
| **Binary size** | **20.54 MiB / 15.77 MiB stripped**, 2× the README claim, over budget (F-23) | Get under 10 MiB stripped, then compare like for like |
| **Allocation churn** | Claimed net-delta-3 from an uncommitted run (F-04) | Commit a `--features profiling` run; contrast against a GC framework's allocation rate |

**Do the power measurement first among these.** "Zero idle work" is Aura's most differentiated
claim, it is architecturally true, it is trivially demonstrable (idle an app for 60 s, measure
joules), and neither competitor can match it without changing their runtime. It is also the
claim that matters most to the actual customer — a device OS.

### 8.5 What "winning" concretely looks like, in one paragraph

Twelve months from now, the persuasive artifact is not a benchmark table. It is: **a VYOM
reference device running an Aura system shell, side by side with the same shell in Flutter and
in React Native, on identical hardware, with a power meter attached and a screen reader turned
on.** Aura's is the one that idles at near-zero draw, has the flattest p99, keeps working when
the GPU is disabled, and speaks every control aloud. That demo is winnable. It requires
F-07, F-08, F-10, F-14, F-15, F-18, and §11 — and nothing about Android or iOS.

---

## 9. What is pending to make Aura a full-fledged framework

A framework is not a rendering engine. It is an engine, a component library, a platform layer, a
toolchain, a distribution channel, and a support surface. Below is every domain, with what
exists and what is missing. **Status is verified against the tree, not against `phase.md`'s
intentions.**

Legend: ✅ done · 🟡 partial · ❌ missing

### 9.1 Engine

| Item | Status | Note |
|---|:--:|---|
| Widget tree + keyed reconciliation | ✅ | `runtime/src/reconcile.rs`, commit `09d0c9c2` |
| Incremental layout with cache | ✅ | `Tree::layout_with_cache`, `1e525bd2` |
| Damage tracking & invalidation | 🟡 | Computed and correct; **discarded on GPU present** (F-08) |
| Retained layers | 🟡 | Whole-frame granularity only (F-09) |
| Frame scheduler + presentation feedback | ✅ | `runtime/src/frame_scheduler.rs`, suspend/resume, motion registry |
| GPU renderer (quad/text/triangle/image) | ✅ | 4 pipelines, batching, atlas |
| **GPU effects** | ❌ | **Empty stub** (F-07) |
| Software renderer | ✅ | `tiny_skia`, real damage-based partial redraw |
| Off-thread image decode | ❌ | Written, dead (F-11) |
| Raster/UI thread separation | ❌ | Single-threaded frame (§5.2) |
| Quality tiers / adaptive degradation | 🟡 | `graphics/src/quality.rs` policy exists; nothing to degrade yet |
| Memory pressure & recovery | ✅ | `runtime/src/memory_pressure.rs`, `recovery.rs` |

### 9.2 Components

| Item | Status | Note |
|---|:--:|---|
| Core control set | ✅ | 61 widgets + 14 helpers |
| Design-system theming | 🟡 | **24 of 41 catalogs** (F-13) |
| Icon set | ❌ | Manifest with 2 TBD-licensed entries, 0 SVGs (F-14) |
| Bundled font | ❌ | `assets/fonts/` is one README (F-14) |
| Dark/light themes | ✅ | Both, throughout |
| Reduced motion | ✅ | `aura-motion/src/reduced_motion.rs`, honoured by `skeleton` and others |
| Text scaling / large fonts | 🟡 | `docs/guide/scaling.md` exists; no end-to-end test |
| RTL / bidi | 🟡 | `mirror_rtl` in the icon manifest; no RTL layout test found |
| List family (reorder, dismiss, sticky, sections) | ❌ | §7.4 rank 5 |
| Routing model | ❌ | `navigator` animates; it does not route (§7.2) |
| Menus / popovers | 🟡 | `menu::Catalog` themed; no constructor |

### 9.3 Platform

| Item | Status | Note |
|---|:--:|---|
| Linux X11 | ✅ | via winit |
| Linux Wayland | ✅ | via winit |
| VYOM native (Smithay) | 🟡 | 1,608 LOC, **not the default path** (F-18) |
| Windows / macOS | 🟡 | winit supports them; **untested here, unclaimed** |
| **Android** | ❌ | **No code exists** (F-17) |
| **iOS** | ❌ | **No code exists** (F-17) |
| Web / WASM | 🟡 | Inherited from iced, unexercised (F-19) |
| Embedded / framebuffer | 🟡 | `tiny_skia` makes it possible; no backend written |
| IME | ✅ | Real preedit with selection spans |
| Clipboard | ✅ | Both backends |
| Drag & drop | 🟡 | `aura-platform-vyom/src/drag_drop.rs` only |
| Display cutouts / safe-area insets | ❌ | `safe_area` widget has no inset source |
| Multi-window | ✅ | `runtime/src/window.rs`, per-window state |
| Power / thermal policy | 🟡 | `runtime/src/power_policy.rs` exists; unmeasured |

### 9.4 Accessibility

| Item | Status | Note |
|---|:--:|---|
| Retained semantic tree + diffing | ✅ | 4,904 LOC, incremental |
| Roles/actions from component spec | ✅ | Best-in-class design |
| Focus engine, scopes, traps | ✅ | `focus.rs`, 744 LOC |
| Live regions with privacy policy | ✅ | `live_region.rs` |
| AT-SPI vocabulary | ✅ | 451 LOC |
| AT-SPI D-Bus server | ✅ | 804 LOC, **behind a non-default feature** |
| **Reaches a screen reader on the default path** | ❌ | **F-15 — the whole chain is built and disconnected** |
| Verified against Orca / TalkBack / VoiceOver | ❌ | Never tested against a real AT |
| Per-day `date_picker` semantics | ❌ | One node for the whole grid |

### 9.5 Tooling and developer experience

| Item | Status | Note |
|---|:--:|---|
| Profiler | ✅ | Zero-cost when off, Chrome trace export |
| **Inspector application (`aura_lab`)** | ❌ | 11 overlay modules, **no app** — `docs/tools/aura-lab.md` says so |
| Hot reload | 🟡 | `hot` feature, `Cargo.toml:56` calls it "very experimental" |
| Project scaffolding (`aura new`) | ❌ | None |
| Published on crates.io | ❌ | `0.15.0-dev`, unpublished |
| Examples | ❌ | **1** (upstream iced ships ~80) |
| Guides | ✅ | 10 guides, ~45,900 words, genuinely good |
| API docs | ✅ | Dense, explanatory doc comments throughout |
| Changelog | 🟡 | Prose feature list, not a changelog |
| Error messages / diagnostics | 🟡 | `runtime/src/diagnostics.rs` exists; no DX pass done |

### 9.6 Quality gates

| Item | Status | Note |
|---|:--:|---|
| Unit/integration tests | ✅ | **1,705 green, 0 failed, 0 ignored** |
| `cargo check` | ✅ | Clean |
| `cargo fmt --check` | ❌ | **68 violations — CI red** (F-20) |
| `cargo clippy -D warnings` | ❌ | **15 errors — CI red** (F-21) |
| Budget enforcement | ❌ | Script fails; **never invoked by CI** (F-22) |
| Golden/visual regression | ❌ | Harness ✅, **zero cases** (F-10) |
| Fuzzing | 🟡 | 4 targets (`damage_region`, `layout_tree`, `pointer_stream`, `text_edit`), workflow exists |
| Perf regression gate | 🟡 | Smoke only; explicitly not a gate yet, by design |
| Licence/dependency audit | ✅ | `deny.toml`, `licenses.yml` |
| Security threat model | 🟡 | `docs/security/threat-model.md` is 15 lines |
| Release matrix | 🟡 | `docs/testing/release-matrix.md` is 24 lines |

### 9.7 Distribution and support

| Item | Status |
|---|:--:|
| Published package | ❌ |
| Semantic version commitment / API stability policy | ❌ |
| Migration guide from iced | ❌ |
| Community channel | ❌ |
| Commercial support story | ❌ |
| Security disclosure policy | ❌ |
| `CREDITS.md` (referenced by README) | ❌ **file does not exist** |

### 9.8 Completeness scorecard

| Domain | Complete |
|---|---|
| Engine | ~65% |
| Components | ~55% |
| Platform | ~25% |
| Accessibility | ~70% built, **~10% reachable** |
| Tooling / DX | ~30% |
| Quality gates | ~50% |
| Distribution | ~5% |
| **Weighted overall** | **~40% of a framework; ~20% of a *competitive* one** |

---

## 10. Execution plan

Sequenced by dependency, not by preference. Each milestone has an exit gate that is a command
someone else can run.

### M0 — Restore truth (1 week, blocks everything)

| # | Task | Finding |
|---|---|---|
| 1 | `cargo fmt --all`; add a pre-commit hook | F-20 |
| 2 | Fix 3 `manual_let_else`; wire or delete the dead image decoder and pipeline-cache surface | F-21, F-11, F-12 |
| 3 | Add a `budgets` job to `ci.yml`; reconcile the 244-dep and 20.54 MiB overruns deliberately | F-22, F-23 |
| 4 | Re-capture all 13 baselines: clean tree, one machine, truthfully registered, `--features profiling` on | F-02, F-03, F-04 |
| 5 | Rewrite README perf numbers from those baselines; publish worst-case alongside best; fix the crate LOC table | F-01 |
| 6 | Delete or empty `docs/performance/aura-1.0-report.md`; fix units in `methodology.md` | F-05, F-06 |
| 7 | Teach `aura_bench --save` to refuse a `-dirty` tree | F-03 |
| 8 | Write the missing `CREDITS.md` | §9.7 |

**Exit gate:** `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings &&
cargo test --workspace && bash scripts/ci/check-budgets.sh` all pass, and every number in
`README.md` traces to a committed file.

### M1 — Make it look finished (3 weeks, parallel with M2)

| # | Task | Finding |
|---|---|---|
| 1 | Adopt an MIT/Apache icon set; commit SVGs with licence records; generate a typed `icon::` module | F-14 |
| 2 | Bundle one variable font; make it the default | F-14 |
| 3 | Implement the 17 missing `Catalog for Theme` impls | F-13 |
| 4 | Add a compile-time test: a `Catalog` without a `Theme` impl is a build failure | F-13 |
| 5 | Add per-day `date_picker` semantics | F-16 |
| 6 | Add an accessibility panel to `aura-showcase`: reduced motion, text scale, high contrast, live toggle | §8.2 |

**Exit gate:** `cargo run -p aura-showcase` renders every widget with real icons and a bundled
font, in both themes, with the a11y panel switching live.

### M2 — Visual truth (3 weeks, parallel with M1)

| # | Task | Finding |
|---|---|---|
| 1 | Pin a software adapter (`llvmpipe`) for deterministic, hardware-independent capture | F-10 |
| 2 | Write render-and-compare cases for every widget × theme × state | F-10 |
| 3 | Commit the `.rgba` goldens; wire `golden.yml` to block merges | F-10 |
| 4 | Add damage-correctness goldens: assert the *undamaged* region is byte-identical | F-08, F-10 |

**Exit gate:** an intentional 1-px change to any widget fails CI with an uploaded diff.

### M3 — Complete the renderer (6 weeks)

| # | Task | Finding |
|---|---|---|
| 1 | Author the effect WGSL: separable Gaussian, backdrop capture/downsample/blur/upsample, 5×4 colour matrix, noise, mask | F-07 |
| 2 | Wire `effect_pipeline.prepare`/`render` into the engine's pass; retire the stub | F-07 |
| 3 | Real damage present via `wl_surface.damage_buffer` on the VYOM backend | F-08 |
| 4 | Push retention to subtree boundaries; replace the single-slot `Option` with `RetainedLayerCache` | F-09 |
| 5 | Wire the async image decoder into the image cache | F-11 |
| 6 | Re-baseline; `blur_layers` and `effect_ladder` should get **slower** — that is the proof they now render | F-07 |

**Exit gate:** an effects golden shows a real blur; a damage golden proves untouched pixels were
not re-presented; `effect_ladder`'s cost rises and is re-budgeted.

### M4 — Make accessibility real (2 weeks)

| # | Task | Finding |
|---|---|---|
| 1 | Add an `accessibility` feature to the `aura` facade enabling `aura-accessibility/atspi` | F-15 |
| 2 | Spawn the D-Bus bus task from the winit event loop; drain the queue | F-15 |
| 3 | **Verify against a live Orca session** — record the transcript | F-15 |
| 4 | Add an AT-SPI integration test against a real session bus in CI | F-15 |

**Exit gate:** a video of Orca reading `aura-showcase` end to end.

### M5 — Own the platform (8 weeks)

| # | Task | Finding |
|---|---|---|
| 1 | Make `vyom` the default backend on VYOM targets; route `application()` through it | F-18 |
| 2 | Port the winit backend's window/input/IME/clipboard coverage onto it | F-18 |
| 3 | Wire frame-clock pacing, presentation feedback, and power policy to real compositor signals | F-18 |
| 4 | Decide the raster/UI thread split *before* effects make it urgent | §5.2 |
| 5 | Add a framebuffer/DRM backend for `tiny_skia` — the no-GPU story neither competitor can tell | §8.1 |

**Exit gate:** `mobile_shell` runs on a VYOM device through the native backend, with real
partial present and real power measurement.

### M6 — Competitive proof (6 weeks, requires M0–M5)

Execute §11 in full. **No public comparison before this milestone completes.**

### M7 — Adoption (ongoing, parallel from M1)

Publish to crates.io · `aura new` scaffolding · build `aura_lab` into an actual application ·
grow examples from 1 to 20 · migration guide from iced · API stability policy · security
disclosure policy.

### Critical path

```
M0 (1w) ─┬─> M1 (3w) ─┐
         ├─> M2 (3w) ─┼─> M3 (6w) ──> M5 (8w) ──> M6 (6w)
         └─> M4 (2w) ─┘
                                              ≈ 24 weeks to a defensible claim
```

M7 runs alongside from M1 onward and is what determines whether anyone outside this repository
ever uses the result.

---

## 11. Benchmark protocol for a defensible comparison

`benchmarks/competitors/README.md` already states the right rules. This is what executing them
requires. **Until every line below is satisfied, no comparative claim should be made in public,
in a README, in a deck, or in a commit message.**

### 11.1 The application

Build **the same application three times** — Aura, Flutter, React Native (New Architecture,
Hermes, Fabric enabled). Not three benchmark harnesses: one real application, with the same
screens, the same asset set, the same fonts, the same interaction script.

Minimum: a list screen (10k rows, images, mixed heights), a detail screen with a shared-element
transition, a form with text input and IME, a settings screen with toggles and pickers, and a
modal with a backdrop blur.

### 11.2 The device

One physical device. Registered in `benchmarks/devices.toml` **truthfully** (see F-02).
CPU governor pinned to `performance`. Thermals settled between runs. Screen brightness fixed.
Airplane mode. Nothing else running. Record kernel, driver, and compositor versions.

### 11.3 What to measure

| Metric | Method | Why it matters |
|---|---|---|
| **p50 / p95 / p99 frame time** | Input→pixels, not update+draw. ≥1,800 frames, ≥5 runs, median-of-runs | The headline |
| **p99 − p50 spread** | Same samples | **Aura's real story — the GC-free tail** (§8.3) |
| **Missed frame rate** | Against the device's actual refresh | A dropped frame is visible; a slow mean is not |
| **Cold start to first frame** | Process killed between runs, 20 runs | Where Hermes/Dart VM startup shows |
| **Steady-state and peak RSS** | Sampled over the interaction script | Where no-GC should win — currently unproven (§4.4) |
| **Energy over 60 s idle** | Hardware power meter, not software estimate | **The zero-idle-work claim** — Aura's best structural advantage |
| **Energy over 60 s of the scripted interaction** | Same meter | The claim that matters for a device OS |
| **Install size and RSS-at-launch** | Stripped release artifact | F-23; currently 2× the claimed number |

### 11.4 Anti-cheating rules

1. **Publish the worst scene, not just the best.** F-01 is precisely this failure.
2. **Publish the competitors' configuration** — Flutter release+Impeller, RN release+Hermes+Fabric. A comparison against a debug build is fraud.
3. **Never fold harness cost into a framework number.** The repository already learned this — `README.md:38-46` explains why `present` is excluded. Hold competitors to the same rule.
4. **Publish raw samples**, not summaries alone.
5. **Publish the source of all three applications.**
6. **Have someone outside the team reproduce it before publishing.**
7. **Report every metric where Aura loses.** A comparison that reports only wins is read as one that hides losses — and Aura will lose on binary size and possibly on memory. Say so.

---

## 12. Go / no-go for public claims

Each claim, and the single condition that makes it sayable.

| Claim | Say it when |
|---|---|
| "Faster than Flutter / React Native" | §11 executed on one device with all three apps published. **Not before.** |
| "Damage-driven repaint" | F-08 fixed — real partial present on at least one backend. Today this is **false on the GPU path**. |
| "GPU-accelerated visual effects" | F-07 fixed. Today the pipeline is an empty stub. |
| "Accessibility by construction" | F-15 fixed **and** verified against a live Orca session. Today nothing reaches an AT. |
| "Mobile framework" / "mobile components" | A mobile backend exists (F-17). Today: desktop Linux only. |
| "No GC, therefore less memory" | Memory measured against a competitor (§11). Today unproven, and 283.8 MiB idle RSS sits unexplained in the repo. |
| "Zero idle work saves power" | Joules measured (§11). Architecturally true; **empirically unmeasured**. |
| "1,600+ tests" | ✅ **Sayable today** — it is 1,705, all green. Understated. |
| "15 `unsafe` blocks, 9 audited files, `deny` by default" | ✅ **Sayable today** — verified, and under-marketed. |
| "Zero frames produced when idle" | ✅ **Sayable today** — architecturally true and visible in the scheduler. |
| "Software renderer fallback with no GPU" | ✅ **Sayable today** — `tiny_skia` is real, and it is the one thing neither competitor can do. |
| "Deterministic, GPU-free, device-farm-free testing" | ✅ **Sayable today** — 1,705 headless tests prove it. |
| "10.3 MiB binary" | ❌ **Never** — it is 20.54 MiB unstripped, 15.77 MiB stripped. Correct the README. |

**Four claims are sayable today and are currently buried under claims that are not.** That is
the fastest available credibility win in this repository: delete the unsupportable claims and
lead with the supportable ones.

---

## 13. What not to do

1. **Do not chase Flutter's component count.** Several hundred widgets over ten years is not
   catchable and is not the axis anyone chooses a systems framework on.
2. **Do not start Android/iOS backends now.** Two more platforms with an incomplete renderer,
   no icons, and no visual tests means three broken platforms instead of one strong one.
3. **Do not `#[allow(dead_code)]` the clippy failures.** Those warnings are the only signal that
   the image decoder and pipeline cache are disconnected (F-11, F-12). Suppressing them deletes
   the finding, not the problem.
4. **Do not publish any comparative number before §11.** One unreproducible claim costs more
   credibility than ten real advantages earn.
5. **Do not micro-optimise the frame path.** 493× of headroom on the worst scene. The bottleneck
   is completeness, not speed.
6. **Do not write more specification documents.** `phase.md` is 1,010 lines and `docs/` is
   45,900 words. The gap between design and implementation (§5.11: arch 4.1, impl 2.5) is not a
   documentation gap.
7. **Do not delete the honest engineering.** The harness that refuses to auto-adopt goldens, the
   report field that admits allocation counting was off, the methodology note that records an
   unreliable measurement rather than laundering it — that instinct is the most valuable thing
   in this repository. Every finding in §6.1 exists because someone else's honesty made it
   findable. Keep it, and make the README match it.

---

## Appendix A — Raw measurements

### A.1 Commands run, 2026-08-23, at `bf065be7`

```
cargo check --workspace --all-targets       → exit 0, 19.61s, 11 warnings (aura_wgpu dead code)
cargo test --workspace                      → exit 0, 1705 passed / 0 failed / 3 ignored, 63 binaries
cargo build -p mobile_shell --release       → exit 0, 44.86s
cargo fmt --all -- --check                  → FAIL, 68 diff sites
cargo clippy --workspace --all-targets -D warnings → FAIL, 15 errors (aura_wgpu)
bash scripts/ci/check-budgets.sh            → FAIL 2/4
bash scripts/ci/check-workspace.sh          → PASS, 23 members
```

### A.2 Binary size

```
target/release/mobile_shell   21,536,248 bytes  (20.54 MiB)  unstripped
after `strip`                 16,533,088 bytes  (15.77 MiB)
README.md:30 claims                             (10.3  MiB)  unstripped
check-budgets.sh:33 budget    16,777,216 bytes  (16.00 MiB)
```

### A.3 Budget check output

```
Dependency budget
FAIL: dependency count 244 exceeds budget 190
Binary size budget
FAIL: mobile_shell is 21536248 bytes, over the 16777216 byte budget
Unsafe budget
  ok: 9 files opt out of unsafe_code, all on the allow list
Benchmark budget
  ok: frame-time metric guards hold
```

### A.4 Structural counts

| Thing | Count |
|---|---|
| Lines of Rust | 154,107 |
| Workspace members | 23 |
| `#[test]` attributes | 1,566 |
| Tests actually run and passed | **1,705** |
| Tests ignored | 3 (none via `#[ignore]`) |
| Public constructors in `widget/src/helpers.rs` | 75 (61 widgets + 14 helpers) |
| `pub trait Catalog` declarations | 41 |
| `Catalog for Theme` impls in `widget/src/aura.rs` | **24** |
| Unthemed catalogs | **17** |
| `aura-design` component specs | 18 |
| WGSL shaders | 11 (0 for effects) |
| Golden reference images | **0** |
| `compare()` calls outside the golden harness | **0** |
| SVG icons in `assets/` | **0** |
| Bundled fonts | **0** |
| Examples | **1** |
| Docs (`.md` under `docs/`) | 36 (~45,900 words) |
| `unsafe` blocks | 15 across 9 allow-listed files |
| `todo!()` / `unimplemented!()` in production code | **0** |
| Dependencies (normal graph, per budget script) | 244 (budget 190) |
| Android / iOS source references | **1** (a `cfg!` for styling) |

### A.5 Baseline provenance (all 13 files, identical)

```
environment.git_commit                = 30b5e969f3db0123721597dbbc894aca63a69af9-dirty   (HEAD is bf065be7)
environment.device                    = dev-workstation-01                                (registered as Linux)
environment.os                        = macos                                             (contradicts the registry)
environment.renderer_backend          = wgpu
environment.build_profile             = release
environment.allocation_counting_active = false                                            (all allocation fields read 0)
refresh_rate                          = hz60
seed                                  = 42
runs                                  = 5
sample_count                          = 180 frames per run
```

---

## Appendix B — Reproducing this audit

```bash
# Correctness
cargo check --workspace --all-targets
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

# Budgets
bash scripts/ci/check-workspace.sh
bash scripts/ci/check-budgets.sh

# Binary size
cargo build -p mobile_shell --release
ls -l target/release/mobile_shell

# Component coverage — should print 17 unthemed catalogs
comm -23 \
  <(grep -rln 'pub trait Catalog' widget/src/*.rs | xargs -n1 basename | sed 's/.rs//' | sort) \
  <(grep -o '^impl \(crate::\)\?[a-z_]*::Catalog for Theme' widget/src/aura.rs \
      | sed 's/impl \(crate::\)\?//;s/::Catalog for Theme//' | sort)

# Visual truth — should print 0: every compare() call lives in the harness's own self-tests
grep -rn 'compare(' tests/golden/*.rs | grep -v 'harness.rs' | wc -l
ls tests/golden/images 2>/dev/null || echo "no goldens committed"

# Effect shaders — should print only quad/solid.wgsl, which is the inherited
# drop-shadow blur, not an effect pipeline. No effect has a shader.
grep -rl 'blur' --include='*.wgsl' wgpu/src/shader/
sed -n '1,24p' wgpu/src/effects/pipeline.rs

# GPU damage — read the fallback comment
sed -n '396,436p' wgpu/src/window/compositor.rs

# Accessibility reachability — the feature is off by default
sed -n '16,22p' aura-accessibility/Cargo.toml
grep -rn 'atspi' --include='*.toml' . | grep -v Cargo.lock

# Mobile platform — should return one styling cfg!
grep -rn 'android\|target_os = "ios"\|ndk\|UIKit' --include='*.rs' . | grep -v ./target

# Baselines
python3 -c "
import json,glob
for f in sorted(glob.glob('benchmarks/baseline/*.json')):
    d=json.load(open(f)); m=d['median']['frame_time']
    ns=lambda x: x['secs']*1e9+x['nanos']
    print(f\"{f.split('/')[-1]:<28} p95={ns(m['p95'])/1000:7.2f}us  \"
          f\"os={d['environment']['os']:<6} commit={d['environment']['git_commit'][:12]}\")
"
```

---

## Closing

The engineering in this repository is better than its reputation will be if the claims are not
fixed first. A framework with 1,705 green tests, 15 `unsafe` blocks, a gesture arena at parity
with Flutter's, an accessibility design better than either competitor's, and a software renderer
neither of them has, is a serious piece of work.

It is also a framework whose GPU effect pipeline is an empty struct, whose damage tracking is
discarded before it reaches the screen, whose accessibility never reaches a screen reader, which
has no icons, no visual regression tests, and no mobile platform — while its README claims frame
times its own committed baselines do not support and a binary size half of what it actually
builds.

Both of those paragraphs are true. **The second one is fixable in a week for the claims and six
months for the substance.** The order matters: fix the claims first, because every genuine
advantage in the first paragraph depends on being believed.

Aura will not beat Flutter and React Native at being Flutter and React Native. On VYOM OS and
constrained Linux devices — no GC, no idle work, no GPU required, no second runtime to ship —
it can beat both, decisively, and neither can follow without becoming something else.

Choose that fight. Then measure it honestly enough that the win is undeniable.

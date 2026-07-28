# VYOM Aura — Production Execution Roadmap

> Status: implementation contract for the Aura-derived Aura fork  
> Goal: a Rust-first, Wayland-native, Vulkan-first UI framework for VYOM OS with measurable leadership in latency, frame consistency, efficiency, visual quality, accessibility, and developer experience.

## 1. Non-negotiable execution rules

1. **Measure first.** “Fastest” is not a claim until the same public scene, hardware, build flags, warm-up, and sampling method reproduce the result.
2. **Correctness before optimization.** No performance change merges without functional, visual, memory, and accessibility checks.
3. **No permanent idle rendering.** With no animation, input, damage, or external surface update, Aura must submit no GPU frame.
4. **Bound every cache and queue.** Glyphs, images, layers, display lists, effects, pipelines, events, and telemetry must all have budgets and eviction/backpressure behavior.
5. **One architectural risk per commit.** Each central-engine change must be independently benchmarked and revertible.
6. **Keep fallback paths until replacement parity.** Keep `winit` during the VYOM backend migration and keep `tiny-skia` behind a recovery/debug feature until GPU-loss recovery is proven.
7. **Do not mass-rename the Aura internals yet.** Stabilize behavior and crate boundaries first; cosmetic renaming is not a performance phase.
8. **No unsafe code without an RFC.** Any new `unsafe` block requires a safety invariant, Miri/sanitizer coverage where applicable, and review.
9. **No LOC padding.** The ceilings below are maximum review limits, not targets. Prefer deletion, reuse, and small APIs.
10. **A phase is complete only when its exit gate passes.** “Code compiles” is never the sole completion condition.

## 2. Repository and path assumptions

This roadmap uses the paths in the supplied Aura/Aura fork plan. Phase 0 must verify them against the actual checkout before feature work. If the fork uses a semantically equivalent renamed file, record it in `docs/architecture/path-map.md`; do not create duplicate modules just to match this document.

Expected top-level modules:

```text
core/       Fundamental types, widget contracts, layout, text, input
runtime/    Widget-tree reconciliation, event dispatch, layout/draw orchestration
graphics/   Renderer-neutral primitives, damage, layers, caches, compositor contracts
wgpu/       GPU renderer and window compositor
widget/     Standard widgets
tools/      Benchmarks and developer tools
tests/      Cross-crate integration, golden, accessibility, gesture and text tests
```

Nested integration suites use a top-level Cargo harness: for example, `tests/layout.rs` registers modules under `tests/layout/`. The phase that adds the first file in a category must create its matching harness; later phases extend it. Harness LOC is included in that category's file-row estimate.

### LOC accounting

- **Estimated +LOC**: expected net new non-blank source/test/config/documentation lines after refactoring.
- **Maximum +LOC**: hard phase ceiling across the listed files. Generated code, golden PNG bytes, lockfiles, vendored code, and shader compiler output are excluded.
- Count with one pinned tool/config in `scripts/loc.sh`; CI uses the same command.
- A ceiling increase requires `docs/rfcs/RFC-NNN-*.md` explaining why reuse or decomposition cannot keep the phase within budget.
- Deleted LOC is reported separately; it does not permit unrelated new features inside the same phase.

## 3. Global quality budgets

Phase 1 records the real baseline and locks exact target devices in `benchmarks/devices.toml`. Until then, these are required engineering targets, not public guarantees.

| Metric | 60 Hz gate | 90 Hz gate | 120 Hz gate |
|---|---:|---:|---:|
| P95 total frame time | <= 13.0 ms | <= 8.5 ms | <= 6.5 ms |
| P99 total frame time | <= 16.67 ms | <= 11.11 ms | <= 8.33 ms |
| P95 UI-thread work for a typical frame | <= 2.0 ms | <= 1.5 ms | <= 1.0 ms |
| P95 render preparation | <= 1.5 ms | <= 1.0 ms | <= 0.75 ms |
| Missed-frame rate in a 10-minute scripted run | < 0.1% | < 0.1% | < 0.1% |

Additional gates:

- Cold start to first visible simple shell frame: baseline first, then >= 30% improvement; final reference-phone target <= 350 ms after process start, excluding OS boot.
- Idle scene after two seconds: 0 GPU submissions, 0 layout passes, 0 paint passes, <= 1 framework wake-up/second, <= 0.5% of one CPU core.
- Unchanged retained subtree: 0 widget layouts, 0 primitive rebuilds, 0 GPU uploads.
- `VirtualList` with 100,000 logical rows: work proportional to visible rows plus configured cache extent, not total row count.
- Stable memory: after 100 navigation loops and a purge opportunity, RSS and GPU allocations return to within 5% or an explained allocator high-water mark.
- No phase may regress P95/P99 by > 3%, allocations/frame by > 5%, or steady-state memory by > 5% without an approved trade-off report.
- Every touch target in the default design system is at least 48 x 48 logical pixels unless the accessibility policy explicitly permits an exception.

## 4. Phase overview

Phases are dependency ordered. Parallel work is allowed only when file ownership does not overlap and the prerequisite gate has passed.

| Phase | Outcome | Depends on | Est. +LOC | Max +LOC |
|---:|---|---|---:|---:|
| 0 | Repository truth, repaired workspace, CI | — | 1,120 | 1,840 |
| 1 | Reproducible benchmarks and telemetry | 0 | 3,080 | 4,550 |
| 2 | Stable core contracts and diagnostics | 1 | 3,090 | 4,540 |
| 3 | Incremental widget tree and invalidation | 2 | 5,550 | 7,850 |
| 4 | Damage tracking and retained layers | 3 | 4,270 | 6,100 |
| 5 | GPU renderer hot-path optimization | 4 | 7,870 | 11,160 |
| 6 | Frame scheduling and presentation pacing | 5 | 3,400 | 4,840 |
| 7 | Incremental layout and virtualization | 3, 6 | 6,120 | 8,670 |
| 8 | Unified pointer input and gesture arena | 3, 7 | 6,890 | 9,810 |
| 9 | Motion engine and transitions | 6, 8 | 5,320 | 7,560 |
| 10 | Typography, editing, and IME | 3, 5 | 8,020 | 11,240 |
| 11 | Aura visual language and components | 7, 9, 10 | 11,980 | 16,760 |
| 12 | GPU effects and adaptive quality | 5, 6, 11 | 4,790 | 6,840 |
| 13 | Accessibility and semantics | 3, 10, 11 | 5,550 | 7,810 |
| 14 | Native VYOM platform backend | 6, 8, 10, 13 | 8,300 | 11,610 |
| 15 | Profiler, inspector, and Aura Lab | 1–14 | 5,750 | 8,180 |
| 16 | Golden, stress, and regression gates | 11–15 | 5,520 | 7,810 |
| 17 | Power, recovery, security, and hardening | 12, 14, 16 | 5,000 | 7,170 |
| 18 | Reference shell, comparative proof, Aura 1.0 | all | 5,820 | 8,120 |
| **Total** | **Complete production program** | | **107,440** | **152,460** |

The total includes production code, tests, benchmarks, developer tools, CI, and architecture documentation. Expected runtime/library code is approximately 60–70k of the estimate; the remainder is verification and tooling required for production quality.

---

## Phase 0 — Establish repository truth and repair the fork

**Objective:** make the current deletion state explicit, restore a trustworthy build, and prevent feature work on a broken workspace.

### File plan

| File | Action | Exact work | Est. +LOC | Max +LOC |
|---|---|---|---:|---:|
| `Cargo.toml` | Modify | Remove deleted members/dependencies/features; add conservative `default`, `recovery`, `profiling`, and platform features; pin workspace lints | 120 | 220 |
| `Cargo.lock` | Regenerate | Regenerate only through Cargo; no manual edits | 0 | 0 |
| `rust-toolchain.toml` | Create/modify | Pin toolchain and required components | 20 | 40 |
| `deny.toml` | Create | License, source, duplicate, and advisory policy | 80 | 140 |
| `.github/workflows/ci.yml` | Create | fmt, metadata, check, clippy, test, release build, minimal feature matrix | 150 | 240 |
| `.github/workflows/perf.yml` | Create | benchmark smoke job and artifact/report retention; no noisy PR comparisons yet | 110 | 180 |
| `.github/workflows/licenses.yml` | Create | cargo-deny and third-party notice verification | 70 | 120 |
| `scripts/ci/check-workspace.sh` | Create | detect missing members, stale path dependencies, forbidden default features | 100 | 150 |
| `scripts/loc.sh` | Create | reproducible LOC categories and exclusions | 80 | 120 |
| `tests/smoke.rs` | Create | top-level harness for `tests/smoke/` | 20 | 40 |
| `tests/smoke/workspace.rs` | Create | compile-time public API and feature-combination smoke tests | 170 | 250 |
| `docs/architecture/path-map.md` | Create | actual fork path mapping and deleted-crate decisions | 100 | 170 |
| `docs/architecture/feature-policy.md` | Create | default/minimal/recovery/profiling feature rules | 100 | 170 |

### Work sequence

1. Run `cargo metadata --format-version 1` and inventory all workspace members and path dependencies.
2. Use `rg` to find references to removed crates: `beacon`, `devtools`, `highlighter`, `selector`, `test`, `tester`, and deleted examples.
3. Remove only stale references. Restore a deleted capability only when a later phase explicitly needs an Aura-owned replacement.
4. Keep `wgpu`, Wayland, advanced shaping, hinting, and the thread pool in the VYOM profile. Keep X11, web, QR, markdown, syntax highlighting, and software rendering out of default features.
5. Verify default, minimal-headless, profiling, and recovery feature combinations.

### Required verification

```bash
cargo metadata --format-version 1
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
cargo deny check
```

**Exit gate:** clean commands above; no path reference to deleted crates; CI passes twice from a fresh checkout; path map matches the actual repository.

**Phase budget:** estimated 1,120 +LOC; hard maximum 1,840 +LOC.

---

## Phase 1 — Freeze a reproducible baseline

**Objective:** create a benchmark laboratory before changing engine behavior.

### File plan

| File | Action | Exact work | Est. +LOC | Max +LOC |
|---|---|---|---:|---:|
| `tools/aura_bench/Cargo.toml` | Create | isolated benchmark app and optional profiling features | 80 | 130 |
| `tools/aura_bench/src/main.rs` | Create | scene selection, warm-up, fixed-duration run, report output | 180 | 260 |
| `tools/aura_bench/src/config.rs` | Create | device, refresh-rate, scene, seed, and cache-state configuration | 140 | 210 |
| `tools/aura_bench/src/scenes/mod.rs` | Create | deterministic scene registry | 90 | 140 |
| `tools/aura_bench/src/scenes/idle_shell.rs` | Create | static shell/no-redraw case | 110 | 170 |
| `tools/aura_bench/src/scenes/ten_thousand_rows.rs` | Create | list churn, scroll, update and jump cases | 170 | 240 |
| `tools/aura_bench/src/scenes/text_heavy.rs` | Create | multilingual paragraphs, edits, emoji, fallback | 160 | 230 |
| `tools/aura_bench/src/scenes/image_grid.rs` | Create | decode, upload, resize, eviction workload | 150 | 220 |
| `tools/aura_bench/src/scenes/blur_layers.rs` | Create | controlled effect radii/areas/layer counts | 130 | 190 |
| `tools/aura_bench/src/scenes/animated_cards.rs` | Create | transforms, opacity, layout animation stress | 140 | 210 |
| `tools/aura_bench/src/scenes/multitouch.rs` | Create | recorded deterministic pointer streams | 140 | 210 |
| `tools/aura_bench/src/metrics/frame_time.rs` | Create | P50/P95/P99 CPU/GPU phase timings and missed frames | 180 | 260 |
| `tools/aura_bench/src/metrics/allocations.rs` | Create | opt-in counting allocator; disabled outside profiling | 170 | 250 |
| `tools/aura_bench/src/metrics/memory.rs` | Create | RSS and renderer resource snapshots | 130 | 200 |
| `tools/aura_bench/src/metrics/latency.rs` | Create | event-to-submit and present-feedback correlation | 210 | 310 |
| `tools/aura_bench/src/report.rs` | Create | versioned JSON schema plus human-readable summary | 220 | 320 |
| `benchmarks/devices.toml` | Create | named reference hardware, display, OS, driver and power mode | 100 | 160 |
| `benchmarks/baseline/.gitkeep` | Create | report destination; actual machine reports handled by policy | 0 | 0 |
| `scripts/perf/run-baseline.sh` | Create | release flags, governor checks, warm-up, repeated samples | 160 | 230 |
| `tests/bench/report_schema.rs` | Create | stable schema and missing-metric failures | 170 | 250 |
| `docs/performance/methodology.md` | Create | exact reproducibility and statistical rules | 250 | 360 |

### Measurements required

- Cold start to first visible frame.
- CPU frame phases: diff, event, layout, semantics, paint preparation, upload.
- GPU duration and presentation time where supported.
- Allocations/frame, RSS, GPU bytes, draw calls, batch count, uploaded bytes.
- Damage area/full-screen area, visited/laid-out/painted/cached nodes.
- Event timestamp to submit and to compositor presentation feedback.
- Idle wake-ups and submissions.

Use at least 30 seconds of warm-up and five measured runs per scene; store raw samples, median-of-runs summary, commit, compiler, GPU driver, build flags, power mode, and hardware ID. Do not compare results from unlike environments.

**Exit gate:** deterministic scenes run headlessly where possible and on one real VYOM target; two consecutive baseline runs differ by <= 5% on P95; JSON report is schema-tested; current numbers are checked into the agreed benchmark record.

**Phase budget:** estimated 3,080 +LOC; hard maximum 4,550 +LOC.

---

## Phase 2 — Define stable core contracts

**Objective:** introduce revisions, identity, damage, semantics, cache policy, frame context, and diagnostics without yet changing all behavior.

### File plan

| File | Action | Exact work | Est. +LOC | Max +LOC |
|---|---|---|---:|---:|
| `core/src/widget/id.rs` | Modify | compact stable `Id`, keyed-list identity, no string allocation in hot paths | 180 | 270 |
| `core/src/widget/tree.rs` | Modify | `Revisions`, `DirtyFlags`, cached bounds/constraints/transform/clip metadata | 360 | 520 |
| `core/src/widget.rs` | Modify | defaulted `paint_bounds`, `hit_test`, `semantics`, `cache_policy` methods | 170 | 260 |
| `core/src/damage.rs` | Create | renderer-neutral damage request types and invariants | 180 | 260 |
| `core/src/semantics.rs` | Create | platform-neutral semantic node, role, state, action types | 300 | 430 |
| `core/src/cache.rs` | Create | `CachePolicy`, budget class and purge reason | 150 | 230 |
| `core/src/frame.rs` | Create | frame ID, monotonic timing, target present time, scale and quality tier | 190 | 280 |
| `core/src/shell.rs` | Modify | typed layout/paint/semantics/animation/redraw requests; compatibility shim | 300 | 430 |
| `core/src/lib.rs` | Modify | bounded public exports and feature gates | 70 | 120 |
| `runtime/src/diagnostics.rs` | Create | phase counters and zero-cost-disabled instrumentation | 230 | 340 |
| `tests/widgets/identity.rs` | Create | list reorder, insertion, removal and preserved state | 280 | 400 |
| `tests/widgets/dirty_flags.rs` | Create | propagation, clearing and revision wrap policy | 260 | 380 |
| `tests/api/core_contracts.rs` | Create | default-method compatibility and public API checks | 230 | 340 |
| `docs/architecture/invalidation.md` | Create | ownership, propagation, state transitions and invariants | 190 | 280 |

### Design constraints

- Revisions use monotonic wrapping-safe comparison or generation replacement; document the chosen invariant.
- Dirty propagation stops as early as correctness allows.
- The compatibility redraw API maps conservatively to full damage until Phase 3.
- Instrumentation must compile out or be near-zero overhead in non-profiling release builds.
- No mandatory `Widget` method is introduced in this phase.

**Exit gate:** all existing widgets compile unchanged; new identity/dirty tests pass; benchmark scenes remain visually identical; P95 and allocations do not regress beyond global limits.

**Phase budget:** estimated 3,090 +LOC; hard maximum 4,540 +LOC.

---

## Phase 3 — Incremental widget tree and precise invalidation

**Objective:** ensure a local state change visits and updates only the necessary paths/subtrees.

### File plan

| File | Action | Exact work | Est. +LOC | Max +LOC |
|---|---|---|---:|---:|
| `runtime/src/user_interface.rs` | Modify in four commits | split reconcile/event/layout/semantics/paint phases; reuse clean results; report counters | 1,050 | 1,450 |
| `runtime/src/reconcile.rs` | Create | keyed structural diff and stable state retention | 500 | 700 |
| `runtime/src/invalidation.rs` | Create | dirty propagation, bounded damage aggregation, phase scheduling | 460 | 650 |
| `runtime/src/hit_test.rs` | Create | cached path lookup interface; linear fallback retained initially | 350 | 500 |
| `runtime/src/semantics.rs` | Create | dirty semantic subtree update and stable semantic IDs | 300 | 440 |
| `core/src/widget/tree.rs` | Modify | cached layout/paint/hit metadata lifecycle and invariants | 370 | 520 |
| `core/src/shell.rs` | Modify | consume typed requests; coalesce damage and deadlines | 230 | 340 |
| `graphics/src/primitive.rs` | Modify | stable subtree/display-list identity hooks | 160 | 250 |
| `tests/layout/incremental.rs` | Create | constraint/revision matrix and clean-subtree assertions | 440 | 600 |
| `tests/widgets/keyed_reconcile.rs` | Create | reorder/replace/nested identity tests | 410 | 570 |
| `tests/widgets/invalidation.rs` | Create | local paint vs layout vs semantics invalidation | 430 | 600 |
| `tests/events/hit_path.rs` | Create | overlapping, clipped, transformed and captured paths | 360 | 510 |
| `tests/semantics/incremental.rs` | Create | dirty-subtree correctness and stable IDs | 300 | 440 |
| `docs/architecture/widget-lifecycle.md` | Create | state machine and permitted invalidation transitions | 190 | 280 |

### Required commit split

1. Structural reconciliation and stable identity.
2. Layout reuse with unchanged constraints/revision.
3. Hit-path dispatch with correctness fallback.
4. Incremental semantics.
5. Paint invalidation and phase counters.

Each commit gets an independent before/after benchmark record. Never land all five as one diff.

**Exit gate:** changing one leaf in a 10,000-node synthetic tree lays out/paints only the required path/subtree; clean frames do no work; dynamic lists preserve state; no hit-test or semantics mismatch against the linear/full-tree oracle.

**Phase budget:** estimated 5,550 +LOC; hard maximum 7,850 +LOC.

---

## Phase 4 — Damage tracking and retained layers

**Objective:** rebuild and upload only pixels/primitives affected by a change.

### File plan

| File | Action | Exact work | Est. +LOC | Max +LOC |
|---|---|---|---:|---:|
| `graphics/src/damage.rs` | Create/upgrade | `SmallVec` region, clipping, merge heuristic, full-screen threshold, old/new transformed bounds | 460 | 650 |
| `graphics/src/layer.rs` | Create | retained layer key, revision, clip, transform, display list and resource refs | 520 | 720 |
| `graphics/src/display_list.rs` | Create | immutable compact commands and resource generations | 420 | 600 |
| `graphics/src/cache.rs` | Create/upgrade | byte-bounded generational/LRU caches, purge, telemetry | 430 | 620 |
| `graphics/src/compositor.rs` | Modify | frame context, damage region, scale, presentation target, color mode | 250 | 370 |
| `runtime/src/user_interface.rs` | Modify | reuse clean retained layers; damage old and new moving bounds | 360 | 520 |
| `wgpu/src/window/compositor.rs` | Modify | consume damage contract; conservative full present fallback | 250 | 380 |
| `tests/graphics/damage_region.rs` | Create | merge/clip/transform/property tests and threshold cases | 450 | 620 |
| `tests/graphics/retained_layers.rs` | Create | cache hit, revision invalidation, resource generation and eviction | 420 | 590 |
| `tests/golden/damage_motion.rs` | Create | moving/translucent/blurred old-new bounds oracle | 300 | 440 |
| `tools/aura_bench/src/scenes/damage_churn.rs` | Create | tiny, scattered and full-screen damage patterns | 170 | 250 |
| `docs/architecture/retained-rendering.md` | Create | ownership, invalidation, eviction, fallback and correctness model | 240 | 340 |

### Critical invariants

- Damage is clipped to the surface, finite, and normalized.
- Moving content damages both its previous and current device-space bounds.
- Filters expand damage by their sampling radius.
- Too many/scattered rectangles collapse to a full-surface redraw.
- Layer/display-list caches never retain stale GPU generation handles.
- Unsupported backend damage paths remain visually correct through full present.

**Exit gate:** static subtrees produce no primitive rebuild/upload; damage golden tests match full redraw; random property tests find no uncovered changed pixel; memory stays within configured layer/display-list budgets.

**Phase budget:** estimated 4,270 +LOC; hard maximum 6,100 +LOC.

---

## Phase 5 — Optimize the GPU renderer hot path

**Objective:** reduce allocations, pipeline stalls, uploads, state switches, and draw calls while retaining exact output.

### File plan

| File | Action | Exact work | Est. +LOC | Max +LOC |
|---|---|---|---:|---:|
| `wgpu/src/engine.rs` | Modify | lazy pipelines, critical prewarm, resource generations, device-loss rebuild, GPU budgets | 650 | 900 |
| `wgpu/src/pipeline_cache.rs` | Create | keyed pipeline cache and supported persistent backend data | 350 | 500 |
| `wgpu/src/resource_budget.rs` | Create | texture/buffer/atlas byte accounting and pressure signals | 320 | 460 |
| `wgpu/src/layer.rs` | Modify | reusable arenas/vectors, retained batches, conditional sorting, metrics | 650 | 900 |
| `wgpu/src/batch.rs` | Create | material/clip/z-order-compatible batching | 430 | 620 |
| `wgpu/src/quad.rs` | Modify | unified material key and instanced quad path | 520 | 730 |
| `wgpu/src/quad/solid.rs` | Modify | persistent exponential buffers and partial writes | 300 | 440 |
| `wgpu/src/quad/gradient.rs` | Modify | stop dedupe/cache, inline common gradients, limits | 270 | 400 |
| `wgpu/src/quad/sdf.wgsl` | Create/modify | analytic rounded rect, border, focus ring; profiled simple shadow only | 240 | 360 |
| `wgpu/src/text.rs` | Modify | persistent raster/shaping caches, shorter locks, batched uploads | 650 | 900 |
| `graphics/src/text/cache.rs` | Create | stable shaped-run keys and bounded cache | 350 | 500 |
| `wgpu/src/image/atlas.rs` | Create | icon/thumbnail atlas, generations and eviction | 460 | 650 |
| `wgpu/src/image/decode.rs` | Create | bounded background decode, requested-size decode, cancellation | 380 | 540 |
| `wgpu/src/image/cache.rs` | Create | bounded texture lifecycle and memory pressure | 340 | 490 |
| `wgpu/src/image/mod.rs` | Modify | integrate atlas/decode/cache and mipmap policy | 230 | 340 |
| `tests/gpu/batching.rs` | Create | ordering/blend/clip correctness and draw-call expectations | 420 | 580 |
| `tests/gpu/device_loss.rs` | Create | stale generation rejection and rebuild | 340 | 480 |
| `tests/gpu/cache_pressure.rs` | Create | deterministic eviction and no use-after-evict | 330 | 470 |
| `tests/golden/gpu_primitives.rs` | Create | exact/tolerance images across scales and color spaces | 420 | 580 |
| `docs/performance/gpu-pipeline.md` | Create | hot path, cache budgets, metrics and device-loss flow | 220 | 320 |

### Optimization order

1. Instrument current primitive, batch, upload, atlas miss, pipeline switch and draw counts.
2. Remove avoidable allocation/cloning before adding specialized GPU code.
3. Add batching with an ordering-correct oracle.
4. Add persistent buffers and partial writes only where profiling proves a win.
5. Persist text and image resources with byte budgets.
6. Add SDF primitives only when they reduce frame time/draws on target GPUs.
7. Test device loss and memory pressure before enabling the new path by default.

**Exit gate:** no visual mismatch outside approved tolerance; animated-cards and text-heavy P95 improve >= 20% from Phase 4 or an evidence-backed hardware limit is documented; steady-state allocations/frame improve >= 50%; all GPU caches stay bounded; device loss recovers or triggers the documented safe fallback.

**Phase budget:** estimated 7,870 +LOC; hard maximum 11,160 +LOC.

---

## Phase 6 — Frame scheduling, presentation feedback, and pacing

**Objective:** produce exactly the frames the display can present, at the right time.

### File plan

| File | Action | Exact work | Est. +LOC | Max +LOC |
|---|---|---|---:|---:|
| `runtime/src/frame_scheduler.rs` | Create | state machine for idle/requested/animating/submitted/suspended frames | 520 | 720 |
| `runtime/src/user_interface.rs` | Modify | consume scheduler deadlines and skip clean frames | 220 | 320 |
| `core/src/frame.rs` | Modify | monotonic clock, predicted target, feedback sample types | 190 | 280 |
| `core/src/shell.rs` | Modify | coalesced animation/redraw deadlines | 180 | 270 |
| `wgpu/src/window/compositor.rs` | Modify | FIFO default, feedback, max-latency tuning, occlusion/suspend | 450 | 630 |
| `wgpu/src/window/frame_feedback.rs` | Create | backend-neutral presentation feedback adapter | 320 | 460 |
| `tests/runtime/frame_scheduler.rs` | Create | deterministic fake-clock state-machine tests | 460 | 640 |
| `tests/runtime/idle.rs` | Create | no submit/layout/paint after quiescence | 280 | 400 |
| `tests/runtime/occlusion.rs` | Create | suspend/resume/deadline and missed-feedback cases | 300 | 430 |
| `tools/aura_bench/src/metrics/pacing.rs` | Create | cadence, jitter, queue depth, missed-frame classification | 270 | 390 |
| `docs/architecture/frame-scheduling.md` | Create | clock ownership and backend fallback semantics | 210 | 300 |

### Scheduler rules

- Exactly one monotonic framework clock owns animation time.
- Coalesce deadlines; widgets never create independent render loops.
- Stop producing frames for fully occluded/suspended surfaces.
- Handle missing/late feedback without spinning.
- FIFO is the safe default. Lower-latency modes require evidence on every supported target GPU/compositor.

**Exit gate:** idle gate passes; animation cadence has no periodic beat/jitter defect; queue depth stays within policy; occlusion stops submissions; input-latency improvement does not create frame instability.

**Phase budget:** estimated 3,400 +LOC; hard maximum 4,840 +LOC.

---

## Phase 7 — Incremental layout and large-data virtualization

**Objective:** make common layout allocation-light and make large collections proportional to visible content.

### File plan

| File | Action | Exact work | Est. +LOC | Max +LOC |
|---|---|---|---:|---:|
| `core/src/layout/node.rs` | Modify | cached constraints hash, measure result, positions, baseline, transform, clip, revision | 420 | 590 |
| `core/src/layout/limits.rs` | Modify | compact normalized hashable constraints | 220 | 330 |
| `core/src/layout/flex.rs` | Modify | separate measure/place, stack-small storage, intrinsic cache, baseline | 550 | 760 |
| `core/src/layout/cache.rs` | Create | bounded per-node measure cache and invalidation rules | 300 | 430 |
| `widget/src/row.rs` | Modify | use new flex fast path and baseline | 180 | 270 |
| `widget/src/column.rs` | Modify | use new flex fast path | 160 | 240 |
| `widget/src/grid.rs` | Modify | track/measurement cache and visible range hooks | 300 | 430 |
| `widget/src/stack.rs` | Modify | transformed/positioned child bounds and cache | 220 | 320 |
| `widget/src/scrollable.rs` | Modify | viewport model, nested-scroll hooks and stable offset | 420 | 580 |
| `widget/src/virtual_list.rs` | Create | builder, visible window, cache extent, variable-size anchor, reuse | 700 | 950 |
| `widget/src/sliver.rs` | Create | lazy scroll geometry protocol | 460 | 640 |
| `widget/src/safe_area.rs` | Create | platform insets and display cutout policy | 160 | 240 |
| `widget/src/adaptive.rs` | Create | compact/medium/expanded decisions | 190 | 280 |
| `widget/src/constraint.rs` | Create | min/max/aspect and diagnostic widget | 190 | 280 |
| `tests/layout/property.rs` | Create | finite/non-negative/constraint property tests | 420 | 580 |
| `tests/layout/incremental.rs` | Extend | cache correctness under mutation/scale/text changes | 300 | 440 |
| `tests/widgets/virtual_list.rs` | Create | 100k rows, variable height, jump, insert/remove, focus | 520 | 720 |
| `tools/aura_bench/src/scenes/virtual_list.rs` | Create | fixed/variable-height and churn workloads | 180 | 260 |
| `docs/architecture/layout.md` | Create | measure/place contracts, invalidation and virtualization | 230 | 330 |

### Required results

- Common row/column layouts allocate zero heap objects after warm-up for typical child counts.
- Equivalent constraints hash identically.
- Layout cache never survives content/font/scale/direction changes incorrectly.
- Virtual list does not instantiate, reconcile, layout, or paint off-window content except the bounded cache extent.
- Focus and semantics can traverse virtualized content through explicit bring-into-view behavior.

**Exit gate:** 100,000-row benchmark maintains bounded live widget count and stable memory; common layout P95 improves >= 20%; property/oracle tests pass across random finite constraints and scale factors.

**Phase budget:** estimated 6,120 +LOC; hard maximum 8,670 +LOC.

---

## Phase 8 — Unified pointer input and gesture arena

**Objective:** deliver low-latency, touch-first interaction with correct competition, capture, cancellation, nesting, and handoff.

### File plan

| File | Action | Exact work | Est. +LOC | Max +LOC |
|---|---|---|---:|---:|
| `core/src/pointer.rs` | Create | unified mouse/touch/pen/trackpad events and metadata | 380 | 540 |
| `core/src/touch.rs` | Modify | compatibility mapping and full pointer metadata | 220 | 330 |
| `core/src/event.rs` | Modify | pointer phase, cancel, capture and coalesced-event contract | 230 | 340 |
| `aura-gesture/Cargo.toml` | Create | minimal dependencies and feature policy | 70 | 110 |
| `aura-gesture/src/arena.rs` | Create | competition, resolution, teams, cancellation and capture | 650 | 880 |
| `aura-gesture/src/recognizer.rs` | Create | recognizer contract and state invariants | 280 | 410 |
| `aura-gesture/src/tap.rs` | Create | tap and multi-tap | 210 | 310 |
| `aura-gesture/src/long_press.rs` | Create | deadline/movement policy | 180 | 270 |
| `aura-gesture/src/pan.rs` | Create | axis lock, slop and capture | 300 | 430 |
| `aura-gesture/src/swipe.rs` | Create | direction/distance/velocity rules | 180 | 270 |
| `aura-gesture/src/edge_swipe.rs` | Create | reserved edges and system priority | 220 | 320 |
| `aura-gesture/src/pinch.rs` | Create | scale focal-point tracking | 240 | 350 |
| `aura-gesture/src/rotate.rs` | Create | stable multi-pointer angle handling | 210 | 310 |
| `aura-gesture/src/velocity.rs` | Create | timestamped bounded estimator | 220 | 320 |
| `aura-gesture/src/nested_scroll.rs` | Create | pre/post consumption, child-parent handoff and overscroll | 420 | 580 |
| `runtime/src/hit_test.rs` | Modify | spatial index with transformed/clipped bounds and linear oracle | 430 | 600 |
| `runtime/src/pointer_router.rs` | Create | capture table, arena routing, move coalescing | 480 | 660 |
| `runtime/src/user_interface.rs` | Modify | integrate route without full-tree traversal | 210 | 310 |
| `tests/gestures/arena.rs` | Create | competition/cancellation/capture state matrix | 430 | 590 |
| `tests/gestures/nested_scroll.rs` | Create | handoff, fling and boundary cases | 390 | 540 |
| `tests/gestures/recorded_streams.rs` | Create | deterministic multi-device event replays | 360 | 510 |
| `tests/events/spatial_hit_test.rs` | Create | compare index against exhaustive oracle | 340 | 480 |
| `docs/architecture/input-gestures.md` | Create | protocol, priority, capture and system-edge policy | 240 | 350 |

### Correctness rules

- Every pointer sequence ends in up or cancel for all participants.
- Losing recognizers receive cancellation exactly once.
- Captured pointers keep routing outside original bounds.
- High-frequency moves may be coalesced, but down/up/cancel and semantic boundary events may not be dropped.
- Prediction stays behind an experimental feature until wrong-prediction correction is visually proven.

**Exit gate:** recorded streams are deterministic; spatial index always agrees with exhaustive hit testing; nested scroll has no stuck capture; gesture latency improves without missing critical events; system-edge gestures remain reserved by policy.

**Phase budget:** estimated 6,890 +LOC; hard maximum 9,810 +LOC.

---

## Phase 9 — Motion engine and transition system

**Objective:** provide coherent, interruptible, frame-rate-independent motion driven by the shared frame clock.

### File plan

| File | Action | Exact work | Est. +LOC | Max +LOC |
|---|---|---|---:|---:|
| `aura-motion/Cargo.toml` | Create | minimal no-std-friendly math core where practical | 70 | 110 |
| `aura-motion/src/clock.rs` | Create | scheduler time adapter; no independent timers | 160 | 240 |
| `aura-motion/src/tween.rs` | Create | typed interpolation and easing | 280 | 400 |
| `aura-motion/src/spring.rs` | Create | stable analytic/semi-implicit spring with velocity continuity | 360 | 500 |
| `aura-motion/src/decay.rs` | Create | fling/decay with bounds and handoff | 260 | 380 |
| `aura-motion/src/keyframes.rs` | Create | validated normalized keyframes | 300 | 430 |
| `aura-motion/src/timeline.rs` | Create | sequencing, stagger, pause/reverse/seek | 350 | 490 |
| `aura-motion/src/transition.rs` | Create | interruptible state transition orchestration | 360 | 510 |
| `aura-motion/src/shared_element.rs` | Create | registration, snapshot/layer and lifecycle protocol | 430 | 590 |
| `aura-motion/src/reduced_motion.rs` | Create | policy mapping, duration/effect substitution | 160 | 240 |
| `core/src/animation.rs` | Modify | compatibility adapters into Aura Motion | 180 | 270 |
| `widget/src/transition.rs` | Modify | enter/exit/replace/layout/shared-element state machine | 520 | 720 |
| `runtime/src/frame_scheduler.rs` | Modify | active-motion registration and precise deadlines | 220 | 320 |
| `runtime/src/user_interface.rs` | Modify | layout transition and retained-layer coordination | 230 | 340 |
| `tests/motion/spring.rs` | Create | reference solutions, stability and refresh-rate equivalence | 320 | 450 |
| `tests/motion/interruption.rs` | Create | rapid reverse/replace/gesture takeover | 350 | 480 |
| `tests/golden/transitions.rs` | Create | fixed-time frames at 60/90/120 Hz | 340 | 470 |
| `tools/aura_bench/src/scenes/motion_stress.rs` | Create | 1/10/100/1000 simultaneous animations | 170 | 250 |
| `docs/design/motion-language.md` | Create | durations, springs, hierarchy and reduced-motion substitutions | 260 | 370 |

### Required behavior

- Motion result is based on elapsed monotonic time, not frame count.
- Interruptions preserve position and, where physically meaningful, velocity.
- Gesture-driven animation can hand off to spring/decay without a jump.
- Reduced motion is a semantic alternative, not simply a global zero-duration switch.
- Shared-element transitions never keep stale layers/resources alive after cancel or navigation.

**Exit gate:** fixed-time state matches across 60/90/120 Hz within numerical tolerance; interruption tests show no jumps; 1,000-animation stress scene remains bounded; idle resumes immediately when motion completes.

**Phase budget:** estimated 5,320 +LOC; hard maximum 7,560 +LOC.

---

## Phase 10 — Typography, text editing, and IME

**Objective:** make multilingual text visually correct, fast, editable, and secure for VYOM’s target languages.

### File plan

| File | Action | Exact work | Est. +LOC | Max +LOC |
|---|---|---|---:|---:|
| `core/src/text/paragraph.rs` | Modify | stable cache key, locale/script/direction, fallback, axes, selection geometry | 650 | 900 |
| `core/src/text/editor.rs` | Modify | grapheme/word/line movement, BiDi cursor, undo, composition, secure mode | 950 | 1,280 |
| `core/src/input_method.rs` | Create | preedit, selection, commit/cancel, candidate rect, surrounding text policy | 480 | 680 |
| `core/src/text/selection.rs` | Create | logical/visual ranges and geometry | 320 | 470 |
| `core/src/text/fallback.rs` | Create | deterministic script/emoji fallback chain | 300 | 450 |
| `core/src/text/line_break.rs` | Create | line-break/hyphenation hook and locale policy | 260 | 390 |
| `graphics/src/text/cache.rs` | Extend | shaped run and paragraph cache, byte bounds and invalidation | 430 | 600 |
| `wgpu/src/text.rs` | Modify | mono/color atlas split, batch uploads, prewarm, lock reduction | 540 | 760 |
| `widget/src/text.rs` | Modify | locale, direction, overflow and semantics | 260 | 390 |
| `widget/src/text_input.rs` | Modify | composition, BiDi selection, secure behavior, virtual-keyboard insets | 700 | 960 |
| `tests/text/corpus.rs` | Create | English, Devanagari, Gurmukhi, Arabic, Bengali, Tamil, emoji, mixed BiDi | 650 | 880 |
| `tests/text/editing.rs` | Create | grapheme/BiDi movement, selection, undo and deletion | 620 | 850 |
| `tests/text/ime.rs` | Create | composition lifecycle, candidate geometry and cancellation | 500 | 700 |
| `tests/text/security.rs` | Create | secure-field clipboard/surrounding-text/snapshot policy | 350 | 500 |
| `tests/golden/multilingual_text.rs` | Create | deterministic reference font rendering at scale factors | 440 | 600 |
| `tools/aura_bench/src/scenes/text_edit.rs` | Create | long document, IME, selection and cache churn | 190 | 280 |
| `assets/fonts/test/README.md` | Create | licensed deterministic test-font manifest; no untracked fonts | 80 | 130 |
| `docs/architecture/text-ime.md` | Create | shaping/cache/editor/IME/security contracts | 300 | 420 |

### Mandatory language matrix

English, Hindi/Devanagari, Punjabi/Gurmukhi, Arabic/RTL, Bengali, Tamil, emoji ZWJ sequences, combining marks, mixed LTR/RTL paragraphs, variable fonts, and missing-glyph fallback.

### Security requirements

- Secure text widgets do not expose content through clipboard, semantics value, debug logs, surrounding-text APIs, retained screenshots, or profiler captures.
- Password reveal is explicit, timed/policy-controlled, and visually/accessibly communicated.
- Untrusted text cannot create unbounded shaping/cache allocations.

**Exit gate:** corpus and editing tests pass; cache keys include every shaping-affecting input; no reshaping for unchanged text; secure-field threat tests pass; text-heavy P95 improves or stays within global limits with significantly fewer repeated shapes/uploads.

**Phase budget:** estimated 8,020 +LOC; hard maximum 11,240 +LOC.

---

## Phase 11 — Aura design system and production components

**Objective:** create a coherent visual language whose beauty comes from typography, hierarchy, spacing, color, material, and motion consistency.

### File plan

| File/group | Action | Exact work | Est. +LOC | Max +LOC |
|---|---|---|---:|---:|
| `aura-design/Cargo.toml`, `aura-design/src/lib.rs` | Create | crate boundary, public prelude and feature policy | 160 | 240 |
| `aura-design/src/tokens/color.rs` | Create | semantic roles, light/dark/high-contrast, contrast validation | 380 | 520 |
| `aura-design/src/tokens/typography.rs` | Create | type ramp, language-aware line metrics | 300 | 430 |
| `aura-design/src/tokens/spacing.rs` | Create | compact canonical scale | 120 | 190 |
| `aura-design/src/tokens/radius.rs` | Create | shape scale and component mappings | 110 | 170 |
| `aura-design/src/tokens/elevation.rs` | Create | elevation/material hierarchy independent of blur | 180 | 260 |
| `aura-design/src/tokens/motion.rs` | Create | semantic durations/springs tied to Aura Motion | 170 | 250 |
| `aura-design/src/icon.rs`, `assets/icons/manifest.toml`, `scripts/design/check-icons.sh` | Create | optical-grid icon API, licensed asset manifest, SVG geometry/naming validation | 400 | 600 |
| `aura-design/src/theme.rs` | Create | immutable theme, inheritance, dynamic color and change revision | 430 | 600 |
| `aura-design/src/surface.rs` | Create | material/surface/elevation/interaction layer | 320 | 450 |
| `aura-design/src/state.rs` | Create | default/hover/focus/press/select/disable/load/error state resolver | 260 | 380 |
| `aura-design/src/components/button.rs` | Create | button variants, icon/loading/focus/touch states | 400 | 560 |
| `aura-design/src/components/icon_button.rs` | Create | icon control and tooltip/accessibility policy | 250 | 360 |
| `aura-design/src/components/card.rs` | Create | interactive/static cards and elevation | 280 | 400 |
| `aura-design/src/components/text_field.rs` | Create | text/password variants, validation and IME | 650 | 880 |
| `aura-design/src/components/selection.rs` | Create | switch, checkbox, radio | 570 | 780 |
| `aura-design/src/components/slider.rs` | Create | single/range, keyboard/touch/RTL | 430 | 600 |
| `aura-design/src/components/dialog.rs` | Create | modal semantics, focus trap, adaptive layout | 430 | 600 |
| `aura-design/src/components/bottom_sheet.rs` | Create | drag anchors, nested scroll and dismiss | 500 | 690 |
| `aura-design/src/components/menu.rs` | Create | menu/submenu, keyboard, placement, typeahead | 460 | 640 |
| `aura-design/src/components/toast.rs` | Create | timed/live-region action behavior | 230 | 340 |
| `aura-design/src/components/notification.rs` | Create | grouped content/actions/privacy levels | 320 | 460 |
| `aura-design/src/components/navigation.rs` | Create | nav bar, rail/sidebar and adaptive destinations | 620 | 850 |
| `aura-design/src/components/app_bar.rs` | Create | standard/large/collapsing title | 320 | 450 |
| `aura-design/src/components/list_tile.rs` | Create | one/two/three-line and leading/trailing slots | 280 | 400 |
| `aura-design/src/components/window_chrome.rs` | Create | desktop/tablet chrome and system gestures | 360 | 500 |
| `aura-design/src/components/progress.rs` | Create | linear/circular determinate and indeterminate | 300 | 430 |
| `aura-design/src/components/tooltip.rs` | Create | pointer/focus/long-press activation | 230 | 330 |
| `tests/design/tokens.rs` | Create | invariants, contrast, scale and theme revision | 420 | 570 |
| `tests/design/components.rs` | Create | state/input/semantics matrix | 900 | 1,200 |
| `tests/golden/design_system.rs` | Create | all components/states/themes/scales/languages | 700 | 950 |
| `docs/design/aura-language.md` | Create | visual principles, tokens, usage and anti-patterns | 500 | 680 |

### Component completion contract

Every component defines and tests default, hover, focus-visible, pressed, selected, disabled, loading, error, high-contrast, RTL, large-text, and reduced-motion behavior where applicable. It also declares:

- semantic role/name/value/actions;
- keyboard, pointer, touch and gamepad/focus navigation;
- minimum touch target and focus ring;
- compact/medium/expanded layout behavior;
- theme tokens only—no unexplained hard-coded color/spacing/motion values;
- zero expensive effect requirement for readability.

**Exit gate:** all production components pass the state matrix; contrast and touch targets pass; design golden suite is reviewed at phone/tablet/desktop/watch profiles; visual review finds one coherent language rather than individually styled widgets; performance remains within global gates.

**Phase budget:** estimated 11,980 +LOC; hard maximum 16,760 +LOC.

---

## Phase 12 — GPU visual effects and adaptive quality

**Objective:** provide premium effects with bounded cost and automatic degradation under power, thermal, or GPU pressure.

### File plan

| File | Action | Exact work | Est. +LOC | Max +LOC |
|---|---|---|---:|---:|
| `wgpu/src/effects/mod.rs` | Create | effect graph contract, bounds expansion and quality tier | 220 | 320 |
| `wgpu/src/effects/shadow.rs` | Create | cached analytic/raster strategy chosen by profile | 420 | 580 |
| `wgpu/src/effects/blur.rs` | Create | separable downsampled blur with radius/area limits | 480 | 650 |
| `wgpu/src/effects/backdrop_blur.rs` | Create | bounded capture region and compositor fallback | 380 | 530 |
| `wgpu/src/effects/color_matrix.rs` | Create | fused color transforms | 220 | 330 |
| `wgpu/src/effects/saturation.rs` | Create | specialization or shared matrix path | 130 | 210 |
| `wgpu/src/effects/noise.rs` | Create | deterministic low-cost material noise | 160 | 240 |
| `wgpu/src/effects/mask.rs` | Create | clip/mask composition and atlas integration | 300 | 430 |
| `wgpu/src/effects/pool.rs` | Create | reusable intermediate texture pool with byte budget | 350 | 500 |
| `graphics/src/quality.rs` | Create | Ultra/Balanced/Efficient/Recovery policy | 230 | 330 |
| `runtime/src/quality_controller.rs` | Create | hysteresis-based thermal/power/frame-pressure adaptation | 360 | 510 |
| `aura-design/src/tokens/elevation.rs` | Modify | per-tier effect recipes and no-blur fallback | 130 | 210 |
| `tests/gpu/effect_bounds.rs` | Create | damage expansion, clipping and sampling correctness | 330 | 470 |
| `tests/gpu/effect_budget.rs` | Create | texture-pool/area/radius limits and tier switching | 320 | 450 |
| `tests/golden/effects.rs` | Create | tier-by-tier approved output | 360 | 500 |
| `tools/aura_bench/src/scenes/effect_ladder.rs` | Create | radius/area/layer scaling curve | 180 | 260 |
| `docs/performance/effect-budgets.md` | Create | limits per tier/device and fallback rules | 220 | 320 |

### Effect policy

- Downsample blur passes and reuse bounded intermediate textures.
- Never blur the full surface when a smaller sampling region is correct.
- Never require blur/noise/shadow to distinguish content or controls.
- Use hysteresis and minimum residency time when changing quality tiers; prevent visual oscillation.
- Recovery tier must render the shell without expensive effects.

**Exit gate:** effects respect damage expansion and budgets; tier switching is stable; Efficient and Recovery remain readable and coherent; no effect can trigger unbounded texture allocation; effect ladder quantifies the cost curve on every reference GPU.

**Phase budget:** estimated 4,790 +LOC; hard maximum 6,840 +LOC.

---

## Phase 13 — Accessibility and semantic navigation

**Objective:** make accessibility a retained, incremental core feature rather than a late adapter.

### File plan

| File | Action | Exact work | Est. +LOC | Max +LOC |
|---|---|---|---:|---:|
| `aura-accessibility/Cargo.toml`, `src/lib.rs` | Create | semantic engine crate and adapters | 150 | 230 |
| `aura-accessibility/src/tree.rs` | Create | stable semantic tree, dirty subtree update and diff | 520 | 720 |
| `aura-accessibility/src/node.rs` | Create | role/name/description/value/state/action/bounds/order/live region | 350 | 500 |
| `aura-accessibility/src/focus.rs` | Create | traversal groups, spatial/directional focus and restoration | 500 | 690 |
| `aura-accessibility/src/actions.rs` | Create | action routing to stable widget identity | 260 | 380 |
| `aura-accessibility/src/live_region.rs` | Create | polite/assertive queue, coalescing and privacy | 220 | 330 |
| `aura-accessibility/src/adapter.rs` | Create | platform adapter trait and capability negotiation | 230 | 340 |
| `core/src/semantics.rs` | Modify | complete roles/states/actions and virtualized child protocol | 320 | 450 |
| `runtime/src/semantics.rs` | Modify | incremental diff, bounds updates, action routing | 420 | 580 |
| `runtime/src/user_interface.rs` | Modify | focus/semantics lifecycle coordination | 170 | 260 |
| `widget/src/focus_scope.rs` | Create | trap/group/restore/autofocus behavior | 300 | 430 |
| `tests/accessibility/tree.rs` | Create | full/incremental oracle comparison | 430 | 590 |
| `tests/accessibility/focus.rs` | Create | keyboard/spatial/trap/restore/virtual list | 480 | 650 |
| `tests/accessibility/components.rs` | Create | design component semantic snapshots and actions | 520 | 710 |
| `tests/accessibility/policies.rs` | Create | contrast, large text, reduced motion, touch target | 380 | 530 |
| `docs/accessibility/contract.md` | Create | authoring rules, privacy, platform gaps and test matrix | 300 | 420 |

### Required behavior

- Semantic IDs remain stable across ordinary view reconstruction.
- Dirty visual bounds update semantics without rebuilding clean subtrees.
- Focus order is deterministic in LTR/RTL and with virtualized content.
- Live regions do not leak secure/private text.
- Components expose equivalent action paths across touch, keyboard, screen reader, and switch-style navigation where supported.

**Exit gate:** incremental semantic output matches full rebuild oracle; all Aura components have semantic snapshots/actions; focus never becomes lost or trapped unintentionally; large text and high contrast remain usable on all device profiles.

**Phase budget:** estimated 5,550 +LOC; hard maximum 7,810 +LOC.

---

## Phase 14 — Native VYOM platform backend

**Objective:** connect Aura directly to the Smithay-based VYOM compositor while preserving `winit` as a known-good comparison/fallback until parity.

### File plan

| File | Action | Exact work | Est. +LOC | Max +LOC |
|---|---|---|---:|---:|
| `aura-platform-vyom/Cargo.toml`, `src/lib.rs` | Create | platform crate, negotiated capabilities and safe exports | 180 | 280 |
| `aura-platform-vyom/src/event_loop.rs` | Create | poll integration, tasks, timers, shutdown and no-spin idle | 650 | 900 |
| `aura-platform-vyom/src/surface.rs` | Create | surface lifecycle, configure/ack, scale, damage and roles | 620 | 850 |
| `aura-platform-vyom/src/frame_clock.rs` | Create | presentation feedback to Aura scheduler | 320 | 470 |
| `aura-platform-vyom/src/input.rs` | Create | seats, pointer/touch/keyboard, device lifecycle | 600 | 820 |
| `aura-platform-vyom/src/gesture_bridge.rs` | Create | compositor-reserved edges and gesture handoff | 300 | 450 |
| `aura-platform-vyom/src/ime.rs` | Create | input-method/text-input protocol and virtual keyboard insets | 500 | 700 |
| `aura-platform-vyom/src/clipboard.rs` | Create | typed offers, cancellation, size limits and secure policy | 300 | 450 |
| `aura-platform-vyom/src/drag_drop.rs` | Create | offer lifecycle, actions, bounds and cancellation | 360 | 500 |
| `aura-platform-vyom/src/accessibility.rs` | Create | semantic adapter transport and action return path | 400 | 560 |
| `aura-platform-vyom/src/power.rs` | Create | visibility, suspend, thermal and battery signals | 300 | 440 |
| `aura-platform-vyom/src/window_protocol.rs` | Create | layer/lock/overlay/overview/security roles | 620 | 850 |
| `aura-platform-vyom/src/capabilities.rs` | Create | protocol/version negotiation and fallbacks | 250 | 380 |
| `tests/platform/fake_compositor.rs` | Create | deterministic compositor protocol harness | 800 | 1,050 |
| `tests/platform/lifecycle.rs` | Create | create/configure/scale/suspend/destroy/reconnect | 500 | 680 |
| `tests/platform/input_ime.rs` | Create | multi-seat, cancel, hot-unplug and IME lifecycle | 550 | 750 |
| `tests/platform/security.rs` | Create | lock surface, screenshot restriction, clipboard privacy | 430 | 600 |
| `tools/aura_bench/src/platform_latency.rs` | Create | protocol timestamps and present correlation | 240 | 350 |
| `docs/platform/vyom-protocol.md` | Create | supported versions, ownership, fallbacks and threat boundaries | 380 | 530 |

### Required integrations

- Layer-shell surfaces, secure lock screen, status bar and overlays.
- Presentation feedback, fractional scaling, multi-display and rotation.
- Surface suspend/resume and occlusion notifications.
- Screenshot/privacy restrictions and overview thumbnails.
- Text-input/input-method, clipboard and drag/drop.
- Protocol version/capability negotiation; no assumption that every compositor feature exists.

### Migration rule

`winit` may leave the VYOM production default only after the same reference shell passes both backends and the native backend has feature parity, equal-or-better stability, idle behavior, input/IME/accessibility correctness, and measurable latency benefit. Keep it as a developer feature for at least one release cycle.

**Exit gate:** 24-hour automated lifecycle soak; hot-plug/scale/rotation/suspend tests pass; no protocol error or idle spin; lock-screen security tests pass; native backend meets or beats `winit` P95 input-to-present on reference devices.

**Phase budget:** estimated 8,300 +LOC; hard maximum 11,610 +LOC.

---

## Phase 15 — Aura Lab, profiler, and diagnostics

**Objective:** make performance and correctness observable without attaching a general-purpose debugger.

### File plan

| File | Action | Exact work | Est. +LOC | Max +LOC |
|---|---|---|---:|---:|
| `aura-profiler/Cargo.toml`, `src/lib.rs` | Create | low-overhead event schema, ring buffer and export | 250 | 370 |
| `aura-profiler/src/events.rs` | Create | versioned CPU/GPU/layout/cache/input/semantics events | 300 | 430 |
| `aura-profiler/src/collector.rs` | Create | bounded nonblocking collection and sampling | 420 | 580 |
| `aura-profiler/src/export.rs` | Create | Chrome trace and Aura JSON export with redaction | 300 | 440 |
| `tools/aura_lab/Cargo.toml`, `src/main.rs` | Create | standalone inspector app and connection lifecycle | 350 | 500 |
| `tools/aura_lab/src/widget_inspector.rs` | Create | tree, state, revisions and identity | 430 | 590 |
| `tools/aura_lab/src/layout_overlay.rs` | Create | constraints, bounds, baselines and cache hits | 300 | 430 |
| `tools/aura_lab/src/damage_overlay.rs` | Create | damage rectangles/full-redraw cause | 260 | 380 |
| `tools/aura_lab/src/overdraw_overlay.rs` | Create | overdraw heat map and layer breakdown | 280 | 400 |
| `tools/aura_lab/src/frame_graph.rs` | Create | CPU/GPU/present timeline and missed-frame cause | 450 | 620 |
| `tools/aura_lab/src/gpu_resources.rs` | Create | buffers/textures/atlases/pipelines/budgets/evictions | 360 | 500 |
| `tools/aura_lab/src/semantics_inspector.rs` | Create | semantic tree, focus order and actions | 330 | 470 |
| `tools/aura_lab/src/animation_inspector.rs` | Create | timelines, velocity, spring and reduced motion | 320 | 450 |
| `tools/aura_lab/src/device_profiles.rs` | Create | phone/tablet/desktop/watch viewport and capability simulation | 300 | 430 |
| `runtime/src/diagnostics.rs` | Modify | instrumentation hooks behind profiling feature | 220 | 330 |
| `tests/profiler/overhead.rs` | Create | disabled/enabled overhead and buffer overflow policy | 320 | 460 |
| `tests/profiler/privacy.rs` | Create | secure text/image/title redaction | 260 | 380 |
| `docs/tools/aura-lab.md` | Create | workflows, interpretation and privacy restrictions | 300 | 420 |

### Tooling rules

- Disabled profiler overhead must be statistically indistinguishable or below 1%.
- Enabled default sampling overhead must remain below 3% in the benchmark suite.
- Buffers are bounded and drop telemetry—not billing/system events—under pressure.
- Secure text, private notifications, protected surfaces, clipboard contents, and raw images are redacted by default.
- Hot reload may cover tokens/component declarations; it must not weaken type or resource safety.

**Exit gate:** every global metric can be traced to a phase/cause; overlays agree with benchmark counters; overflow cannot crash or grow memory; privacy tests pass; Aura Lab works on all device profiles.

**Phase budget:** estimated 5,750 +LOC; hard maximum 8,180 +LOC.

---

## Phase 16 — Visual regression, stress, fuzz, and CI gates

**Objective:** convert visual and performance quality into repeatable merge requirements.

### File plan

| File/group | Action | Exact work | Est. +LOC | Max +LOC |
|---|---|---|---:|---:|
| `aura-showcase/Cargo.toml`, `src/main.rs` | Create | deterministic component/state showcase | 300 | 440 |
| `aura-showcase/src/catalog.rs` | Create | all components, variants and state controls | 650 | 880 |
| `aura-showcase/src/profiles.rs` | Create | theme/scale/language/width/motion/contrast matrix | 300 | 430 |
| `tests/golden/harness.rs` | Create | deterministic render/capture/diff/tolerance/report | 650 | 880 |
| `tests/golden/manifest.toml` | Create | explicit approved cases and tolerance classes | 180 | 270 |
| `tests/stress/navigation.rs` | Create | 100/1,000-loop state/resource stability | 380 | 530 |
| `tests/stress/window_lifecycle.rs` | Create | surface/display/suspend churn | 400 | 550 |
| `tests/stress/resource_pressure.rs` | Create | fonts/images/layers/effects eviction | 420 | 580 |
| `fuzz/Cargo.toml` | Create | isolated cargo-fuzz package, pinned features and corpus policy | 80 | 130 |
| `fuzz/fuzz_targets/damage_region.rs` | Create | malformed/extreme rectangles/transforms | 190 | 280 |
| `fuzz/fuzz_targets/pointer_stream.rs` | Create | sequence/capture/cancel invariants | 210 | 300 |
| `fuzz/fuzz_targets/text_edit.rs` | Create | Unicode editing and IME operation streams | 240 | 340 |
| `fuzz/fuzz_targets/layout_tree.rs` | Create | random bounded layout trees and constraints | 240 | 340 |
| `.github/workflows/golden.yml` | Create | controlled renderer/golden comparison and review artifact | 180 | 270 |
| `.github/workflows/perf.yml` | Extend | statistically valid protected-runner regression gate | 250 | 360 |
| `.github/workflows/fuzz.yml` | Create | bounded PR smoke plus scheduled longer runs | 150 | 230 |
| `scripts/ci/check-budgets.sh` | Create | LOC, binary, dependencies, caches and perf thresholds | 220 | 320 |
| `docs/testing/golden-policy.md` | Create | approval/tolerance/font/driver rules | 240 | 340 |
| `docs/testing/release-matrix.md` | Create | device, language, input, platform and recovery coverage | 240 | 340 |

### Golden policy

- Pin fonts, scale, color space, renderer configuration and deterministic time.
- Separate exact primitives from tolerance-based effects/text cases.
- A changed golden requires a rendered diff, design reason, performance report, and approval; never auto-update goldens in CI.
- Cover light/dark/high-contrast, reduced motion, all component states, scale factors, target languages, and compact/medium/expanded profiles.

**Exit gate:** PR CI blocks functional, golden, budget and statistically significant performance regressions; scheduled fuzz/stress jobs are clean; repeated navigation/resource pressure stabilizes within memory gates.

**Phase budget:** estimated 5,520 +LOC; hard maximum 7,810 +LOC.

---

## Phase 17 — Power, recovery, security, and production hardening

**Objective:** keep the shell responsive and safe through pressure, suspension, GPU loss, malformed content, and degraded hardware.

### File plan

| File | Action | Exact work | Est. +LOC | Max +LOC |
|---|---|---|---:|---:|
| `runtime/src/power_policy.rs` | Create | battery/thermal/visibility policy and stable quality transitions | 350 | 500 |
| `runtime/src/memory_pressure.rs` | Create | ordered cache purge and critical recovery | 300 | 430 |
| `runtime/src/recovery.rs` | Create | renderer restart/safe-shell escalation and crash-loop limit | 360 | 510 |
| `wgpu/src/engine.rs` | Modify | robust device-loss, timeout/OOM classification and recreation | 320 | 470 |
| `wgpu/src/resource_budget.rs` | Modify | tier/device budgets and critical reserve | 220 | 330 |
| `aura-platform-vyom/src/power.rs` | Modify | real suspend/thermal/battery/visibility integration | 250 | 370 |
| `aura-platform-vyom/src/window_protocol.rs` | Modify | protected-content and lock-role enforcement | 220 | 330 |
| `core/src/security.rs` | Create | privacy classification propagated to render/tool/platform paths | 300 | 430 |
| `tests/recovery/gpu_loss.rs` | Create | fault injection during text/image/effect/navigation workloads | 420 | 580 |
| `tests/recovery/oom.rs` | Create | deterministic allocation/resource pressure and graceful purge | 360 | 510 |
| `tests/recovery/suspend_resume.rs` | Create | repeated suspend, display removal and stale resources | 360 | 510 |
| `tests/security/protected_content.rs` | Create | screenshots, overview, profiler, semantics and notifications | 420 | 580 |
| `tests/security/malformed_inputs.rs` | Create | extreme images/fonts/text/protocol/event bounds | 350 | 500 |
| `tools/aura_bench/src/scenes/power_idle.rs` | Create | idle/animation/scroll energy sampling protocol | 180 | 270 |
| `docs/security/threat-model.md` | Create | trust boundaries, assets, attacks, mitigations and residual risks | 330 | 470 |
| `docs/operations/recovery.md` | Create | purge/restart/safe-shell/crash reporting runbook | 260 | 380 |

### Recovery order

1. Stop optional prefetch/decode.
2. Evict inactive image/effect/display-list caches.
3. Reduce quality tier with hysteresis.
4. Rebuild lost GPU resources by generation.
5. Restart the renderer while preserving safe application state.
6. Escalate to a minimal Recovery-tier shell if repeated restart fails.

Never continue with potentially stale GPU handles or expose protected pixels through fallback tooling.

**Exit gate:** injected GPU loss and pressure recover without stale-resource use; repeated suspend/resume is stable; Recovery tier can render lock screen and essential settings; threat-model tests pass; idle and thermal policies meet reference-device targets.

**Phase budget:** estimated 5,000 +LOC; hard maximum 7,170 +LOC.

---

## Phase 18 — Reference shell, competitive proof, and Aura 1.0

**Objective:** prove the complete system in a real VYOM flow and publish only reproducible performance claims.

### File plan

| File/group | Action | Exact work | Est. +LOC | Max +LOC |
|---|---|---|---:|---:|
| `examples/vyom_shell/Cargo.toml`, `src/main.rs` | Create | deterministic production-style integration shell | 300 | 440 |
| `examples/vyom_shell/src/boot.rs` | Create | bounded boot animation and first-frame path | 260 | 380 |
| `examples/vyom_shell/src/lock_screen.rs` | Create | secure surface, auth handoff and protected content | 450 | 620 |
| `examples/vyom_shell/src/launcher.rs` | Create | virtualized app grid/search/drag states | 500 | 690 |
| `examples/vyom_shell/src/settings.rs` | Create | navigation, forms, text/IME and accessibility | 600 | 820 |
| `examples/vyom_shell/src/control_centre.rs` | Create | sheet gestures, sliders/toggles and adaptive effects | 420 | 580 |
| `examples/vyom_shell/src/notifications.rs` | Create | live regions, privacy, grouping and dismiss | 380 | 530 |
| `examples/vyom_shell/src/overview.rs` | Create | retained thumbnails, gestures and protected-window handling | 430 | 600 |
| `tests/e2e/shell_flow.rs` | Create | boot to lock to launcher/settings/control/notification/overview/home | 550 | 750 |
| `tests/e2e/recovery_flow.rs` | Create | GPU loss/suspend/pressure inside end-to-end flow | 350 | 490 |
| `benchmarks/competitors/README.md` | Create | public-version, scene-parity and configuration rules | 260 | 370 |
| `scripts/perf/compare-frameworks.sh` | Create | pinned build/run/report orchestration; no selective metrics | 300 | 430 |
| `docs/performance/aura-1.0-report.md` | Create | hardware, raw reports, statistics, wins, losses, limitations | 480 | 650 |
| `docs/release/aura-1.0-checklist.md` | Create | API, platform, design, a11y, perf, security and recovery sign-off | 260 | 370 |
| `CHANGELOG.md`, `README.md` | Modify | accurate capabilities, migration and non-marketing limitations | 280 | 400 |

### End-to-end acceptance flow

```text
Boot animation
→ Lock screen
→ Gesture unlock
→ Launcher
→ Open Settings
→ Edit multilingual text through IME
→ Open Control Centre
→ Receive and dismiss a notification
→ Window overview
→ Return home
→ Suspend and resume
→ Inject GPU loss and recover
```

### Comparative benchmark rules

- Use public, production builds of selected frameworks and disclose versions/commits.
- Match pixels, content count, fonts, images, refresh rate, window size, animation, warm-up, and input trace as closely as possible.
- Run on the same machine, power mode, compositor/session, driver and sampling order; randomize order when thermal drift matters.
- Publish raw samples, methodology, confidence intervals, failures, and metrics where Aura loses.
- Claim only the exact tested domain, for example “lowest P95 frame preparation in these scenes on device X,” never the universal phrase “fastest ever.”

**Exit gate:** full flow passes 1,000 automated loops and a 24-hour soak; all global budgets pass on the locked device matrix; API/security/accessibility reviews are signed off; comparative report is reproducible; no P0/P1 defects and no unexplained P2 defect remain.

**Phase budget:** estimated 5,820 +LOC; hard maximum 8,120 +LOC.

---

## 5. Commit and review protocol

Every implementation commit must contain or link:

```text
Scope and invariant changed
Files changed
Benchmark scene and reference hardware
Build profile and feature set
Before/after P50, P95, P99
Before/after allocations, RSS, GPU bytes, draw calls, uploads
Golden/oracle result
Accessibility impact
Power/thermal impact when relevant
Rollback condition
```

Required branch/commit sequence:

```text
00-repair-workspace
01-lock-benchmark-baseline
02-add-core-contracts
03-incremental-reconcile
04-incremental-layout-and-events
05-damage-regions
06-retained-layers
07-gpu-batching-and-resources
08-frame-scheduler
09-layout-virtualization
10-pointer-and-gesture-arena
11-motion-engine
12-text-ime
13-aura-design-system
14-adaptive-effects
15-accessibility
16-vyom-backend
17-aura-lab
18-regression-hardening
19-reference-shell-release
```

## 6. File ownership boundaries

| Area | Owns | Must not own |
|---|---|---|
| `core/` | portable types and contracts | GPU, Wayland, VYOM protocol, concrete design theme |
| `runtime/` | reconciliation, scheduling, routing | raw GPU commands, platform protocol implementation |
| `graphics/` | renderer-neutral display/damage/cache model | window event loop, product component styles |
| `wgpu/` | GPU resources, passes, shaders, present integration | widget business state, compositor policy |
| `widget/` | generic controls/layout widgets | VYOM visual identity or platform protocol |
| `aura-design/` | tokens and production components | GPU backend internals |
| `aura-motion/` | math/timelines/transitions | independent frame loop |
| `aura-gesture/` | recognition/competition/nesting | platform event collection |
| `aura-accessibility/` | semantics/focus/adapters | visual painting |
| `aura-platform-vyom/` | VYOM/Wayland integration | renderer algorithms or component styling |
| `aura-profiler/`, `tools/` | observability and development UX | production decisions required for correctness |

## 7. Change-stop conditions

Stop and repair the active phase before continuing if any occurs:

- clean idle produces a frame, layout, paint, or recurring timer;
- a cache/queue lacks a measurable bound;
- a new fast path has no correctness oracle/fallback;
- a benchmark improvement depends on different pixels, fonts, data, or disabled behavior;
- P99 improves while visible hitching, power, memory, accessibility, or correctness regresses;
- secure text/protected content reaches logs, profiler, semantics value, clipboard, screenshots, or overview thumbnails;
- a phase exceeds its maximum LOC without an approved RFC;
- the native VYOM backend removes the known-good backend before parity;
- a golden changes without a reviewed diff and reason;
- unsafe code lacks written invariants and targeted verification.

## 8. Definition of Aura 1.0

Aura 1.0 is production-ready only when all of the following are true:

- Reference shell completes the full Phase 18 flow reliably.
- Selected phone, tablet, desktop and watch profiles meet their locked 60/90/120 Hz budgets.
- No redraw occurs while idle and no clean subtree is laid out, rebuilt, or uploaded.
- Memory and GPU resources stabilize after navigation, image, text, effect and window churn.
- Touch cancellation, capture, nested scrolling, keyboard navigation and screen-reader actions are correct.
- English, Hindi/Devanagari, Punjabi/Gurmukhi, Arabic/RTL, Bengali, Tamil and emoji test corpora pass.
- Rotation, fractional scaling, multi-display, suspend/resume and input hot-plug pass.
- GPU loss recovers or enters a safe minimal shell without exposing protected content.
- Design-system states, contrast, large text, reduced motion and 48 x 48 touch targets pass.
- Benchmark and golden methods are reproducible from a clean checkout.
- Competitive claims name the exact device, scene and metric and publish both strengths and limitations.
- All phase exit gates, LOC ceilings, CI checks and release reviews pass.

The result is not “best” because the roadmap says so. It earns that position only when Aura repeatedly produces better measured latency and efficiency while preserving visual quality, accessibility, safety, and developer usability on VYOM’s real hardware.
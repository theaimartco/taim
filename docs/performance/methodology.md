# Benchmark methodology

> Status: Phase 1 baseline. This document is the reproducibility contract
> `aura_bench` (`tools/aura_bench/`) and `scripts/perf/run-baseline.sh`
> exist to enforce. Update it whenever a phase changes what is measured
> or how.

## What this phase establishes, and what it does not

Phase 1's objective (`phase.md`) is "create a benchmark laboratory before
changing engine behavior." It does not optimize anything. Every number
`aura_bench` produces today is the **unoptimized baseline** every later
phase's "P95 improved by N%" claim must be measured against. Do not read
Phase 1 numbers as a performance claim about VYOM Aura; read them as the
starting line.

## Reproducibility rules

1. **Same scene.** Compare reports only where `report.scene` matches.
2. **Same renderer backend.** Compare reports only where
   `report.environment.renderer_backend` matches (`"wgpu"` vs
   `"tiny-skia"` are different code paths with different cost profiles;
   see `tools/aura_bench/src/main.rs`'s backend selection, which tries
   `wgpu` first and falls back to `tiny-skia` unless `--backend` forces
   one).
3. **Same device.** Compare reports only where
   `report.environment.device` matches. `aura_bench --compare <path>`
   enforces rules 1-3 automatically and refuses to compare otherwise.
4. **Same build profile.** Compare `release`-profile reports only against
   other `release`-profile reports. `scripts/perf/run-baseline.sh` always
   builds `--release`.
5. **Warm-up before measurement.** At least 30 seconds of warm-up
   (`--warm-up-secs`, default 30) runs before any measured frame, so JIT/
   cache/allocator warm-up cost does not leak into the measured samples.
6. **At least five measured runs.** `aura_bench` refuses `--runs` below
   5. Each run replays the scene's full deterministic frame sequence
   (`Scene::advance`/`Scene::input_events` keyed by frame index and
   `--seed`) from a fresh scene instance, so two runs with the same seed
   produce byte-identical input.
7. **Report the median run, but keep every run.** `report.median` is the
   run whose P95 frame time is the middle value across all measured runs
   (see `median_of` in `tools/aura_bench/src/report.rs`); `report.runs`
   keeps every individual run's summary so outliers are visible, not
   hidden by the aggregate.
8. **CPU governor.** `scripts/perf/run-baseline.sh` checks (on Linux)
   that every CPU's `scaling_governor` is `performance` and warns loudly
   if not; it does not change the governor for you, since that requires
   privileges this script should not assume it has.
9. **Do not compare across unlike environments.** This is the rule every
   other rule above exists to make machine-checkable. A number from a
   laptop on battery power is not comparable to the same laptop plugged
   in, let alone to different hardware.

## What is measured, and what is not yet

`phase.md`'s "Measurements required" list, and Phase 1's actual coverage:

| Required measurement | Status | Where |
|---|---|---|
| Cold start to first visible frame | **Not measured.** `aura_bench` never creates a real window; there is no "first visible frame" in a headless run. This belongs to Phase 14's platform-backend work, where a real compositor surface exists. | — |
| CPU frame phases: diff, event, layout, semantics, paint prep, upload | **Partially measured.** `aura_runtime::UserInterface` does not yet split diff/event/layout into separately-timed phases (that split is Phase 3's job); `aura_bench` records `update` (diff+event+layout folded together) and `draw` (paint preparation) as they exist today. Semantics and upload have no timing hooks yet and are not fabricated. | `tools/aura_bench/src/metrics/frame_time.rs` |
| GPU duration and presentation time | **Approximated.** No real presentation feedback exists headlessly (Phase 6). `present` measures `Renderer::screenshot`'s readback time as a stand-in. | `tools/aura_bench/src/metrics/frame_time.rs` |
| Allocations/frame, RSS, GPU bytes, draw calls, batch count, uploaded bytes | **Partially measured.** Allocation counting and RSS are implemented (opt-in via `--features profiling` for allocations; RSS via `/proc/self/status` on Linux only). GPU bytes/draw calls/batch count/uploaded bytes have no instrumentation hook in `aura_wgpu` yet; that is Phase 5's `wgpu/src/engine.rs` and `wgpu/src/layer.rs` work. | `tools/aura_bench/src/metrics/{allocations,memory}.rs` |
| Damage area/full-screen area, visited/laid-out/painted/cached nodes | **Not measured.** There is no damage tracking yet (Phase 4) and no per-node visit counters (Phase 2's `runtime/src/diagnostics.rs`). | — |
| Event timestamp to submit, and to compositor presentation feedback | **Partially measured.** `metrics::latency` times "event dispatched to `UserInterface::update`" through "readback returned", which is the headless stand-in for "event to submit." There is no compositor, so there is no presentation-feedback leg yet. | `tools/aura_bench/src/metrics/latency.rs` |
| Idle wake-ups and submissions | **Not measured.** There is no frame scheduler yet (Phase 6); a headless harness that calls `draw`/`screenshot` on every configured frame has no concept of "idle" to begin with. | — |

Every "not measured" row above is a real gap, not an oversight to paper
over. The phase responsible for closing it is named so a later
`aura_bench` change can find the right point to add the missing metric
rather than guessing at one.

## Renderer backend selection

`aura_bench` asks `aura_renderer::Renderer::new(settings, backend)` for a
headless renderer. With `backend: None` (the default), it tries `wgpu`
first (a real GPU adapter, headless — no window, no surface) and falls
back to `tiny-skia` (CPU rasterization) if no GPU adapter is available,
matching `aura_renderer`'s own fallback-dispatcher behavior. Pass
`--backend wgpu` or `--backend tiny-skia` to force one and fail loudly if
it is unavailable, rather than silently measuring the other.

The report's `environment.renderer_backend` field always records which
backend actually ran, since "requested `wgpu`, silently got `tiny-skia`"
would otherwise produce numbers that look comparable but are not.

## Scenes

See `tools/aura_bench/src/scenes/mod.rs` for the full registry
(`aura_bench --list`). Each scene module's doc comment records which
phase.md-described workload it stands in for today and which later phase
is expected to change its underlying mechanism (e.g. `blur_layers` uses
styled containers as a stand-in for GPU blur effects, which do not exist
until Phase 12).

Every scene is deterministic: `Scene::advance(frame_index)` and
`Scene::input_events(frame_index)` are pure functions of the frame index
and the scene's own seeded state, never of wall-clock time or an RNG
without a fixed seed. This is what makes two runs of the same scene
comparable at all.

## Statistical method

- Percentiles (P50/P95/P99) are computed by sorting all frame-total
  durations in a measured run and taking the value at
  `round((n - 1) * p)`, clamped to the last index. See `percentile` in
  `tools/aura_bench/src/metrics/frame_time.rs`.
- The "missed frame rate" is the fraction of frames in a run whose total
  CPU time (`update + draw + present`) exceeded the frame budget for the
  configured `--refresh-rate` (`1/60`, `1/90`, or `1/120` second).
- The reported "median run" is chosen by comparing each run's P95 (not
  by averaging runs together), so the reported numbers are always a real
  run's real samples, never a synthetic blend.
- `aura_bench --compare <path>` computes the relative difference between
  two P95s as `(max - min) / max`, matching the exit gate's "differ by
  <= 5%" wording (a symmetric, bounded measure -- see
  `relative_difference` in `tools/aura_bench/src/report.rs`).

## Report format

Reports are versioned JSON (`schema_version`, currently `1`); see
`tests/bench/report_schema.rs` for the fields every report must contain
and `tools/aura_bench/src/report.rs` for the authoritative shape. A
schema version bump is required whenever an existing field is removed,
renamed, or changes meaning; adding a new required field is also treated
as requiring a bump under this phase's policy, since
`tests/bench/report_schema.rs` asserts an exact required-field list per
version rather than "at least these fields."

## Running a baseline

```bash
# One scene, human summary + JSON to stdout:
cargo run --release --package aura_bench -- --scene idle_shell

# Every registered scene, saved to benchmarks/baseline/<scene>.json:
scripts/perf/run-baseline.sh

# Compare a fresh run against a saved baseline:
cargo run --release --package aura_bench -- \
    --scene idle_shell --compare benchmarks/baseline/idle_shell.json
```

See `benchmarks/devices.toml` for how to register a named reference
device before checking in a report that should be trusted as a real
baseline rather than an ad hoc local run.

## Known limitation: P95 stability on an unpinned development machine

The reports checked into `benchmarks/baseline/` for this phase were
produced on `dev-workstation-01` (see `benchmarks/devices.toml`) with the
CPU frequency governor at `powersave`, not `performance` --
`scripts/perf/run-baseline.sh` warned about exactly this and did not (and
should not, without explicit authorization) change a system-wide governor
setting itself.

Re-running `text_heavy` against its saved baseline on this machine
produced P95 values ranging from ~2.44 ms to ~3.56 ms across otherwise
identical invocations -- a ~30% spread, exceeding the exit gate's 5%
threshold. P50 was stable within 0.5% across the same runs (~2.34 ms
every time). This is consistent with occasional OS scheduling
interference on a shared, unpinned machine hitting one or two of the 180
sampled frames per run: at `FRAMES_PER_RUN = 180`
(`tools/aura_bench/src/main.rs`), P95 is the value at rank
`round(179 * 0.95) = 170`, so a single delayed frame landing in the top
~9 of 180 can move the reported P95 substantially. The other six scenes
(`idle_shell`, `ten_thousand_rows`, `image_grid`, `blur_layers`,
`animated_cards`, `multitouch`) stayed within 0.35-2.70% across the same
comparison, which is consistent with `text_heavy` being the most
per-frame-expensive scene (real multilingual glyph shaping via
cosmic-text) and therefore the most exposed to a fixed-duration
scheduling hiccup landing inside any single frame.

This is not treated as a passing baseline for `text_heavy` on this
machine, and it is recorded here rather than hidden, per phase.md's own
rule: "Do not compare results from unlike environments" and the general
principle that a benchmark tool's job is to surface unreliable
measurements, not launder them into a false pass. Closing this gap
requires one of: a real reference device with a pinned governor (Phase
1's device registration process, not a code change), a larger
`FRAMES_PER_RUN` for text-heavy scenes specifically (trades runtime for
statistical stability), or accepting P99/mean as the tracked statistic
for scenes with this cost profile instead of P95. None of those is a
Phase 1 decision to make unilaterally; it is recorded here for whoever
locks the first real VYOM reference device.

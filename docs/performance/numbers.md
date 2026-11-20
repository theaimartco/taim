# Taim by the numbers

Every number in the README comes from this page, and every number on this
page comes from a file in this repository or a command you can run. When a
number has not been measured, this page says so instead of estimating it.

## Quality

From the release check of commit `936de6c` (2026-11-20, Apple M1, macOS,
Rust 1.95.0), `scripts/release/check.sh`, report in
`target/release-check-<commit>/report.md`:

| | |
|---|---|
| Release-gate steps | 14 of 14 passed, none skipped |
| Tests | 2,534 passed, 0 failed, 11 ignored, in 101 test suites |
| Targets built and linted, warnings denied | 11: macOS (arm64, x86_64), Linux (x86_64, arm64), Windows (x64, arm64), Android (arm64, x86_64), iOS (device, simulator), WebAssembly |
| Browsers | Chromium, Firefox and WebKit draw the app, respond to clicks and log no errors |
| New project | `taim-new` output builds and passes its own tests |
| External consumer | A crate outside the workspace builds against Taim |
| Dependencies | `cargo deny`: advisories, licenses, bans and sources pass |

Reproduce: `bash scripts/release/check.sh target/release-check` (see
[production gates](../release/production-gates.md) for the tools it needs).

## Frame cost

The framework's own CPU time per frame: event handling, state propagation,
layout and recording draw commands. It does **not** include GPU execution,
compositor presentation or display latency, so it is not "frame time" in
the sense of how long until pixels change. It is the part a UI framework is
responsible for, and what the rest of the 8.33 ms (120 Hz) or 16.67 ms
(60 Hz) budget is left for.

Measured by `tools/taim_bench` on `dev-workstation-01` (Intel Core
i7-12700H, Iris Xe via Mesa Vulkan, 16 GiB, Ubuntu 26.04, headless, 60 Hz
pacing, powersave governor), commit `5e2d789` (2026-08-23), release build,
5 runs × 180 frames per scene, median run. Raw data:
[`benchmarks/baseline/*.json`](../../benchmarks/baseline/).

| Scene | Workload | p50 | p95 | p99 | Dropped |
|---|---|---:|---:|---:|---:|
| `idle_shell` | An app shell with nothing changing | 4.01 µs | 6.51 µs | 7.22 µs | 0 |
| `effect_ladder` | Layered visual effects | 4.46 µs | 8.75 µs | 11.42 µs | 0 |
| `ten_thousand_rows` | 10,000 widgets in layout | 6.24 µs | 10.76 µs | 14.35 µs | 0 |
| `multitouch` | Several pointers contending in the gesture arena | 6.63 µs | 12.02 µs | 16.81 µs | 0 |
| `animated_cards` | Concurrent spring animations | 7.88 µs | 14.66 µs | 25.08 µs | 0 |
| `text_heavy` | Shaped multi-font paragraphs | 8.50 µs | 14.86 µs | 24.77 µs | 0 |
| `blur_layers` | Stacked blurs | 8.35 µs | 15.95 µs | 22.12 µs | 0 |
| `motion_stress` | High-frequency transform animation | 4.55 µs | 23.21 µs | 36.08 µs | 0 |
| `text_edit` | Typing: mutation, shaping, layout | 8.79 µs | 23.38 µs | 33.79 µs | 0 |
| `damage_churn` | Rapid invalidation of subtrees | 10.49 µs | 24.02 µs | 26.47 µs | 0 |
| `image_grid` | Image atlas packing | 10.81 µs | 29.95 µs | 36.32 µs | 0 |
| `virtual_list_variable` | Virtualized list, variable row heights | 15.72 µs | 33.70 µs | 42.33 µs | 0 |
| `virtual_list` | Scrolling 100,000 rows | 27.49 µs | 57.59 µs | 74.40 µs | 0 |

The heaviest scene's p95, 57.59 µs, is 1/145 of a 120 Hz frame and 1/289
of a 60 Hz frame.

Reproduce: `scripts/perf/run-baseline.sh --device <name>` with a device
entry in `benchmarks/devices.toml`; compare runs with
`scripts/perf/compare-baseline.sh`.

## Memory

From the same runs:

| | |
|---|---|
| Live heap at the end of a run | ~24 KiB in every scene, identical in all 5 runs |
| Peak resident memory | 136–197 MiB (`text_heavy` is the 197) |
| Allocations per frame | 200 (`blur_layers`) to 9,800 (`text_edit`); not zero, and a target for 1.0 |

The live heap staying constant across runs means no growth under load.
Allocations per frame are reported because they are measured; reducing
them in the text and list paths is planned work.

## Binary size

A complete app (`examples/counter`), stripped, measured 2026-11-20:

| | thin LTO | `release` (fat LTO) | `release-small` |
|---|---:|---:|---:|
| macOS arm64 | 6.05 MiB | 5.26 MiB | **3.58 MiB** |
| iOS arm64 | 6.47 MiB | 5.77 MiB | **4.00 MiB** |
| iOS arm64, gzip -9 | 2.87 MiB | 2.65 MiB | **2.07 MiB** |

`release-small` optimizes for size except the rasterizers. It costs about
25% in software rendering (`text_heavy` p50 3.6 → 4.1 ms on tiny-skia) and
nothing on the GPU. Pass `--size` to the macOS or iOS packager to use it.

## Runtime microbenchmarks

From [phase 2 results](phase2-results.md) (Apple Silicon, macOS, September
2026):

| Operation | Time | Allocations |
|---|---:|---:|
| Broadcast an event, 0 listeners | 21 ns | 0 |
| Broadcast an event, 1 listener | 108 ns | 0 |
| Broadcast an event, 32 listeners | 1.9 µs | 0 |
| Spawn a task | ~127 ns | — |
| Poll a task to completion | ~64 ns | — |
| Normalize one input event | ~44 ns | — |
| Atlas allocation, p50 / p99 | 500 ns / 4.3 µs | 0 |

An unchanged window schedules no frames: zero framework wakeups when every
window is clean and nothing animates.

## Not yet measured

These matter, and we will publish them when they are measured on named
hardware, not before:

- Cold start to first frame.
- Input-to-photon latency (needs a calibrated sensor, not a timestamp).
- Energy use, idle and under load.
- Frame cost on phones.
- Comparisons with other Rust GUI frameworks. The protocol, which requires
  the same device, scenes and inputs for every framework, is in
  [four-framework comparison](four-framework-comparison.md). Until it has
  been run, Taim makes no claim of being faster than any other framework.

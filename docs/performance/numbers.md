# Taim by the numbers

Every number in the README comes from this page, and every number on this
page comes from a file in this repository or a command you can run. When a
number has not been measured, this page says so instead of estimating it.

## Quality

From the release check of commit `0571d48` (2026-11-20, Apple M1, macOS,
Rust 1.95.0), `scripts/release/check.sh`, report in
`target/release-check-<commit>/report.md`:

| | |
|---|---|
| Release-gate steps | 14 of 14 passed, none skipped |
| Tests | 2,542 passed, 0 failed, 11 ignored, in 101 test suites |
| Targets built and linted, warnings denied | 11: macOS (arm64, x86_64), Linux (x86_64, arm64), Windows (x64, arm64), Android (arm64, x86_64), iOS (device, simulator), WebAssembly |
| Browsers | Chromium, Firefox and WebKit draw the app, respond to clicks and log no errors |
| New project | `taim-new` output builds and passes its own tests |
| External consumer | A crate outside the workspace builds against Taim |
| Dependencies | `cargo deny`: advisories, licenses, bans and sources pass |

Reproduce: `bash scripts/release/check.sh target/release-check` (see
[production gates](../release/production-gates.md) for the tools it needs).

## Frame cost

The framework's own CPU time per frame: building the view, reconciling and
laying it out, handling events and recording draw commands
(`build + update + draw`). It does **not** include GPU execution,
compositor presentation or display latency, so it is not "time until pixels
change". It is the part a UI framework is responsible for, and what the
rest of the 8.33 ms (120 Hz) or 16.67 ms (60 Hz) budget is left for.

Measured by `tools/taim_bench` on an Apple M1 (8 GB, macOS 26.6, Metal, on
battery; device `apple-m1-dev` in `benchmarks/devices.toml`),
commit `c3af138` (2026-11-20), release build with allocation counting,
30 s warm-up, 5 runs × 180 frames per scene, median run. Raw data:
[`benchmarks/runs/2026-11-20-apple-m1/`](../../benchmarks/runs/2026-11-20-apple-m1/).

| Scene | Workload | p50 | p95 | p99 | Allocs/frame | Peak RSS | Dropped |
|---|---|---:|---:|---:|---:|---:|---:|
| `idle_shell` | An app shell with nothing changing | 6.5 µs | **17.5 µs** | 19.7 µs | 69 | 40 MiB | 0 |
| `text_edit` | Typing: mutation, shaping, layout | 19.6 µs | **33.8 µs** | 62.7 µs | 49 | 62 MiB | 0 |
| `animated_cards` | Concurrent spring animations | 28.4 µs | **54.3 µs** | 58.1 µs | 123 | 39 MiB | 0 |
| `text_heavy` | Shaped multi-font paragraphs | 16.0 µs | **55.9 µs** | 69.2 µs | 44 | 103 MiB | 0 |
| `multitouch` | Several pointers contending in the gesture arena | 21.8 µs | **57.8 µs** | 65.2 µs | 72 | 46 MiB | 0 |
| `effect_ladder` | Layered visual effects | 64.5 µs | **75.6 µs** | 86.8 µs | 37 | 42 MiB | 0 |
| `ten_thousand_rows` | 10,000 widgets in layout | 35.5 µs | **88.2 µs** | 96.8 µs | 82 | 68 MiB | 0 |
| `blur_layers` | Stacked blurs | 78.0 µs | **90.0 µs** | 92.4 µs | 96 | 39 MiB | 0 |
| `image_grid` | Image atlas packing | 64.1 µs | **106.3 µs** | 126.1 µs | 290 | 48 MiB | 0 |
| `signal_list_100k` | 100,000-row list, one row changing each frame through a signal | 102.8 µs | **117.1 µs** | 125.5 µs | 164 | 36 MiB | 0 |
| `signal_list_1m` | 1,000,000-row list, one row changing each frame through a signal | 108.9 µs | **121.7 µs** | 157.1 µs | 164 | 42 MiB | 0 |
| `virtual_list` | Scrolling 100,000 rows | 95.9 µs | **123.3 µs** | 137.3 µs | 394 | 45 MiB | 0 |
| `damage_churn` | Rapid invalidation of subtrees | 163.6 µs | **225.7 µs** | 232.5 µs | 436 | 53 MiB | 0 |
| `motion_stress` | High-frequency transform animation | 46.5 µs | **409.5 µs** | 422.0 µs | 298 | 42 MiB | 0 |
| `virtual_list_variable` | 100,000 rows of variable height | 502.7 µs | **681.6 µs** | 709.5 µs | 276 | 43 MiB | 0 |

What this shows:

- **Cost follows what changes, not how much data there is.** Growing the
  signal list tenfold, from 100,000 to 1,000,000 rows, raises p95 by 4%
  (117.1 → 121.7 µs).
- **Every scene's p95 fits 12 times into a 120 Hz frame**; all but two fit
  36 times. No frame was dropped in 13,500 measured frames.
- **The slowest scene is a known cost.** A variable-height list rebuilds
  its table of row offsets on every layout, which is O(rows); fixed-height
  lists and the view API's `list` do not. Caching that table is planned.

Reproduce: `scripts/perf/run-baseline.sh --device <name>` with a device
entry in `benchmarks/devices.toml`, or one scene:
`cargo run --release -p taim_bench --features profiling -- --scene virtual_list --device <name>`.

### Earlier baseline

[`benchmarks/baseline/`](../../benchmarks/baseline/) holds a run from
2026-08-23 (commit `b67fd3f`, Intel Core i7-12700H, Ubuntu). Its frame
cost was `update + draw` only, before the harness began timing `build`
(view construction and layout) in commit `b6d62bf`, so its numbers are
lower than the full-frame figures above and are not comparable with
them. Use the table above.

## Memory

From the same runs:

| | |
|---|---|
| Peak resident memory | 36–103 MiB per scene (`text_heavy` is the 103) |
| Live heap at the end of a run | 6–114 KiB, within 1 KiB across runs 2 to 5 of every scene: no growth under sustained load |
| Allocations per frame | 37 (`effect_ladder`) to 436 (`damage_churn`) |

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

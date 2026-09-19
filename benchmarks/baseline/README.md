# Baseline reports

One `<scene>.json` report per `taim_bench` scene, written by
`scripts/perf/run-baseline.sh`. 13 scenes are currently checked in.

All 13 were captured on **2026-08-23** on `dev-workstation-01`, from a clean
worktree at commit `5e2d7896dbe4`, with the `profiling` feature on so
allocation counting is live.

These files use report schema v1. That schema omitted UI build/diff/layout
from headline frame time. Current `taim_bench` uses v2 and deliberately
refuses v1 comparisons. Keep the files as historical evidence, not as a
current performance baseline; recapture on the named device from a clean tree.

## What was wrong with the previous set

The reports checked in before that date recorded `environment.os = "macos"`
while `benchmarks/devices.toml` registers `dev-workstation-01` as a Linux
machine, so they could not be attributed to any device at all. They were also
captured from a dirty worktree (`...-dirty`), meaning the commit they name
does not describe the sources measured and the run cannot be reproduced by
anyone, including its author. And `allocation_counting_active` was `false`
throughout, so every allocation field read zero because it was never measured
-- while `README.md` quoted allocation figures from a run that was never
committed.

`git log -- benchmarks/baseline/` still has the old values. See `Capstone.md`
F-02, F-03 and F-04 for the full analysis.

## How this is prevented from recurring

- `tools/taim_bench/build.rs` captures the git commit and rustc version at
  build time, and re-runs when `HEAD`, the ref it resolves to, or the index
  changes -- watching `HEAD` alone silently went stale on every commit made
  while staying on one branch.
- `taim_bench --save` refuses a report when any of `git_commit`,
  `rustc_version`, or `device` is unknown (`--allow-unidentified` opts out),
  **and** refuses one captured from a dirty worktree (`--allow-dirty` opts
  out). Either flag prints a warning, and a report saved with either must not
  be committed here.
- `scripts/perf/run-baseline.sh` requires `--device` naming a
  `benchmarks/devices.toml` entry, and builds with `--features profiling`.

## Producing a baseline

```bash
scripts/perf/run-baseline.sh --device dev-workstation-01
```

Register the machine in `benchmarks/devices.toml` first, filling in every
field of the `[[device]]` template. `docs/performance/methodology.md` has the
warm-up, run-count, governor, and cache-state contract a run must satisfy
before its output belongs here.

These are headless harness measurements. They establish that the harness is
reproducible on a named machine. They are **not** a claim about VYOM target
hardware, and they are **not** comparable to a windowed Flutter or React
Native run -- see `benchmarks/competitors/README.md` for what such a
comparison would require, and `Capstone.md` section 11 for the full protocol.

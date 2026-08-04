# Baseline reports

This directory holds one `<scene>.json` report per `aura_bench` scene, as
written by `scripts/perf/run-baseline.sh`.

**It is currently empty by design.** No attributable baseline exists yet.

## Why it is empty

An earlier set of reports was checked in here with
`environment.git_commit`, `environment.rustc_version`, and
`environment.device` all `null`. The machine that produced them is
described in `docs/performance/methodology.md` and
`benchmarks/devices.toml` as `dev-workstation-01`, but the reports
themselves did not say so, and the commit and toolchain that produced
them cannot be recovered.

A report that cannot name the tree, compiler, and machine behind it
cannot be compared against anything, which is the only thing a baseline
is for. Rather than leave numbers here that look authoritative and are
not, they were removed; `git log -- benchmarks/baseline/` still has them
if the raw values are ever needed.

The analysis those runs produced is not lost -- the `text_heavy` P95
stability finding is written up in full in
`docs/performance/methodology.md` under "Known limitation: P95 stability
on an unpinned development machine".

## How this is prevented from recurring

- `tools/aura_bench/build.rs` captures the git commit (with a `-dirty`
  suffix when the worktree is modified) and the rustc version at build
  time.
- `aura_bench --save <PATH>` refuses to write a report when any of
  `git_commit`, `rustc_version`, or `device` is unknown. Saving one
  anyway requires an explicit `--allow-unidentified`, which also prints a
  warning.
- `scripts/perf/run-baseline.sh` requires `DEVICE` to be set and to match
  a `[[device]]` entry in `benchmarks/devices.toml` before it will run.

## Producing a baseline

```bash
DEVICE=dev-workstation-01 scripts/perf/run-baseline.sh
```

Register the machine in `benchmarks/devices.toml` first, filling in every
field of the `[[device]]` template. See `docs/performance/methodology.md`
for the warm-up, run-count, governor, and cache-state contract the run
has to satisfy before its output belongs in this directory.

Reports here are still headless harness measurements. They establish that
the harness is reproducible on a named machine; they are not a claim
about VYOM target hardware, and they are not comparable to a windowed
Flutter or React Native run. See `docs/status.md` for what the
performance evidence currently does and does not support.

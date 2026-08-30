# Taim 1.0 Performance Report

**Not yet measured.**

This document previously reported frame times for scenes named "Launcher
Scroll", "Settings Navigation", and "Complex Grid" on "VYOM Reference
Hardware, 8-core ARM64". None of those existed: no such device is registered
in `benchmarks/devices.toml`, no ARM64 run is committed under
`benchmarks/baseline/`, and `tools/taim_bench/src/scenes/` defines no scene by
any of those names. The numbers had no provenance and have been removed
rather than corrected. See `Capstone.md` F-05.

## What is actually measured

The 13 committed scene baselines under `benchmarks/baseline/`, reproducible
with:

```bash
scripts/perf/run-baseline.sh --device <name from benchmarks/devices.toml>
```

Read them with `docs/performance/methodology.md`, which defines what the
frame-cost metric does and does not include.

## What is not measured

- **Any comparison against Flutter, React Native, or Qt.**
  `benchmarks/competitors/` states the rules such a comparison must follow and
  contains no results. See `Capstone.md` §11.
- **Cold start**, **energy draw**, and **steady-state application memory** —
  the three axes on which a framework with no garbage collector should win,
  and on which Taim currently has no evidence at all.
- **Anything on VYOM target hardware.** Every committed baseline comes from a
  development machine.

This file will report results when there are results. Until then it reports
that there are none, which is the only honest thing it can say.

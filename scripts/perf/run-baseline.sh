#!/usr/bin/env bash
#
# Captures a performance baseline for every scene and commits it to
# benchmarks/baseline/.
#
# These baselines are what `compare-baseline.sh` gates future runs
# against. They are only meaningful when captured on locked reference
# hardware -- a shared CI runner produces numbers nobody should trust
# (roadmap rule 1: measure first, on reproducible hardware). Pass a
# --device name matching an entry in benchmarks/devices.toml so the
# report records what produced it.
#
# Usage:
#   scripts/perf/run-baseline.sh --device vyom-ref-1
#   scripts/perf/run-baseline.sh --device dev-laptop --quick

set -euo pipefail

cd "$(dirname "$0")/../.."

readonly BASELINE_DIR="benchmarks/baseline"

device=""
warm_up=30
runs=5

while [[ $# -gt 0 ]]; do
  case "$1" in
    --device)
      device="${2:-}"
      shift 2
      ;;
    --quick)
      # For checking the pipeline works, not for a baseline worth
      # committing. The report records the shortened warm-up, so a
      # baseline captured this way is identifiable after the fact.
      warm_up=3
      runs=5
      shift
      ;;
    -h | --help)
      sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

if [[ -z "$device" ]]; then
  echo "error: --device is required so the baseline records what produced it." >&2
  echo "       See benchmarks/devices.toml for known device names." >&2
  exit 1
fi

# `profiling` switches on the counting allocator behind
# `metrics::allocations`. Without it every allocation field in the saved
# report reads zero because it was never measured, not because zero
# allocations occurred -- which is exactly how the previous baselines came
# to record `allocation_counting_active: false` while the README quoted
# allocation figures (Capstone.md F-04).
echo "Building aura_bench (release, profiling)..."
cargo build --release -p aura_bench --features profiling

readonly BENCH="target/release/aura_bench"

mkdir -p "$BASELINE_DIR"

scenes=$("$BENCH" --list)

echo "Capturing baselines for device '$device' (warm-up ${warm_up}s, $runs runs/scene)"
echo

for scene in $scenes; do
  printf '  %-28s' "$scene"

  if "$BENCH" \
    --scene "$scene" \
    --device "$device" \
    --warm-up-secs "$warm_up" \
    --runs "$runs" \
    --save "$BASELINE_DIR/$scene.json" \
    >/dev/null 2>&1; then
    echo "ok"
  else
    echo "FAILED"
    echo "    re-run without --save to see the error" >&2
    exit 1
  fi
done

echo
echo "Baselines written to $BASELINE_DIR/."
echo "Commit them in the same change as whatever they were captured for."

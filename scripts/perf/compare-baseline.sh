#!/usr/bin/env bash
#
# Re-runs every scene that has a committed baseline and fails if any has
# regressed beyond the tolerance `taim_bench --compare` enforces.
#
# This is the regression gate. It is only trustworthy on the same locked
# hardware the baselines were captured on, so it is not wired into the
# hosted-runner CI job; run it on the reference device, or on a
# self-hosted runner pinned to one.
#
# Usage:
#   scripts/perf/compare-baseline.sh --device vyom-ref-1

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
      warm_up=3
      runs=5
      shift
      ;;
    -h | --help)
      sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

if [[ -z "$device" ]]; then
  echo "error: --device is required; a comparison across devices is meaningless." >&2
  exit 1
fi

if [[ ! -d "$BASELINE_DIR" ]] || [[ -z "$(ls -A "$BASELINE_DIR" 2>/dev/null)" ]]; then
  echo "error: no baselines in $BASELINE_DIR." >&2
  echo "       Run scripts/perf/run-baseline.sh --device $device first." >&2
  exit 1
fi

echo "Building taim_bench (release)..."
cargo build --release -p taim_bench

readonly BENCH="target/release/taim_bench"

regressions=0
compared=0

for baseline in "$BASELINE_DIR"/*.json; do
  scene=$(basename "$baseline" .json)

  printf '  %-28s' "$scene"

  if "$BENCH" \
    --scene "$scene" \
    --device "$device" \
    --warm-up-secs "$warm_up" \
    --runs "$runs" \
    --compare "$baseline" \
    >/dev/null 2>&1; then
    echo "ok"
  else
    echo "REGRESSED"
    regressions=$((regressions + 1))
  fi

  compared=$((compared + 1))
done

echo
if [[ "$regressions" -gt 0 ]]; then
  echo "$regressions of $compared scenes regressed against their baseline." >&2
  echo "Re-run a single scene with --compare to see the numbers:" >&2
  echo "  $BENCH --scene <name> --device $device --compare $BASELINE_DIR/<name>.json" >&2
  exit 1
fi

echo "All $compared scenes within tolerance."

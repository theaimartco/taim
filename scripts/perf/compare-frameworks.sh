#!/usr/bin/env bash
# Fail closed: this command only validates actual same-device evidence.
# Never print "comparison complete" without real source reports.
set -euo pipefail

if [ "$#" -ne 5 ]; then
  echo "Usage: bash scripts/perf/compare-frameworks.sh OUTPUT.json TAIM.json ICED.json SLINT.json EGUI.json" >&2
  echo "Acquire and review actual physical presentation traces first." >&2
  exit 64
fi

OUTPUT="$1"
shift
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
exec python3 "$SCRIPT_DIR/compare-evidence.py" --output "$OUTPUT" "$@"

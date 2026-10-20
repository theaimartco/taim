#!/usr/bin/env bash
# Real Wayland session preflight. A successful present marker is NOT a photon,
# VYOM-native backend certification, latency, accessibility, or power proof.
set -euo pipefail

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
  echo "Usage: bash scripts/qa/wayland-smoke.sh NEW_EVIDENCE_DIR [POWER_CAP_ENERGY_UJ_PATH]" >&2
  exit 64
fi

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUTPUT="$1"
if [ -e "$OUTPUT" ]; then
  echo "Refusing to overwrite or append to existing evidence: $OUTPUT" >&2
  exit 2
fi
if [ -z "$(git -C "$ROOT" rev-parse --verify HEAD)" ]; then
  echo "No repository commit" >&2
  exit 2
fi
if [ -n "$(git -C "$ROOT" status --porcelain)" ]; then
  echo "Refusing to collect release evidence from a dirty checkout" >&2
  exit 2
fi
if [ -z "$(printenv WAYLAND_DISPLAY || true)" ] || [ -z "$(printenv XDG_RUNTIME_DIR || true)" ]; then
  echo "WAYLAND_DISPLAY and XDG_RUNTIME_DIR must identify a real Wayland session" >&2
  exit 2
fi
case "$WAYLAND_DISPLAY" in
  /*) SOCKET="$WAYLAND_DISPLAY" ;;
  *) SOCKET="$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ;;
esac
if [ ! -S "$SOCKET" ]; then
  echo "Wayland socket is missing or not a Unix socket: $SOCKET" >&2
  exit 2
fi

mkdir -p "$OUTPUT"
chmod 700 "$OUTPUT"
OUTPUT="$(cd "$OUTPUT" && pwd)"
cd "$ROOT"

{
  echo "Source commit: $(git rev-parse HEAD)"
  echo "UTC time: $(date -u +%FT%TZ)"
  echo "Kernel: $(uname -a)"
  echo "Wayland socket: $SOCKET"
  echo "XDG_SESSION_TYPE: $(printenv XDG_SESSION_TYPE || echo unavailable)"
  echo "LANG: $(printenv LANG || echo unavailable)"
  rustc --version
  cargo --version
  if command -v lspci >/dev/null 2>&1; then
    echo "GPU device inventory:"
    lspci -nn | grep -Ei 'VGA|3D controller|Display controller' || true
  fi
  if command -v vulkaninfo >/dev/null 2>&1; then
    echo "Vulkan summary:"
    vulkaninfo --summary 2>&1 | head -n 100 || true
  else
    echo "vulkaninfo unavailable (GPU capabilities not recorded)"
  fi
  if command -v wayland-info >/dev/null 2>&1; then
    echo "Wayland protocol globals:"
    wayland-info 2>&1 | head -n 160 || true
  else
    echo "wayland-info unavailable (protocol globals not recorded)"
  fi
} > "$OUTPUT/device-inventory.txt"

echo "Building the application with the FIRST_PRESENT measurement feature."
cargo build --locked --release -p mobile_shell --features taim/measurement 2>&1 | tee "$OUTPUT/build.log"

echo "Launching real Wayland session smoke. The result is a process/present marker test."
if [ "$#" -eq 2 ]; then
  python3 scripts/perf/measure-app.py \
    --output "$OUTPUT/measurement.json" \
    --startup-timeout 30 --warmup 10 --duration 15 \
    --workload "Wayland idle shell protocol smoke, not physical presentation timing" \
    --energy-counter "$2" \
    -- "$ROOT/target/release/mobile_shell" 2>&1 | tee "$OUTPUT/measurement.log"
else
  python3 scripts/perf/measure-app.py \
    --output "$OUTPUT/measurement.json" \
    --startup-timeout 30 --warmup 10 --duration 15 \
    --workload "Wayland idle shell protocol smoke, not physical presentation timing" \
    -- "$ROOT/target/release/mobile_shell" 2>&1 | tee "$OUTPUT/measurement.log"
fi

python3 - "$OUTPUT/measurement.json" <<'PY'
import json
import sys
from pathlib import Path
d = json.loads(Path(sys.argv[1]).read_text())
assert d["status"] == "ok", "Wayland preflight failed"
assert d["first_present_observed_upper_bound_ms"] is not None, "no successful present marker"
assert d["first_pixel_ms"] is None, "unexpected physical pixel claim"
print("PASS: real Wayland client reported first successful compositor present.")
print("NOT VERIFIED: physical display photons, VYOM backend, input/IME, Orca, GPU recovery, idle power.")
PY

(
  cd "$OUTPUT"
  sha256sum device-inventory.txt build.log measurement.json measurement.log > SHA256SUMS.txt
)
echo "Evidence captured at $OUTPUT; do NOT interpret this as production certification."

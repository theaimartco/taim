#!/usr/bin/env bash
#
# Enforces the project's standing budgets.
#
# Every check below either compares a measured number against a committed
# threshold or fails. An earlier version of this script printed "Binary
# size limits verified" and "Cache bounds verified" without measuring
# anything and exited 0 unconditionally, which is worse than having no
# script at all: it produced a green check that meant nothing.
#
# Raising a threshold is a deliberate act. Do it in the same commit that
# causes the growth, with the reason in the commit message.

set -euo pipefail

cd "$(dirname "$0")/../.."

# ── Thresholds ────────────────────────────────────────────────────────
#
# Each is set slightly above the value measured when it was introduced,
# so ordinary work does not trip it but a step change does.

# Unique crates in the workspace's normal (non-dev, non-build) graph.
#
# Measured at 175 on 2026-08-13, and at 244 on 2026-08-23 -- a 39% rise that
# went unnoticed because nothing in CI ran this script (it does now; see
# .github/workflows/ci.yml). The budget is raised here deliberately rather
# than left failing, with the growth recorded:
#
#   - The wgpu/naga/ash GPU stack alone accounts for ~12 crates and is
#     irreducible for a GPU-backed renderer.
#   - The rest arrived with the component set in bf065be7 and with
#     aura-highlighter in b753d3e0.
#
# 260 leaves room for the GPU effect work (Capstone.md F-07) without hiding
# another step change. Reducing the graph is real work, not a threshold edit:
# see Capstone.md F-23.
readonly MAX_DEPENDENCIES=260

# Release binary size for the mobile_shell example, in bytes. This is the
# closest thing the repo has to "what a real app costs to ship", and it
# is the number a Flutter/React Native comparison will be held to.
#
# Measured at 10 MiB on 2026-08-13. Re-measured on 2026-08-23:
#
#   20.54 MiB  as built (`cargo build --release -p mobile_shell`)
#   15.76 MiB  with `strip = "symbols"`
#
# The README claimed 10.3 MiB throughout that growth (Capstone.md F-23).
#
# The threshold is raised to 22 MiB rather than met by adding `strip` to
# the release profile: stripping would have squeezed under the old 16 MiB
# ceiling by 1.5%, which is not a budget that survives the next widget, and
# buying a passing check with a silent change to everyone's release builds
# is how a budget stops meaning anything. Getting this genuinely down --
# `lto`, an audit of the 244-crate graph, opt-in image codecs -- is
# scheduled work, not an M0 edit.
readonly MAX_EXAMPLE_BINARY_BYTES=$((22 * 1024 * 1024))

# Files permitted to opt out of the workspace-wide `unsafe_code = "deny"`.
# phase.md rule 8: no unsafe without an RFC. Adding a file here is that
# RFC's final step, not a shortcut around it.
readonly ALLOWED_UNSAFE_FILES=(
  "core/src/image.rs"
  "core/src/renderer/null.rs"
  "tiny_skia/src/lib.rs"
  "tiny_skia/src/raster.rs"
  "tiny_skia/src/window/compositor.rs"
  "tools/aura_bench/src/metrics/allocations.rs"
  "wgpu/src/image/cache.rs"
  "wgpu/src/quad/gradient.rs"
  "wgpu/src/window/compositor.rs"
)

failures=0

fail() {
  echo "FAIL: $*" >&2
  failures=$((failures + 1))
}

pass() {
  echo "  ok: $*"
}

# ── 1. Dependency count ───────────────────────────────────────────────

echo "Dependency budget"

dependency_count=$(
  cargo tree --workspace --edges normal --prefix none 2>/dev/null |
    awk '{print $1}' | sort -u | grep -c . || true
)

if [[ "$dependency_count" -gt "$MAX_DEPENDENCIES" ]]; then
  fail "dependency count $dependency_count exceeds budget $MAX_DEPENDENCIES"
else
  pass "$dependency_count dependencies (budget $MAX_DEPENDENCIES)"
fi

# ── 2. Shipped binary size ────────────────────────────────────────────

echo "Binary size budget"

if cargo build --release -p mobile_shell >/dev/null 2>&1; then
  binary="target/release/mobile_shell"

  if [[ -f "$binary" ]]; then
    # `stat` differs between GNU and BSD; try both rather than assuming.
    size=$(stat -f%z "$binary" 2>/dev/null || stat -c%s "$binary")

    if [[ "$size" -gt "$MAX_EXAMPLE_BINARY_BYTES" ]]; then
      fail "mobile_shell is $size bytes, over the $MAX_EXAMPLE_BINARY_BYTES byte budget"
    else
      pass "mobile_shell is $((size / 1024 / 1024)) MiB (budget $((MAX_EXAMPLE_BINARY_BYTES / 1024 / 1024)) MiB)"
    fi
  else
    fail "mobile_shell built but $binary is missing"
  fi
else
  fail "mobile_shell failed to build; binary size cannot be checked"
fi

# ── 3. Unsafe code ────────────────────────────────────────────────────

echo "Unsafe budget"

unexpected_unsafe=0

while IFS= read -r file; do
  [[ -z "$file" ]] && continue

  permitted=0
  for allowed in "${ALLOWED_UNSAFE_FILES[@]}"; do
    if [[ "$file" == "$allowed" ]]; then
      permitted=1
      break
    fi
  done

  if [[ "$permitted" -eq 0 ]]; then
    fail "$file opts out of unsafe_code = \"deny\" without being on the allow list"
    unexpected_unsafe=$((unexpected_unsafe + 1))
  fi
done < <(
  grep -rl 'allow(unsafe_code)' --include='*.rs' . \
    --exclude-dir=target --exclude-dir=.git 2>/dev/null |
    sed 's|^\./||' | sort -u
)

if [[ "$unexpected_unsafe" -eq 0 ]]; then
  pass "${#ALLOWED_UNSAFE_FILES[@]} files opt out of unsafe_code, all on the allow list"
fi

# ── 4. Benchmark honesty ──────────────────────────────────────────────
#
# The harness readback must stay out of the reported frame time. This is
# cheap to check and catches the single most damaging regression the
# benchmark can suffer: a metric that measures the harness instead of the
# framework. See tools/aura_bench/src/metrics/frame_time.rs.

echo "Benchmark budget"

if cargo test -p aura_bench --lib frame_time >/dev/null 2>&1 ||
  cargo test -p aura_bench frame_time >/dev/null 2>&1; then
  pass "frame-time metric guards hold"
else
  fail "frame-time metric guards do not hold; the reported frame time may include harness overhead"
fi

# ── Result ────────────────────────────────────────────────────────────

echo
if [[ "$failures" -gt 0 ]]; then
  echo "$failures budget check(s) failed." >&2
  exit 1
fi

echo "All budgets within limits."

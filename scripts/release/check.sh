#!/usr/bin/env bash
# The release check, run on a developer's machine instead of hosted CI.
#
#   bash scripts/release/check.sh [--allow-dirty] [--host-only] [EVIDENCE_DIR]
#
# A release run requires a clean tracked AND untracked checkout. Dirty and
# host-only diagnostic runs always return an incomplete result.
#
# Runs, in order, and stops at the first failure:
#   1. formatting                 cargo fmt --check
#   2. desktop lints              cargo clippy --workspace --all-targets -D warnings
#      crate lints                each crate alone, so features another
#                                 crate turns on cannot hide a problem
#   3. Android lints              clippy for aarch64- and x86_64-linux-android
#   4. web lints                  clippy for wasm32 (WebGPU and WebGL builds)
#   5. other platform lints       clippy for Windows, macOS, iOS and Linux targets
#   6. tests                      cargo test --workspace
#   7. new-project smoke          taim-new writes a project that builds
#   8. web smoke                  counter runs and responds in headless Chromium
#   9. Android link               cargo ndk build of mobile_shell (needs the NDK)
#
# Steps 4 and 5 type-check and lint without linking, so they need only the
# Rust targets, which they add with rustup; no Windows, Apple or browser
# toolchain is involved. Step 8 is reported as SKIPPED without node,
# playwright or the wasm-bindgen CLI. Step 9 is reported as SKIPPED when
# cargo-ndk or the NDK is missing: a check without it has not linked
# anything for Android. Device
# behaviour is a separate gate: scripts/qa/android-device-check.sh.
#
# With EVIDENCE_DIR, a report with the commit, toolchain and each step's
# result and log is written there.
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT" || exit 2

EVIDENCE=""
ALLOW_DIRTY=0
HOST_ONLY=0
for argument in "$@"; do
  case "$argument" in
    --allow-dirty) ALLOW_DIRTY=1 ;;
    --host-only) HOST_ONLY=1 ;;
    --*) echo "Unknown option: $argument" >&2; exit 2 ;;
    *)
      if [ -n "$EVIDENCE" ]; then echo "Use one evidence directory" >&2; exit 2; fi
      EVIDENCE="$argument"
      ;;
  esac
done

SOURCE_COMMIT="$(git rev-parse --verify HEAD)" || exit 2
SOURCE_STATUS="$(git status --porcelain --untracked-files=normal)" || exit 2
if [ -n "$SOURCE_STATUS" ] && [ "$ALLOW_DIRTY" = 0 ]; then
  echo "Release check requires a clean checkout, including untracked files." >&2
  echo "Use --allow-dirty for diagnostics only; it cannot pass the release gate." >&2
  exit 2
fi

if [ -n "$EVIDENCE" ]; then
  if [ -e "$EVIDENCE" ]; then
    echo "Refusing to overwrite existing evidence: $EVIDENCE" >&2
    exit 2
  fi
  mkdir -p "$EVIDENCE"
  EVIDENCE="$(cd "$EVIDENCE" && pwd)"
  LOGS="$EVIDENCE"
else
  LOGS="$(mktemp -d)"
fi

# The Android toolchain, as scripts/cloud/setup-android.sh installs it.
if [ -f "$HOME/.taim-android-env" ]; then
  # shellcheck disable=SC1091
  . "$HOME/.taim-android-env"
fi

ANDROID_FEATURES="taim/wgpu,taim/tiny-skia,taim/crisp,taim/hinting,taim/android"
RESULTS=()
FAILED=0
INCOMPLETE=0
TAIM_QA_PYTHON="${TAIM_QA_PYTHON:-python3}"

step() {
  local name="$1"
  shift
  local log
  log="$LOGS/$(echo "$name" | tr ' /' '__').log"
  printf '%-28s ' "$name"
  local started=$SECONDS
  if "$@" >"$log" 2>&1; then
    printf 'PASS  (%ss)\n' "$((SECONDS - started))"
    RESULTS+=("PASS | $name | $((SECONDS - started))s")
  else
    printf 'FAIL  (%ss) — see %s\n' "$((SECONDS - started))" "$log"
    tail -n 30 "$log" | sed 's/^/    /'
    RESULTS+=("FAIL | $name | $((SECONDS - started))s")
    FAILED=1
  fi
}

skip() {
  printf '%-28s SKIPPED  (%s)\n' "$1" "$2"
  RESULTS+=("SKIPPED | $1 | $2")
  INCOMPLETE=1
}

android_lints() {
  for target in aarch64-linux-android x86_64-linux-android; do
    rustup target add "$target" >/dev/null 2>&1 || return 1
    cargo clippy --locked -p taim -p mobile_shell --no-default-features \
      --features "$ANDROID_FEATURES" --target "$target" -- -D warnings || return 1
  done
}

EXAMPLES=(-p taim -p counter -p liquid -p mobile_shell -p apple_services_probe)
# Crates whose own tests are portable: linted, tests included, for every
# target. The root crate's integration tests are the Linux desktop suite
# (X11, Wayland, VYOM) and are linted on Linux only.
PORTABLE=(-p taim-view -p taim_core -p taim-signal -p taim-motion -p taim-gesture
  -p taim-design -p taim_graphics -p taim_winit -p taim_wgpu -p taim_widget
  -p taim_runtime -p taim_futures -p taim-raster -p taim_text_gpu -p taim-profiler
  -p counter -p liquid -p mobile_shell -p todos -p gallery -p taim-showcase)

crate_lints() {
  local packages
  packages="$(cargo metadata --locked --no-deps --format-version 1 |
    "$TAIM_QA_PYTHON" -c 'import json,sys; print("\n".join(p["name"] for p in json.load(sys.stdin)["packages"]))')" || return 1
  for package in $packages; do
    echo "== $package"
    cargo clippy --locked -p "$package" --all-targets -- -D warnings || return 1
  done
}

web_lints() {
  rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || return 1
  cargo clippy --locked "${EXAMPLES[@]}" --target wasm32-unknown-unknown -- -D warnings || return 1
  cargo clippy --locked "${EXAMPLES[@]}" --features taim/webgl --target wasm32-unknown-unknown -- -D warnings || return 1
  cargo clippy --locked "${PORTABLE[@]}" --all-targets --target wasm32-unknown-unknown -- -D warnings
}

platform_lints() {
  for target in x86_64-pc-windows-msvc aarch64-pc-windows-msvc \
    x86_64-apple-darwin aarch64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim \
    x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu; do
    rustup target add "$target" >/dev/null 2>&1 || return 1
    cargo clippy --locked "${EXAMPLES[@]}" --target "$target" -- -D warnings || return 1
    cargo clippy --locked "${PORTABLE[@]}" --all-targets --target "$target" -- -D warnings || return 1
  done
}

# Builds `counter` for the browser, then clicks it in headless Chromium and
# checks that it drew, that a click changed the picture, and that nothing
# reached the console as an error. Needs node with playwright, and the
# wasm-bindgen CLI.
web_smoke() {
  bash scripts/web/build.sh counter >/dev/null || return 1
  local browser
  for browser in chromium firefox webkit; do
    node scripts/web/smoke.js target/web/counter "$browser" || return 1
  done
}

new_project() {
  local scratch
  scratch="$(mktemp -d)"
  if ! cargo run --locked -q -p taim-new -- "$scratch/smoke_app" --taim-path "$ROOT"; then
    rm -rf "$scratch"
    return 1
  fi
  if ! (cd "$scratch/smoke_app" && CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}" cargo clippy -- -D warnings); then
    rm -rf "$scratch"
    return 1
  fi
  rm -rf "$scratch"
}

android_link() {
  cargo ndk --platform 26 -t arm64-v8a -t x86_64 \
    -o "$LOGS/jniLibs" build --locked -p mobile_shell --lib --release
}

echo "TAIM release check at $SOURCE_COMMIT"
echo
if [ -n "$SOURCE_STATUS" ]; then skip "clean source" "dirty checkout; diagnostic run only"; fi
if [ "$HOST_ONLY" = 1 ]; then skip "cross-platform coverage" "host-only diagnostic run"; fi

step "formatting" cargo fmt --all --check
[ "$FAILED" = 0 ] && step "QA tooling tests" "$TAIM_QA_PYTHON" -m unittest discover -s scripts/qa -p 'test_*.py'
[ "$FAILED" = 0 ] && step "release tooling tests" "$TAIM_QA_PYTHON" -m unittest discover -s scripts/release -p 'test_*.py'
[ "$FAILED" = 0 ] && step "desktop lints" cargo clippy --locked --workspace --all-targets -- -D warnings
[ "$FAILED" = 0 ] && step "crate lints" crate_lints
if [ "$HOST_ONLY" = 0 ]; then
  [ "$FAILED" = 0 ] && step "android lints" android_lints
  [ "$FAILED" = 0 ] && step "web lints" web_lints
  [ "$FAILED" = 0 ] && step "other platform lints" platform_lints
fi
[ "$FAILED" = 0 ] && step "tests" cargo test --locked --workspace --no-fail-fast -- --nocapture
# GPU tests pass without running when there is no adapter; say so.
if [ "$FAILED" = 0 ] && grep -qE "skipped: no (wgpu|GPU) adapter" "$LOGS/tests.log" 2>/dev/null; then
  skip "gpu tests" "no GPU adapter: install a driver, or mesa-vulkan-drivers"
fi
[ "$FAILED" = 0 ] && step "new-project smoke" new_project
[ "$FAILED" = 0 ] && step "external consumer" bash scripts/release/external-consumer-smoke.sh
if [ "$FAILED" = 0 ]; then
  if command -v cargo-deny >/dev/null 2>&1; then
    step "dependency audit" cargo deny check licenses advisories
  else
    skip "dependency audit" "cargo-deny is not installed"
  fi
fi
if [ "$FAILED" = 0 ] && [ "$HOST_ONLY" = 0 ]; then
  if ! command -v wasm-bindgen >/dev/null 2>&1; then
    skip "web smoke" "the wasm-bindgen CLI is not installed"
  elif ! node -e "require('playwright')" >/dev/null 2>&1; then
    skip "web smoke" "node with playwright is not available"
  else
    step "web smoke" web_smoke
  fi
fi
if [ "$FAILED" = 0 ] && [ "$HOST_ONLY" = 0 ]; then
  if ! command -v cargo-ndk >/dev/null 2>&1; then
    skip "android link" "cargo-ndk is not installed"
  elif [ -z "${ANDROID_NDK_HOME:-}" ] && [ -z "${ANDROID_NDK_ROOT:-}" ]; then
    skip "android link" "ANDROID_NDK_HOME is not set"
  else
    step "android link" android_link
  fi
fi

# A green run is invalidated if its source changed while checks were running.
FINAL_COMMIT="$(git rev-parse --verify HEAD)" || FAILED=1
FINAL_STATUS="$(git status --porcelain --untracked-files=normal)" || FAILED=1
if [ "$FINAL_COMMIT" != "$SOURCE_COMMIT" ] || [ "$FINAL_STATUS" != "$SOURCE_STATUS" ]; then
  skip "source consistency" "checkout changed during verification; rerun on a fixed candidate"
fi

if [ -n "$EVIDENCE" ]; then
  {
    echo "# TAIM release check"
    echo
    echo "- Commit: $SOURCE_COMMIT"
    if [ -z "$SOURCE_STATUS" ]; then echo "- Clean tree: yes"; else echo "- Clean tree: NO"; fi
    echo "- Scope: $([ "$HOST_ONLY" = 1 ] && echo host-only || echo all-platforms)"
    echo "- Date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "- Rust: $(rustc --version)"
    echo "- Host: $(uname -srm)"
    echo
    echo "| Result | Step | Time or reason |"
    echo "|---|---|---|"
    for result in "${RESULTS[@]}"; do echo "| $result |"; done
    echo
    echo '## Log SHA-256 checksums'
    echo
    for log in "$LOGS"/*.log; do
      [ -f "$log" ] && (cd "$LOGS" && shasum -a 256 "$(basename "$log")")
    done
  } >"$EVIDENCE/report.md"
  echo
  echo "Report: $EVIDENCE/report.md"
fi

echo
if [ "$FAILED" != 0 ]; then
  echo "RELEASE CHECK FAILED"
  exit 1
fi
if [ "$INCOMPLETE" != 0 ]; then
  echo "Release check passed with skipped steps: not a complete release check."
  exit 3
fi
echo "Release check passed."

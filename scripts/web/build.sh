#!/usr/bin/env bash
# Builds a TAIM example (or your own binary crate) for the browser.
#
#   bash scripts/web/build.sh [PACKAGE] [OUT_DIR] [--webgpu]
#
# PACKAGE defaults to `counter`, OUT_DIR to target/web/PACKAGE. The page
# uses WebGL2 unless --webgpu is given: WebGL2 runs in every current
# browser; WebGPU needs a browser that has it enabled. Serve OUT_DIR with
# any static file server, e.g. `python3 -m http.server -d OUT_DIR`.
#
# Needs the wasm32-unknown-unknown target and the wasm-bindgen CLI at the
# same version as the wasm-bindgen crate in Cargo.lock.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

PACKAGE="${1:-counter}"
OUT="${2:-target/web/$PACKAGE}"
FEATURES="taim/webgl"
if [ "${3:-}" = "--webgpu" ]; then FEATURES=""; fi

rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true

want="$(grep -A1 '^name = "wasm-bindgen"$' Cargo.lock | sed -n 's/^version = "\(.*\)"/\1/p' | head -n1)"
have="$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)"
if [ "$have" != "$want" ]; then
  echo "wasm-bindgen CLI $want is needed (found: ${have:-none}):" >&2
  echo "  cargo install wasm-bindgen-cli --version $want --locked" >&2
  exit 2
fi

cargo build --locked --release -p "$PACKAGE" --target wasm32-unknown-unknown \
  ${FEATURES:+--features "$FEATURES"}

TARGET_DIR="$(cargo metadata --locked --no-deps --format-version 1 |
  "${TAIM_QA_PYTHON:-python3}" -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
mkdir -p "$OUT"
wasm-bindgen --target web --no-typescript --out-dir "$OUT" \
  "$TARGET_DIR/wasm32-unknown-unknown/release/$PACKAGE.wasm"

cat >"$OUT/index.html" <<HTML
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>$PACKAGE</title>
<style>html,body{margin:0;height:100%;overflow:hidden}#taim{width:100%;height:100%}</style>
</head>
<body>
<div id="taim"></div>
<script type="module">
import init from "./$PACKAGE.js";
// winit leaves its event loop by throwing; that is not an error.
init().catch((e) => {
  if (!String(e).includes("Using exceptions for control flow")) console.error(e);
});
</script>
</body>
</html>
HTML

echo "Built $OUT — serve it with: python3 -m http.server -d $OUT 8000"

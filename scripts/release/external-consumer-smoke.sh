#!/usr/bin/env bash
# Compile TAIM outside its workspace to detect hidden workspace/path assumptions.
# It is a smoke test, not proof of a stable published package.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TEMP="$(mktemp -d)"
trap 'rm -rf "$TEMP"' EXIT

mkdir -p "$TEMP/src"
cat > "$TEMP/Cargo.toml" <<EOF
[package]
name = "taim_external_consumer_smoke"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
taim = { path = "$ROOT" }
EOF

cp "$ROOT/examples/trivial/src/main.rs" "$TEMP/src/main.rs"

echo "Checking public TAIM API in isolated temporary Cargo workspace"
cargo check --manifest-path "$TEMP/Cargo.toml"
echo "External consumer smoke passed (local path dependency; no publication implied)"

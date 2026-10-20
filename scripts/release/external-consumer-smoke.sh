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

echo "Checking legacy API in isolated temporary Cargo workspace"
cargo check --manifest-path "$TEMP/Cargo.toml"

# Exercise the new native application interface as an actual external
# downstream crate. This would catch facade exports and trait-bound regressions
# that cannot be seen from a test inside taim_program.
cat > "$TEMP/src/main.rs" <<'RUST'
use taim::{AppContext, Element, Task, WindowContext};
use taim::native::{app, Dispatch};

fn scene<'a>(count: &'a u64, _window: &WindowContext) -> Element<'a, u64> {
    taim::widget::text(format!("TAIM native: {count}")).into()
}

fn main() -> taim::Result {
    let application = app(
        "Native consumer",
        |_context: &mut AppContext| (0_u64, Task::none()),
        |state: &mut u64, amount: u64, _context: &mut AppContext| {
            if amount == 0 {
                Dispatch::idle()
            } else {
                *state += amount;
                Dispatch::paint()
            }
        },
        scene,
    );
    taim::run_app(application)
}
RUST

echo "Checking TAIM-native API without legacy ProgramAdapter"
cargo check --manifest-path "$TEMP/Cargo.toml"
echo "Both external consumer contracts compiled (path dependency; not publication or hardware proof)"

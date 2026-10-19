# TAIM lineage and implementation checkpoint: 20 October 2026

## Objective and release verdict

**Goal:** a genuinely TAIM-owned active implementation with independent
source provenance, compatibility, safety and performance. **Status: NO-GO**,
neither 100% independently authored nor production-certified.

Baseline Iced fork commit: `56b706fcd35aa881baede77a696bbb842773fdc6`.
Latest candidate before this report: `ccec9397a75bfe87ddbab6c7a026df0332d1afde`.
Complete GitHub recursive trees returned `truncated: false`.

### Narrow *exact same-path blob* comparison

- Earlier checkpoint: 55 identical `.rs/.wgsl/.toml` paths.
- Current candidate: **46 identical paths**.
- Nine exact matches disappeared after source-level changes to:
  `core/src/time.rs`, `core/src/mouse/button.rs`,
  `core/src/keyboard/location.rs`, `core/src/shadow.rs`,
  `core/src/window/mode.rs`, `core/src/window/direction.rs`,
  `core/src/window/settings/linux.rs`,
  `widget/src/pane_grid/pane.rs`, `widget/src/pane_grid/split.rs`.
- Two separate Winit shell conversions consume the new window contracts.

Do not interpret modified paths as independent authorship or treat
`(original - byte-identical) / original` as originality. This is a
**lower bound** on original-fork code still in the current tree. Identical
config and tiny module facade files are materially different issues from
copying executable widget or renderer algorithms. Manual provenance and
whole-source token/lineage review remain mandatory.

## Newly implemented TAIM-specific behavior

1. Monotonic scheduling duration factories have saturating minute/hour/day
   conversion rather than `u64` wraparound; a bounded scheduling helper
   is available to consumers.
2. Pointer buttons expose physical navigation intent without treating
   vendor-specific codes as Back/Forward. Keyboard locations distinguish
   physical modifier sides from numpad keys.
3. Shadow blur can be normalized against NaN, infinities and negative
   input; opacity culling is conservative.
4. Window mode determines fullscreen and visibility once in TAIM core,
   reused by real Winit conversion.
5. Window-edge intent exposes discrete screen-space resize direction.
6. Pane and Split expose typed local workspace IDs without exposing
   the underlying allocator or confusing their identities.
7. Linux `PlatformSpecific::valid_application_id` validates bounded
   ASCII XDG app identities. Both X11 and Wayland Winit attribute paths
   skip malformed/absent IDs instead of sending them to the OS.

A total of **12 unit tests** were added across these areas, but they
are **committed and NOT EXECUTED**. Source code review and Rust compile
are still required.

## Exact identical paths still needing review (46)

- `.cargo/audit.toml`
- `.cargo/config.toml`
- `clippy.toml`
- `core/src/keyboard.rs`
- `core/src/mouse.rs`
- `core/src/window/level.rs`
- `core/src/window/settings/macos.rs`
- `core/src/window/settings/other.rs`
- `core/src/window/settings/windows.rs`
- `core/src/window/user_attention.rs`
- `futures/src/backend/default.rs`
- `futures/src/backend/native.rs`
- `futures/src/backend/native/smol.rs`
- `futures/src/backend/native/thread_pool.rs`
- `futures/src/backend/native/tokio.rs`
- `futures/src/backend/null.rs`
- `futures/src/backend/wasm.rs`
- `futures/src/backend/wasm/wasm_bindgen.rs`
- `futures/src/executor.rs`
- `futures/src/maybe.rs`
- `graphics/src/error.rs`
- `graphics/src/geometry.rs`
- `graphics/src/geometry/fill.rs`
- `graphics/src/geometry/style.rs`
- `program/src/message.rs`
- `program/src/preset.rs`
- `runtime/src/backend.rs`
- `runtime/src/font.rs`
- `runtime/src/image.rs`
- `runtime/src/system.rs`
- `src/advanced.rs`
- `src/error.rs`
- `src/touch.rs`
- `src/window.rs`
- `src/window/icon.rs`
- `tiny_skia/src/window.rs`
- `wgpu/src/image/null.rs`
- `wgpu/src/shader/blit.wgsl`
- `wgpu/src/shader/color.wgsl`
- `wgpu/src/shader/color/linear_rgb.wgsl`
- `wgpu/src/shader/vertex.wgsl`
- `widget/src/action.rs`
- `widget/src/lazy/cache.rs`
- `widget/src/overlay.rs`
- `widget/src/pane_grid/draggable.rs`
- `winit/src/error.rs`

## Outstanding source-independence work beyond those 46 exact paths

- Reconcile edited Iced-derived widget, input, renderer, futures, platform,
  and public API implementations, including moved/renamed fragments.
- Retain historical original MIT LICENSE, CREDITS.md and full Git ancestry.
  Attribution is mandatory even after full code replacement.
- Prove intended TAIM-native execution path through behavioral tests,
  not cosmetic renaming or matching file names.

## Independent release gates

- Real Rust compilation, Clippy and workspace tests on clean isolated
  Linux/Wayland, Windows, macOS and supported target matrix.
- Shader compilation, visual golden and device-loss recovery checks.
- Real hardware keyboard, touch, multilingual IME, AT-SPI and compositor
  presentation acceptance for claimed targets.
- Same-hardware Iced/Slint/egui performance with raw trace integrity.
- Source-origin, security, dependency-license and reproducible-package
  review, signed release and owner acceptance.
- GitHub Actions runner issue #2 prevents trustworthy hosted execution
  at the time of authoring.

### Commands to execute after runner/toolchain becomes available

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
python3 scripts/audit/source-lineage-gate.py --strict --output /tmp/taim-lineage.json
```

The strict lineage gate is expected to return NO-GO with surviving matches.
**Do not merge to main or declare 100% TAIM based on this batch.**

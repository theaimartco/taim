# TAIM native window icon and overlay safety checkpoint

Date: 20 October 2026
Implementation commit: `2ba2b47fa5c20ff4f1375e9558239d7ac4061f47`
Branch: `feature/taim-bounded-icon-decoder-20261020`
Original Iced fork baseline: `81afda72f374386036a5b96a55f57a3dcfb0b653`
Release verdict: **NO-GO** — tests and original source conformance not verified

## Functional changes

### Bounded native window icon ingestion

`src/window/icon.rs` formerly used the image decoder directly on a
path or embedded bytes. Malformed/compressed images could declare
gigantic decoded dimensions and cause excessive allocations.

The new implementation:
- Reads files through one `File` handle with a hard `8 MiB + 1` read
  bound. Byte arrays from `from_file_data` also have an 8 MiB cap.
- Guesses format from content (or uses a caller-supplied explicit
  format) rather than blindly trusting an extension.
- Parses `ImageReader::into_dimensions()` before invoking full decode,
  rejects zero edge lengths, >2048-pixel edges, or >4,194,304 pixels.
- Gives the image 0.25 decoder additional strict per-axis limits
  and a **best-effort** 64 MiB allocation limit; some image codecs
  cannot strictly enforce allocation limits.
- Verifies decoded image dimensions again and passes an owned
  RGBA vector directly to the native icon constructor.
- Retains existing public `from_file`, `from_file_data`, and
  `Error` interfaces, extending errors with typed budget failures.

Changing the maximum accepted icon size is a **deliberate behavior
change** that needs compatibility testing with customer assets.

### TAIM viewport-aware overlay placement

`widget/src/overlay.rs` adds a pure,
allocation-free `fit_menu_anchor` function that validates finite
viewport/menu/pointer coordinates, clamps to visible bounds,
and shifts right-edge menus left instead of clipping them to
zero-width layout.

The real `widget/src/overlay/menu/overlay_runtime.rs` layout
path calls this policy before deriving available-space limits.
Invalid target heights are also normalized. Normal in-bounds
popup anchors remain unchanged.

## Unit tests added, not executed

- Icon: encoded-size and dimension budget boundaries.
- Icon: PNG encode/decode roundtrip and oversized PNG header rejection.
- Overlay: normal and right-edge menu placement.
- Overlay: malformed and out-of-bounds popup geometry.

**Four added test functions are committed, not executed.**
No screenshot-based dropdown visual verification or real-device
platform acceptance has run in this environment.

## Full exact same-path Git blob inventory

The complete recursive Git-tree comparisons with fork baseline and
candidate `097c468426c8e700904133c68baf8575ef213dda`
returned `truncated:false`, finding **31** matching same-path Git
blobs with extensions `.rs/.wgsl/.toml`, down from 33.

Two formerly exact files now have behavioral implementations:
`src/window/icon.rs` and `widget/src/overlay.rs`.

This count is **not a TAIM-source percentage**. It misses edited
and relocated Iced-derived code; full manual/token provenance review
and retention of historical MIT LICENSE and CREDITS.md are required.

### 31 exact surviving files

- `.cargo/audit.toml`
- `.cargo/config.toml`
- `clippy.toml`
- `core/src/keyboard.rs`
- `core/src/window/level.rs`
- `core/src/window/settings/macos.rs`
- `core/src/window/settings/other.rs`
- `core/src/window/user_attention.rs`
- `futures/src/backend/native.rs`
- `futures/src/backend/wasm.rs`
- `futures/src/executor.rs`
- `futures/src/maybe.rs`
- `graphics/src/error.rs`
- `graphics/src/geometry.rs`
- `program/src/message.rs`
- `program/src/preset.rs`
- `runtime/src/backend.rs`
- `runtime/src/image.rs`
- `runtime/src/system.rs`
- `src/advanced.rs`
- `src/error.rs`
- `src/touch.rs`
- `src/window.rs`
- `tiny_skia/src/window.rs`
- `wgpu/src/image/null.rs`
- `wgpu/src/shader/blit.wgsl`
- `wgpu/src/shader/color.wgsl`
- `wgpu/src/shader/color/linear_rgb.wgsl`
- `wgpu/src/shader/vertex.wgsl`
- `widget/src/lazy/cache.rs`
- `winit/src/error.rs`

## Blocking production verification

- Rust/Cargo and rustfmt are unavailable in the execution environment;
  no successful cargo check, Clippy, test or package validation.
- GitHub Actions runner-allocation issue #2 remains open.
- Feature-matrix tests (image enabled/disabled) and Windows/macOS/Linux
  native icon integration tests are pending.
- Real VYOM/Wayland dropdown popup behavior, keyboard/IME, screen reader,
  GPU reset/recovery and performance baselines are unverified.
- Complete source-origin analysis and license/legal review remain open.
- Prior stacked feature branches/draft PRs have not been merged.

To verify in a clean checkout:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
python3 scripts/audit/source-lineage-gate.py --strict --output /tmp/taim-lineage.json
```

Expected strict lineage verdict: FAIL while inherited implementations remain.
Do not merge to main, publish a production tag, or claim 100%
independent TAIM source or performance without independent verification.

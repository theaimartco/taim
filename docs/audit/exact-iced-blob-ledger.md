# TAIM current Iced-origin exact blob inventory

Snapshot of complete recursive GitHub trees at the Iced fork `56b706fc` and TAIM SHA `7edf9417eec20f9c3ba726aaf6bfe235381d0c13`, taken October 20, 2026.

**65 same-path byte-identical .rs/.wgsl/.toml files remain.** Full-tree API responses were not truncated. This is a **lower bound** on inherited origin: edited, partially copied, relocated-source snippets, and any upstream-origin files with changed names still need manual provenance verification and the normalized-token scanner. Not a 100% original-ownership metric.

## Input and core — 19

- `core/src/background.rs`
- `core/src/keyboard.rs`
- `core/src/keyboard/location.rs`
- `core/src/mouse.rs`
- `core/src/mouse/button.rs`
- `core/src/mouse/interaction.rs`
- `core/src/rotation.rs`
- `core/src/shadow.rs`
- `core/src/time.rs`
- `core/src/window/direction.rs`
- `core/src/window/event.rs`
- `core/src/window/level.rs`
- `core/src/window/mode.rs`
- `core/src/window/position.rs`
- `core/src/window/settings/linux.rs`
- `core/src/window/settings/macos.rs`
- `core/src/window/settings/other.rs`
- `core/src/window/settings/windows.rs`
- `core/src/window/user_attention.rs`

## Async and scheduling — 11

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
- `futures/src/stream.rs`

## Graphics — 5

- `graphics/src/error.rs`
- `graphics/src/geometry.rs`
- `graphics/src/geometry/fill.rs`
- `graphics/src/geometry/style.rs`
- `graphics/src/shell.rs`

## Application and runtime — 6

- `program/src/message.rs`
- `program/src/preset.rs`
- `runtime/src/backend.rs`
- `runtime/src/font.rs`
- `runtime/src/image.rs`
- `runtime/src/system.rs`

## Widgets — 8

- `widget/src/action.rs`
- `widget/src/lazy/cache.rs`
- `widget/src/overlay.rs`
- `widget/src/pane_grid/controls.rs`
- `widget/src/pane_grid/direction.rs`
- `widget/src/pane_grid/draggable.rs`
- `widget/src/pane_grid/pane.rs`
- `widget/src/pane_grid/split.rs`

## Rendering and shell — 8

- `tiny_skia/src/window.rs`
- `wgpu/src/image/atlas/entry.rs`
- `wgpu/src/image/null.rs`
- `wgpu/src/shader/blit.wgsl`
- `wgpu/src/shader/color.wgsl`
- `wgpu/src/shader/color/linear_rgb.wgsl`
- `wgpu/src/shader/vertex.wgsl`
- `winit/src/error.rs`

## Other — 8

- `.cargo/audit.toml`
- `.cargo/config.toml`
- `clippy.toml`
- `src/advanced.rs`
- `src/error.rs`
- `src/touch.rs`
- `src/window.rs`
- `src/window/icon.rs`

## Material implementation replacements on the native identity branch

- Inherited subscription tracker replaced by TAIM's cancellation-aware stream reconciler (tests for deduplication, retire, epoch rollover)
- Process-unique non-wrapping window IDs, with concurrent issuance and exhaustion tests
- Frame redraw intent now supports explicit merge and deadlines, used by Winit's real scheduler
- Original native App intent model and FrameGate layout/paint/animation invalidation policy
- Recursive pane-grid geometry replaced with packed-index iterative region planner, including nested/mixed-axis tests
- Native pane configuration now exposes bounded node/depth validation and checked State mounting; malformed split ratios rejected before state allocation

## Production and authenticity gates

`python3 scripts/audit/source-lineage-gate.py --strict --output artifacts/lineage.json` is required for any claim of zero inherited source. `scripts/audit/exact-fork-blobs.py` covers identical files moved to another path. **Neither scanner can prove original human authorship or extinguish MIT copyright notice requirements.**

New Rust source **has not passed an executable CI run**: GitHub Actions runner allocation issue #2 still causes jobs to fail with zero executed steps. Native VYOM/Wayland, GPU recovery, IME, screen reader, hardware latency, performance and security/reproducibility gates remain open. Preserve `LICENSE`, `CREDITS.md` and historical fork Git ancestry. Keep main unchanged and this PR draft until independent acceptance.

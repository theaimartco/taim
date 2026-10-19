# Exact Git blob identity against inherited Iced commit

This report is generated from GitHub's complete recursive tree for upstream fork commit `56b706fc` and branch commit `935e88d1d92ec1b84e628aa330243a6d4eee9e22`. No source similarity heuristic is used here: an identical SHA at the **same path** means identical file bytes.

**Exact identical .rs/.wgsl/.toml files: 67.** Tree reads were complete (not truncated). This is a lower bound on copied origin: renamed/copied files, code moved to new files, source with small changes, historical source lineage and third-party license obligations are not resolved by this count. Do not report 100% source independence based on blob count alone. This report is a pinned snapshot and must be recomputed after later commits.

## core (20)

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
- `core/src/window/redraw_request.rs`
- `core/src/window/settings/linux.rs`
- `core/src/window/settings/macos.rs`
- `core/src/window/settings/other.rs`
- `core/src/window/settings/windows.rs`
- `core/src/window/user_attention.rs`

## futures (11)

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

## graphics (5)

- `graphics/src/error.rs`
- `graphics/src/geometry.rs`
- `graphics/src/geometry/fill.rs`
- `graphics/src/geometry/style.rs`
- `graphics/src/shell.rs`

## program (2)

- `program/src/message.rs`
- `program/src/preset.rs`

## runtime (4)

- `runtime/src/backend.rs`
- `runtime/src/font.rs`
- `runtime/src/image.rs`
- `runtime/src/system.rs`

## widgets (9)

- `widget/src/action.rs`
- `widget/src/lazy/cache.rs`
- `widget/src/overlay.rs`
- `widget/src/pane_grid/configuration.rs`
- `widget/src/pane_grid/controls.rs`
- `widget/src/pane_grid/direction.rs`
- `widget/src/pane_grid/draggable.rs`
- `widget/src/pane_grid/pane.rs`
- `widget/src/pane_grid/split.rs`

## renderer (7)

- `tiny_skia/src/window.rs`
- `wgpu/src/image/atlas/entry.rs`
- `wgpu/src/image/null.rs`
- `wgpu/src/shader/blit.wgsl`
- `wgpu/src/shader/color.wgsl`
- `wgpu/src/shader/color/linear_rgb.wgsl`
- `wgpu/src/shader/vertex.wgsl`

## shell (1)

- `winit/src/error.rs`

## others (8)

- `.cargo/audit.toml`
- `.cargo/config.toml`
- `clippy.toml`
- `src/advanced.rs`
- `src/error.rs`
- `src/touch.rs`
- `src/window.rs`
- `src/window/icon.rs`

## Real replacements in this PR

- Inherited `futures/src/subscription/tracker.rs` replaced by separately authored and wired `taim_tracker.rs` with streaming cancellation, dedup and epoch tests
- `core/src/window/id.rs` reimplemented with checked monotonic atomic IDs and concurrency tests
- A separate native app/intent API with real Winit FrameGate dirty-state support and behavior tests

## Promotion controls

- `scripts/audit/source-lineage-gate.py --strict` must pass before any zero-inherited-source marketing
- Manual origin review remains mandatory after a machine audit; true public distinction requires a novel API and meaningful architecture, not rebranding
- `CREDITS.md`, inherited MIT license and Git history remain intact
- Production gate also requires executable CI, VYOM device and independent licensing/QA signoff

# Current TAIM source-identity ledger

Exact candidate baseline: upstream Iced fork `56b706fc`, TAIM branch SHA `f4b19d7845b2fbf88ddf9a1b1e3f8749e743e9b2`. GitHub full recursive Git tree reads returned untruncated inventories.

## Blocking exact inherited files: 66

Git blob SHA identity at the same path is hard evidence of identical bytes. Modified inherited source, near-matches, and relocated snippets are additional unverified scope; **the remaining source-origin count is not limited to these files**.

### Core types and input — 19

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

### Futures and executors — 11

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

### Graphics — 5

- `graphics/src/error.rs`
- `graphics/src/geometry.rs`
- `graphics/src/geometry/fill.rs`
- `graphics/src/geometry/style.rs`
- `graphics/src/shell.rs`

### Program/application — 2

- `program/src/message.rs`
- `program/src/preset.rs`

### Runtime — 4

- `runtime/src/backend.rs`
- `runtime/src/font.rs`
- `runtime/src/image.rs`
- `runtime/src/system.rs`

### Widgets — 9

- `widget/src/action.rs`
- `widget/src/lazy/cache.rs`
- `widget/src/overlay.rs`
- `widget/src/pane_grid/configuration.rs`
- `widget/src/pane_grid/controls.rs`
- `widget/src/pane_grid/direction.rs`
- `widget/src/pane_grid/draggable.rs`
- `widget/src/pane_grid/pane.rs`
- `widget/src/pane_grid/split.rs`

### GPU/CPU renderer — 7

- `tiny_skia/src/window.rs`
- `wgpu/src/image/atlas/entry.rs`
- `wgpu/src/image/null.rs`
- `wgpu/src/shader/blit.wgsl`
- `wgpu/src/shader/color.wgsl`
- `wgpu/src/shader/color/linear_rgb.wgsl`
- `wgpu/src/shader/vertex.wgsl`

### Window shell — 1

- `winit/src/error.rs`

### Configuration and facade — 8

- `.cargo/audit.toml`
- `.cargo/config.toml`
- `clippy.toml`
- `src/advanced.rs`
- `src/error.rs`
- `src/touch.rs`
- `src/window.rs`
- `src/window/icon.rs`

## Non-cosmetic replacements implemented on this branch

- `futures/src/subscription/tracker.rs` removed and replaced with independently implemented cancellation-aware subscription reconciler, `taim_tracker.rs` (tests added)
- `core/src/window/id.rs` replaced with atomic no-wrap allocation (multi-thread and exhaustion tests)
- `core/src/window/redraw_request.rs` replaced with frame-intent/deadline merge semantics wired into Winit (ordering tests)
- `widget/src/pane_grid/node/regions.rs` rewritten to use iterative packed-index, no-recursion pane geometry planning (mixed-axis, invalid bounds and deep nesting tests)
- Native `App` and explicit dirty-state path integrated in real Winit event loop

## Evidence limitations and required release gates

- This SHA inventory uses a byte-exact comparison; it is not a legal originality verdict.
- `scripts/audit/source-lineage-gate.py --strict --output /tmp/taim-audit.json` rejects unresolved inherited source, including token similarity. `exact-fork-blobs.py` checks exact relocated Git blob matches.
- External source reviewer must independently assess altered but inherited source, license records, CI/goldens/AT-SPI and real VYOM hardware acceptance.
- GitHub Actions runner allocation [issue #2](https://github.com/theaimartco/taim/issues/2) remains blocking; latest executed steps = zero. No Rust pass or runtime speedup has been proven for this patch.
- Iced MIT attribution and historical Git ancestry must remain intact, irrespective of current-source independence.

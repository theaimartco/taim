# TAIM source-lineage exact Git SHA inventory

Latest source commit: `681f172d46cc01d6a5dc8f533121f4ceeab5e683`. Comparison Iced fork baseline `56b706fc`. Both recursive GitHub trees were complete (not truncated).

## 63 exact same-path inherited Rust / WGSL / TOML files remain

This lower-bound inventory only catches byte-identical files at the original paths. Renamed or moved identical files are checked separately by scripts/audit/exact-fork-blobs.py. Modified, partially reused, generated or adapted inherited code still requires independent source review; a zero exact count would NOT certify original authorship.

### Core — 19

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

### Async — 11

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

### App and runtime — 6

- `program/src/message.rs`
- `program/src/preset.rs`
- `runtime/src/backend.rs`
- `runtime/src/font.rs`
- `runtime/src/image.rs`
- `runtime/src/system.rs`

### Widget — 7

- `widget/src/action.rs`
- `widget/src/lazy/cache.rs`
- `widget/src/overlay.rs`
- `widget/src/pane_grid/direction.rs`
- `widget/src/pane_grid/draggable.rs`
- `widget/src/pane_grid/pane.rs`
- `widget/src/pane_grid/split.rs`

### GPU and shell — 7

- `tiny_skia/src/window.rs`
- `wgpu/src/image/null.rs`
- `wgpu/src/shader/blit.wgsl`
- `wgpu/src/shader/color.wgsl`
- `wgpu/src/shader/color/linear_rgb.wgsl`
- `wgpu/src/shader/vertex.wgsl`
- `winit/src/error.rs`

### Other — 8

- `.cargo/audit.toml`
- `.cargo/config.toml`
- `clippy.toml`
- `src/advanced.rs`
- `src/error.rs`
- `src/touch.rs`
- `src/window.rs`
- `src/window/icon.rs`

## Substantive replacements on the native identity branch

- Direct TAIM native intent/reducer/scene application contract; explicit frame dirty-state semantics wired to Winit event loop.
- Original cancellation-aware, bounded subscription tracker, replacing the prior inherited implementation.
- Checked monotonic window-ID allocation and explicit frame request/deadline merging.
- Packed, iterative geometry planner for deeply nested pane-grid regions, plus checked configuration validation.
- Native GPU atlas entry preflight with linear-time exact row-major source coverage. The actual image upload rejects malformed tiling before GPU staging; tests cover overlap, missing tiles, incorrect bounds and valid tiling.
- Original TAIM adaptive pane-title control presentation (full, compact, or title-hidden) unified across layout, draw, input, accessibility visits and overlay selection. Hidden title overlays are suppressed.

## Verification and legal provenance

- Source audit strict runner and full source lineage review must complete before 100% independently implemented claims.
- GitHub Actions issue #2: actual Rust compile, Clippy, tests and hardware verification are not passing. Hosted runner failures happen with zero executed job steps; exact infrastructure root cause not yet established.
- No runtime performance speedup or correctness certification has been established for these changes; all Rust tests are **committed but currently unexecuted**.
- Historical Iced origin, MIT attribution and Git history must be preserved; originality of current code does not erase source ancestry.

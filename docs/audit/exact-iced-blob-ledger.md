# TAIM source-origin exact-SHA ledger

Candidate commit: `63171505e8be004d480c69832a01ce3e12773b20`. Original Iced fork point: `56b706fc`. Both recursive GitHub Git trees were complete, not truncated.

## 62 byte-identical active source and config files still at inherited paths

These SHA matches cover only exactly identical `.rs`, `.wgsl` and `.toml` files at the same paths. This count is a strict lower bound, **not** the actual number of inherited Rust lines: adapted or partially copied files also require review. The `exact-fork-blobs.py` audit checks renamed whole-file copies; token similarity and independent manual classification are separate mandatory gates. No code-origin scanner establishes legal authorship merely by returning zero matches.

### Core / input (18)

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
- `core/src/window/level.rs`
- `core/src/window/mode.rs`
- `core/src/window/position.rs`
- `core/src/window/settings/linux.rs`
- `core/src/window/settings/macos.rs`
- `core/src/window/settings/other.rs`
- `core/src/window/settings/windows.rs`
- `core/src/window/user_attention.rs`

### Async / scheduling (11)

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

### Graphics (5)

- `graphics/src/error.rs`
- `graphics/src/geometry.rs`
- `graphics/src/geometry/fill.rs`
- `graphics/src/geometry/style.rs`
- `graphics/src/shell.rs`

### Program & runtime (6)

- `program/src/message.rs`
- `program/src/preset.rs`
- `runtime/src/backend.rs`
- `runtime/src/font.rs`
- `runtime/src/image.rs`
- `runtime/src/system.rs`

### Widgets (7)

- `widget/src/action.rs`
- `widget/src/lazy/cache.rs`
- `widget/src/overlay.rs`
- `widget/src/pane_grid/direction.rs`
- `widget/src/pane_grid/draggable.rs`
- `widget/src/pane_grid/pane.rs`
- `widget/src/pane_grid/split.rs`

### Rendering / shell (7)

- `tiny_skia/src/window.rs`
- `wgpu/src/image/null.rs`
- `wgpu/src/shader/blit.wgsl`
- `wgpu/src/shader/color.wgsl`
- `wgpu/src/shader/color/linear_rgb.wgsl`
- `wgpu/src/shader/vertex.wgsl`
- `winit/src/error.rs`

### Configuration / facade (8)

- `.cargo/audit.toml`
- `.cargo/config.toml`
- `clippy.toml`
- `src/advanced.rs`
- `src/error.rs`
- `src/touch.rs`
- `src/window.rs`
- `src/window/icon.rs`

## Functional TAIM-native implementations integrated on the feature branch

- Native app intent/reducer/scene contract and real Winit FrameGate invalidation with clean/paint/layout/animation distinctions.
- Replacement subscription tracker with epoch-aware reconciliation, bounded event delivery and cancellation during output backpressure.
- Non-wrapping atomic window IDs, original redraw/deadline merging, packed iterative pane geometry, bounded pane configuration validation.
- GPU atlas fragmented image coverage preflight, adaptive pane-title control visibility and hidden-title overlay suppression.
- **New native event queue coalescing:** immediately adjacent same-window cursor movement, same-finger touch movement, and matching window geometry changes are folded to the latest observation. Critical mouse/touch/keyboard/focus/window lifecycle boundaries, including wheel deltas, are never folded. Tests include multiple windows, gesture boundaries, finger isolation and geometry updates. `core/src/window/event.rs` provides the typed lifecycle coalescing contract.

## Release blockers

- `scripts/audit/source-lineage-gate.py --strict` must pass on the final commit with historical fork ancestry available. All inherited and modified source needs independent human provenance review.
- GitHub Actions issue #2: jobs continue failing before a runner executes any steps, so Rust compilation, Clippy, golden-image, keyboard/IME, screen-reader, real VYOM and GPU hardware tests **have not been proven green**. Do not treat committed unit tests as executed tests.
- Source speedups are unverified. Benchmark against Iced/Slint/egui under controlled hardware before performance claims.
- Preserve actual Iced MIT notices, CREDITS.md and historical Git ancestry regardless of remaining source rewrite work. Keep main unchanged and PR draft.

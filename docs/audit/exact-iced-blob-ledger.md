# TAIM current exact Git SHA source lineage

**Candidate:** `46d7d0bbf1f79053a09f82e6c493e92a513205fa`. **Iced fork point:** `6db55104`.
Both GitHub recursive Git-tree inventories were untruncated. These are exact SHA matches at inherited paths for .rs / .wgsl / .toml files.

## 60 byte-identical files still remaining

This is an **undercount of all inherited implementation**: token-identical renamed functions, moved snippets and edited Iced-derived code still require external provenance review. Do not equate a zero exact count with independent authorship, legal permission to erase notices or stable quality.

### Core and input (17)

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

### Futures, async and scheduling (11)

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

### Graphics (4)

- `graphics/src/error.rs`
- `graphics/src/geometry.rs`
- `graphics/src/geometry/fill.rs`
- `graphics/src/geometry/style.rs`

### Application/runtime (6)

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

### Rendering and window platform (7)

- `tiny_skia/src/window.rs`
- `wgpu/src/image/null.rs`
- `wgpu/src/shader/blit.wgsl`
- `wgpu/src/shader/color.wgsl`
- `wgpu/src/shader/color/linear_rgb.wgsl`
- `wgpu/src/shader/vertex.wgsl`
- `winit/src/error.rs`

### Configuration and facade (8)

- `.cargo/audit.toml`
- `.cargo/config.toml`
- `clippy.toml`
- `src/advanced.rs`
- `src/error.rs`
- `src/touch.rs`
- `src/window.rs`
- `src/window/icon.rs`

## Native TAIM engineering added on the identity branch

- Intent-driven boot/reduce/scene native app with actual Winit FrameGate semantics; old Program compatibility retained.
- Cancellation-aware stream reconciliation, process-unique checked window IDs, redraw deadline merging, bounded and iterative pane-grid layout, source tile coverage guards for GPU atlas uploads.
- Responsive pane title controls with consistent input/accessibility/overlay visibility, and same-pointer adjacent event coalescing in real Winit queue.
- **TAIM batched renderer notification transaction:** one deferred layout/paint/tick notification per kind within each scope, compatibility with immediate notifier implementations retained; four unit tests added.
- **TAIM transparent-surface paint culling:** conservative fully transparent background classification, finite bounded opacity scaling, and actual widget container short-circuit before GPU quad submission; four unit tests added.
- Local Ubuntu verification script `scripts/release/verify-native-identity-local.sh`: verifies tracked source against candidate commit, writes exact source lineage evidence, then Rust fmt, workspace compilation, Clippy, tests and independent downstream Cargo app smoke. It deliberately fails 100%-original-source promotion until inherited code has been replaced and audited.

## Gates currently not passed

The user-requested 100% original live source is **NOT achieved** while 60 exact inherited files plus modified inherited code remain.
The new Rust code, functionality and benchmarks have **not been compiled or tested here**. GitHub Actions workflow jobs are still failing with zero executed steps ([issue #2](https://github.com/theaimartco/taim/issues/2)), so source code is unverified. A local machine with Rust is required to execute the candidate verification script. No production signoff, real VYOM/Wayland/IME/AT-SPI/GPU acceptance, performance claim, or independent licensing review is complete.

Preserve Iced's MIT LICENSE, CREDITS.md and historical Git ancestry even if all active source code is independently replaced. Keep PR draft and main unchanged.

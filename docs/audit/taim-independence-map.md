# TAIM Independence & Architectural Audit Map

> Baseline Fork Point: `81afda72` (iced upstream)
> Current HEAD: `c88d0605`
> Date: 2026-09-13

## 1. Executive Summary

- **Total files audited**: 728
- **Byte-identical / Unmodified Iced files (>=99.9%)**: 137
- **Substantially inherited Iced files (70.0% – 99.9%)**: 134
- **Moderately modified files (20.0% – 70.0%)**: 22
- **Taim-authored / Diverged files (<20.0%)**: 435

### Planned Action Breakdown
- **KEEP**: 432 files
- **REARCHITECT**: 198 files
- **OPTIMIZE**: 74 files
- **REPLACE**: 24 files

### Rewrite Priority Breakdown
- **P0 (Immediate Ground-Up Replacement / Hot-Path Divergence)**: 43 files
- **P1 (Core Structural / Render / Platform Rearchitecture)**: 181 files
- **P2 (Widget Suite Invalidation & State Separation)**: 62 files
- **P3 (Framework Utilities, Manifests & Auxiliary)**: 10 files
- **NONE (Already Taim Original / To Keep Unmodified)**: 432 files

---

## 2. File-by-File Audit Table

| Path | Original Iced Path | Similarity | Architectural Role | Hot-Path | Priority | Benchmark Affected | Action |
|---|---|---:|---|---|---|---|---|
| `core/src/window/direction.rs` | `core/src/window/direction.rs` | 100.0% | Window state, settings, and events | HIGH | P0 | `idle_shell, damage_churn` | **REARCHITECT** |
| `core/src/window/event.rs` | `core/src/window/event.rs` | 100.0% | Window state, settings, and events | HIGH | P0 | `idle_shell, damage_churn` | **REARCHITECT** |
| `core/src/window/icon.rs` | `core/src/window/icon.rs` | 100.0% | Window state, settings, and events | HIGH | P0 | `idle_shell, damage_churn` | **REARCHITECT** |
| `core/src/window/id.rs` | `core/src/window/id.rs` | 100.0% | Window state, settings, and events | HIGH | P0 | `idle_shell, damage_churn` | **REARCHITECT** |
| `core/src/window/level.rs` | `core/src/window/level.rs` | 100.0% | Window state, settings, and events | HIGH | P0 | `idle_shell, damage_churn` | **REARCHITECT** |
| `core/src/window/mode.rs` | `core/src/window/mode.rs` | 100.0% | Window state, settings, and events | HIGH | P0 | `idle_shell, damage_churn` | **REARCHITECT** |
| `core/src/window/position.rs` | `core/src/window/position.rs` | 100.0% | Window state, settings, and events | HIGH | P0 | `idle_shell, damage_churn` | **REARCHITECT** |
| `core/src/window/redraw_request.rs` | `core/src/window/redraw_request.rs` | 100.0% | Window state, settings, and events | HIGH | P0 | `idle_shell, damage_churn` | **REARCHITECT** |
| `core/src/window/screenshot.rs` | `core/src/window/screenshot.rs` | 100.0% | Window state, settings, and events | HIGH | P0 | `idle_shell, damage_churn` | **REARCHITECT** |
| `core/src/window/settings.rs` | `core/src/window/settings.rs` | 100.0% | Window state, settings, and events | HIGH | P0 | `idle_shell, damage_churn` | **REARCHITECT** |
| `core/src/window/settings/linux.rs` | `core/src/window/settings/linux.rs` | 100.0% | Window state, settings, and events | HIGH | P0 | `idle_shell, damage_churn` | **REARCHITECT** |
| `core/src/window/settings/macos.rs` | `core/src/window/settings/macos.rs` | 100.0% | Window state, settings, and events | HIGH | P0 | `idle_shell, damage_churn` | **REARCHITECT** |
| `core/src/window/settings/other.rs` | `core/src/window/settings/other.rs` | 100.0% | Window state, settings, and events | HIGH | P0 | `idle_shell, damage_churn` | **REARCHITECT** |
| `core/src/window/settings/windows.rs` | `core/src/window/settings/windows.rs` | 100.0% | Window state, settings, and events | HIGH | P0 | `idle_shell, damage_churn` | **REARCHITECT** |
| `core/src/window/user_attention.rs` | `core/src/window/user_attention.rs` | 100.0% | Window state, settings, and events | HIGH | P0 | `idle_shell, damage_churn` | **REARCHITECT** |
| `futures/src/backend/default.rs` | `futures/src/backend/default.rs` | 100.0% | Async executor abstraction (tokio/smol/thread-pool) | MEDIUM | P0 | `idle_shell` | **REPLACE** |
| `futures/src/backend/native.rs` | `futures/src/backend/native.rs` | 100.0% | Async executor abstraction (tokio/smol/thread-pool) | MEDIUM | P0 | `idle_shell` | **REPLACE** |
| `futures/src/backend/native/smol.rs` | `futures/src/backend/native/smol.rs` | 100.0% | Async executor abstraction (tokio/smol/thread-pool) | MEDIUM | P0 | `idle_shell` | **REPLACE** |
| `futures/src/backend/native/thread_pool.rs` | `futures/src/backend/native/thread_pool.rs` | 100.0% | Async executor abstraction (tokio/smol/thread-pool) | MEDIUM | P0 | `idle_shell` | **REPLACE** |
| `futures/src/backend/native/tokio.rs` | `futures/src/backend/native/tokio.rs` | 100.0% | Async executor abstraction (tokio/smol/thread-pool) | MEDIUM | P0 | `idle_shell` | **REPLACE** |
| `futures/src/backend/null.rs` | `futures/src/backend/null.rs` | 100.0% | Async executor abstraction (tokio/smol/thread-pool) | MEDIUM | P0 | `idle_shell` | **REPLACE** |
| `futures/src/backend/wasm.rs` | `futures/src/backend/wasm.rs` | 100.0% | Async executor abstraction (tokio/smol/thread-pool) | MEDIUM | P0 | `idle_shell` | **REPLACE** |
| `futures/src/backend/wasm/wasm_bindgen.rs` | `futures/src/backend/wasm/wasm_bindgen.rs` | 100.0% | Async executor abstraction (tokio/smol/thread-pool) | MEDIUM | P0 | `idle_shell` | **REPLACE** |
| `futures/src/subscription/tracker.rs` | `futures/src/subscription/tracker.rs` | 100.0% | Subscription stream lifecycle & diffing | HIGH | P0 | `idle_shell, animated_cards` | **REPLACE** |
| `program/src/message.rs` | `program/src/message.rs` | 100.0% | Application runtime contract & Program trait | HIGH | P0 | `idle_shell, cold_start` | **REPLACE** |
| `program/src/preset.rs` | `program/src/preset.rs` | 100.0% | Application runtime contract & Program trait | HIGH | P0 | `idle_shell, cold_start` | **REPLACE** |
| `src/window/icon.rs` | `src/window/icon.rs` | 100.0% | Window state, settings, and events | HIGH | P0 | `idle_shell, damage_churn` | **REARCHITECT** |
| `wgpu/src/image/atlas/allocation.rs` | `wgpu/src/image/atlas/allocation.rs` | 100.0% | GPU texture atlas allocator & eviction | CRITICAL | P0 | `image_grid, text_heavy` | **REPLACE** |
| `wgpu/src/image/atlas/allocator.rs` | `wgpu/src/image/atlas/allocator.rs` | 100.0% | GPU texture atlas allocator & eviction | CRITICAL | P0 | `image_grid, text_heavy` | **REPLACE** |
| `wgpu/src/image/atlas/entry.rs` | `wgpu/src/image/atlas/entry.rs` | 100.0% | GPU texture atlas allocator & eviction | CRITICAL | P0 | `image_grid, text_heavy` | **REPLACE** |
| `wgpu/src/image/atlas/layer.rs` | `wgpu/src/image/atlas/layer.rs` | 100.0% | GPU texture atlas allocator & eviction | CRITICAL | P0 | `image_grid, text_heavy` | **REPLACE** |
| `wgpu/src/shader/color/linear_rgb.wgsl` | `wgpu/src/shader/color/linear_rgb.wgsl` | 100.0% | WGSL pipeline shaders for quads & geometry | CRITICAL | P0 | `wgpu, effect_ladder, blur_layers` | **REARCHITECT** |
| `wgpu/src/shader/triangle/gradient.wgsl` | `wgpu/src/shader/triangle/gradient.wgsl` | 100.0% | WGSL pipeline shaders for quads & geometry | CRITICAL | P0 | `wgpu, effect_ladder, blur_layers` | **REARCHITECT** |
| `wgpu/src/shader/triangle/solid.wgsl` | `wgpu/src/shader/triangle/solid.wgsl` | 100.0% | WGSL pipeline shaders for quads & geometry | CRITICAL | P0 | `wgpu, effect_ladder, blur_layers` | **REARCHITECT** |
| `widget/src/canvas/program.rs` | `widget/src/canvas/program.rs` | 100.0% | Canvas 2D retained drawing context & commands | HIGH | P0 | `effect_ladder, wgpu` | **REPLACE** |
| `widget/src/image/viewer.rs` | `widget/src/image/viewer.rs` | 100.0% | Image widget & texture loading | HIGH | P0 | `image_grid` | **REPLACE** |
| `widget/src/shader/program.rs` | `widget/src/shader/program.rs` | 100.0% | Custom shader widget & pipeline dispatch | HIGH | P0 | `wgpu, effect_ladder` | **REPLACE** |
| `widget/src/text/rich.rs` | `widget/src/text/rich.rs` | 100.0% | Text layout, shaped runs & selection | CRITICAL | P0 | `text_heavy, text_edit` | **REPLACE** |
| `winit/src/window/state.rs` | `winit/src/window/state.rs` | 100.0% | Window event handling & winit window bridge | HIGH | P0 | `idle_shell, damage_churn` | **REARCHITECT** |
| `program/src/lib.rs` | `program/src/lib.rs` | 99.0% | Application runtime contract & Program trait | HIGH | P0 | `idle_shell, cold_start` | **REPLACE** |
| `core/src/window/settings/wasm.rs` | `core/src/window/settings/wasm.rs` | 90.5% | Window state, settings, and events | HIGH | P0 | `idle_shell, damage_churn` | **REARCHITECT** |
| `program/Cargo.toml` | `program/Cargo.toml` | 82.6% | Application runtime contract & Program trait | HIGH | P0 | `idle_shell, cold_start` | **REPLACE** |
| `core/src/theme/palette.rs` | `core/src/theme/palette.rs` | 63.5% | Legacy theme & palette definitions | MEDIUM | P0 | `all` | **REPLACE** |
| `core/src/alignment.rs` | `core/src/alignment.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/angle.rs` | `core/src/angle.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/background.rs` | `core/src/background.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/clipboard.rs` | `core/src/clipboard.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/content_fit.rs` | `core/src/content_fit.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/font.rs` | `core/src/font.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/gradient.rs` | `core/src/gradient.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/keyboard.rs` | `core/src/keyboard.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/keyboard/location.rs` | `core/src/keyboard/location.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/keyboard/modifiers.rs` | `core/src/keyboard/modifiers.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/mouse.rs` | `core/src/mouse.rs` | 100.0% | Core data types, input, geometry, and SceneArena | CRITICAL | P1 | `ten_thousand_rows, damage_churn` | **REARCHITECT** |
| `core/src/mouse/button.rs` | `core/src/mouse/button.rs` | 100.0% | Core data types, input, geometry, and SceneArena | CRITICAL | P1 | `ten_thousand_rows, damage_churn` | **REARCHITECT** |
| `core/src/mouse/click.rs` | `core/src/mouse/click.rs` | 100.0% | Core data types, input, geometry, and SceneArena | CRITICAL | P1 | `ten_thousand_rows, damage_churn` | **REARCHITECT** |
| `core/src/mouse/cursor.rs` | `core/src/mouse/cursor.rs` | 100.0% | Core data types, input, geometry, and SceneArena | CRITICAL | P1 | `ten_thousand_rows, damage_churn` | **REARCHITECT** |
| `core/src/mouse/interaction.rs` | `core/src/mouse/interaction.rs` | 100.0% | Core data types, input, geometry, and SceneArena | CRITICAL | P1 | `ten_thousand_rows, damage_churn` | **REARCHITECT** |
| `core/src/overlay/group.rs` | `core/src/overlay/group.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/pixels.rs` | `core/src/pixels.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/point.rs` | `core/src/point.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/rectangle.rs` | `core/src/rectangle.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/rotation.rs` | `core/src/rotation.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/shadow.rs` | `core/src/shadow.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/size.rs` | `core/src/size.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/svg.rs` | `core/src/svg.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/text/editor.rs` | `core/src/text/editor.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/text/highlighter.rs` | `core/src/text/highlighter.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/time.rs` | `core/src/time.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/transformation.rs` | `core/src/transformation.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/vector.rs` | `core/src/vector.rs` | 100.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/widget/operation.rs` | `core/src/widget/operation.rs` | 100.0% | Core data types, input, geometry, and SceneArena | CRITICAL | P1 | `ten_thousand_rows, damage_churn` | **REARCHITECT** |
| `core/src/widget/operation/focusable.rs` | `core/src/widget/operation/focusable.rs` | 100.0% | Core data types, input, geometry, and SceneArena | CRITICAL | P1 | `ten_thousand_rows, damage_churn` | **REARCHITECT** |
| `core/src/widget/operation/text_input.rs` | `core/src/widget/operation/text_input.rs` | 100.0% | Core data types, input, geometry, and SceneArena | CRITICAL | P1 | `ten_thousand_rows, damage_churn` | **REARCHITECT** |
| `futures/src/event.rs` | `futures/src/event.rs` | 100.0% | Task scheduling, channels, and async utilities | HIGH | P1 | `idle_shell` | **REARCHITECT** |
| `futures/src/executor.rs` | `futures/src/executor.rs` | 100.0% | Task scheduling, channels, and async utilities | HIGH | P1 | `idle_shell` | **REARCHITECT** |
| `futures/src/keyboard.rs` | `futures/src/keyboard.rs` | 100.0% | Task scheduling, channels, and async utilities | HIGH | P1 | `idle_shell` | **REARCHITECT** |
| `futures/src/maybe.rs` | `futures/src/maybe.rs` | 100.0% | Task scheduling, channels, and async utilities | HIGH | P1 | `idle_shell` | **REARCHITECT** |
| `futures/src/runtime.rs` | `futures/src/runtime.rs` | 100.0% | Task scheduling, channels, and async utilities | HIGH | P1 | `idle_shell` | **REARCHITECT** |
| `futures/src/stream.rs` | `futures/src/stream.rs` | 100.0% | Task scheduling, channels, and async utilities | HIGH | P1 | `idle_shell` | **REARCHITECT** |
| `graphics/src/error.rs` | `graphics/src/error.rs` | 100.0% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `graphics/src/geometry.rs` | `graphics/src/geometry.rs` | 100.0% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `graphics/src/geometry/cache.rs` | `graphics/src/geometry/cache.rs` | 100.0% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `graphics/src/geometry/fill.rs` | `graphics/src/geometry/fill.rs` | 100.0% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `graphics/src/geometry/frame.rs` | `graphics/src/geometry/frame.rs` | 100.0% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `graphics/src/geometry/path.rs` | `graphics/src/geometry/path.rs` | 100.0% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `graphics/src/geometry/path/builder.rs` | `graphics/src/geometry/path/builder.rs` | 100.0% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `graphics/src/geometry/style.rs` | `graphics/src/geometry/style.rs` | 100.0% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `graphics/src/geometry/text.rs` | `graphics/src/geometry/text.rs` | 100.0% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `graphics/src/shell.rs` | `graphics/src/shell.rs` | 100.0% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `graphics/src/text/editor.rs` | `graphics/src/text/editor.rs` | 100.0% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `graphics/src/text/paragraph.rs` | `graphics/src/text/paragraph.rs` | 100.0% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `graphics/src/viewport.rs` | `graphics/src/viewport.rs` | 100.0% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `runtime/src/backend.rs` | `runtime/src/backend.rs` | 100.0% | UI reconciliation, scheduling, hit testing | CRITICAL | P1 | `idle_shell, virtual_list, damage_churn` | **REARCHITECT** |
| `runtime/src/clipboard.rs` | `runtime/src/clipboard.rs` | 100.0% | UI reconciliation, scheduling, hit testing | CRITICAL | P1 | `idle_shell, virtual_list, damage_churn` | **REARCHITECT** |
| `runtime/src/font.rs` | `runtime/src/font.rs` | 100.0% | UI reconciliation, scheduling, hit testing | CRITICAL | P1 | `idle_shell, virtual_list, damage_churn` | **REARCHITECT** |
| `runtime/src/image.rs` | `runtime/src/image.rs` | 100.0% | UI reconciliation, scheduling, hit testing | CRITICAL | P1 | `idle_shell, virtual_list, damage_churn` | **REARCHITECT** |
| `runtime/src/system.rs` | `runtime/src/system.rs` | 100.0% | UI reconciliation, scheduling, hit testing | CRITICAL | P1 | `idle_shell, virtual_list, damage_churn` | **REARCHITECT** |
| `runtime/src/widget/operation.rs` | `runtime/src/widget/operation.rs` | 100.0% | UI reconciliation, scheduling, hit testing | CRITICAL | P1 | `idle_shell, virtual_list, damage_churn` | **REARCHITECT** |
| `runtime/src/window.rs` | `runtime/src/window.rs` | 100.0% | UI reconciliation, scheduling, hit testing | CRITICAL | P1 | `idle_shell, virtual_list, damage_churn` | **REARCHITECT** |
| `src/advanced.rs` | `src/advanced.rs` | 100.0% | Taim top-level umbrella API & facades | MEDIUM | P1 | `idle_shell` | **REARCHITECT** |
| `src/error.rs` | `src/error.rs` | 100.0% | Taim top-level umbrella API & facades | MEDIUM | P1 | `idle_shell` | **REARCHITECT** |
| `src/touch.rs` | `src/touch.rs` | 100.0% | Taim top-level umbrella API & facades | MEDIUM | P1 | `idle_shell` | **REARCHITECT** |
| `src/window.rs` | `src/window.rs` | 100.0% | Taim top-level umbrella API & facades | MEDIUM | P1 | `idle_shell` | **REARCHITECT** |
| `tiny_skia/src/window.rs` | `tiny_skia/src/window.rs` | 100.0% | Software CPU rasterizer fallback | CRITICAL | P1 | `cpu` | **REARCHITECT** |
| `wgpu/src/buffer.rs` | `wgpu/src/buffer.rs` | 100.0% | wgpu backend compositing & pipeline management | HIGH | P1 | `wgpu` | **OPTIMIZE** |
| `wgpu/src/geometry.rs` | `wgpu/src/geometry.rs` | 100.0% | wgpu backend compositing & pipeline management | HIGH | P1 | `wgpu` | **OPTIMIZE** |
| `wgpu/src/image/cache.rs` | `wgpu/src/image/cache.rs` | 100.0% | wgpu backend compositing & pipeline management | HIGH | P1 | `wgpu` | **OPTIMIZE** |
| `wgpu/src/image/null.rs` | `wgpu/src/image/null.rs` | 100.0% | wgpu backend compositing & pipeline management | HIGH | P1 | `wgpu` | **OPTIMIZE** |
| `wgpu/src/image/raster.rs` | `wgpu/src/image/raster.rs` | 100.0% | wgpu backend compositing & pipeline management | HIGH | P1 | `wgpu` | **OPTIMIZE** |
| `wgpu/src/image/vector.rs` | `wgpu/src/image/vector.rs` | 100.0% | wgpu backend compositing & pipeline management | HIGH | P1 | `wgpu` | **OPTIMIZE** |
| `wgpu/src/primitive.rs` | `wgpu/src/primitive.rs` | 100.0% | wgpu backend compositing & pipeline management | HIGH | P1 | `wgpu` | **OPTIMIZE** |
| `wgpu/src/shader/blit.wgsl` | `wgpu/src/shader/blit.wgsl` | 100.0% | WGSL shaders for GPU raster pass | CRITICAL | P1 | `wgpu, blur_layers` | **OPTIMIZE** |
| `wgpu/src/shader/color.wgsl` | `wgpu/src/shader/color.wgsl` | 100.0% | WGSL shaders for GPU raster pass | CRITICAL | P1 | `wgpu, blur_layers` | **OPTIMIZE** |
| `wgpu/src/shader/image.wgsl` | `wgpu/src/shader/image.wgsl` | 100.0% | WGSL shaders for GPU raster pass | CRITICAL | P1 | `wgpu, blur_layers` | **OPTIMIZE** |
| `wgpu/src/shader/triangle.wgsl` | `wgpu/src/shader/triangle.wgsl` | 100.0% | WGSL shaders for GPU raster pass | CRITICAL | P1 | `wgpu, blur_layers` | **OPTIMIZE** |
| `wgpu/src/shader/vertex.wgsl` | `wgpu/src/shader/vertex.wgsl` | 100.0% | WGSL shaders for GPU raster pass | CRITICAL | P1 | `wgpu, blur_layers` | **OPTIMIZE** |
| `winit/src/clipboard.rs` | `winit/src/clipboard.rs` | 100.0% | Platform event loop & winit shell normalization | CRITICAL | P1 | `idle_shell, text_edit, multitouch` | **REARCHITECT** |
| `winit/src/error.rs` | `winit/src/error.rs` | 100.0% | Platform event loop & winit shell normalization | CRITICAL | P1 | `idle_shell, text_edit, multitouch` | **REARCHITECT** |
| `winit/src/proxy.rs` | `winit/src/proxy.rs` | 100.0% | Platform event loop & winit shell normalization | CRITICAL | P1 | `idle_shell, text_edit, multitouch` | **REARCHITECT** |
| `core/src/keyboard/key.rs` | `core/src/keyboard/key.rs` | 99.9% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `runtime/src/task.rs` | `runtime/src/task.rs` | 99.8% | UI reconciliation, scheduling, hit testing | CRITICAL | P1 | `idle_shell, virtual_list, damage_churn` | **REARCHITECT** |
| `core/src/image.rs` | `core/src/image.rs` | 99.7% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/renderer/null.rs` | `core/src/renderer/null.rs` | 99.6% | Core data types, input, geometry, and SceneArena | CRITICAL | P1 | `ten_thousand_rows, damage_churn` | **REARCHITECT** |
| `core/src/text.rs` | `core/src/text.rs` | 99.3% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `wgpu/src/image/atlas.rs` | `wgpu/src/image/atlas.rs` | 99.2% | wgpu backend compositing & pipeline management | HIGH | P1 | `wgpu` | **OPTIMIZE** |
| `core/src/backend.rs` | `core/src/backend.rs` | 99.2% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/padding.rs` | `core/src/padding.rs` | 99.2% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/layout.rs` | `core/src/layout.rs` | 99.0% | Layout solver & node tree constraints | CRITICAL | P1 | `ten_thousand_rows, virtual_list, text_heavy` | **REARCHITECT** |
| `graphics/src/geometry/stroke.rs` | `graphics/src/geometry/stroke.rs` | 99.0% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `core/src/element.rs` | `core/src/element.rs` | 98.9% | Core data types, input, geometry, and SceneArena | CRITICAL | P1 | `ten_thousand_rows, damage_churn` | **REARCHITECT** |
| `wgpu/src/image/mod.rs` | `wgpu/src/image/mod.rs` | 98.6% | wgpu backend compositing & pipeline management | HIGH | P1 | `wgpu` | **OPTIMIZE** |
| `wgpu/src/text.rs` | `wgpu/src/text.rs` | 98.6% | wgpu GPU primitive pipelines, batching & layer passes | CRITICAL | P1 | `wgpu, ten_thousand_rows, effect_ladder` | **REARCHITECT** |
| `tiny_skia/src/geometry.rs` | `tiny_skia/src/geometry.rs` | 98.4% | Software CPU rasterizer fallback | CRITICAL | P1 | `cpu` | **REARCHITECT** |
| `core/src/keyboard/event.rs` | `core/src/keyboard/event.rs` | 98.2% | Core data types, input, geometry, and SceneArena | CRITICAL | P1 | `ten_thousand_rows, damage_churn` | **REARCHITECT** |
| `core/src/mouse/event.rs` | `core/src/mouse/event.rs` | 98.2% | Core data types, input, geometry, and SceneArena | CRITICAL | P1 | `ten_thousand_rows, damage_churn` | **REARCHITECT** |
| `core/src/border.rs` | `core/src/border.rs` | 98.2% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `wgpu/src/triangle.rs` | `wgpu/src/triangle.rs` | 98.2% | wgpu GPU primitive pipelines, batching & layer passes | CRITICAL | P1 | `wgpu, ten_thousand_rows, effect_ladder` | **REARCHITECT** |
| `core/src/window.rs` | `core/src/window.rs` | 98.2% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `graphics/src/geometry/path/arc.rs` | `graphics/src/geometry/path/arc.rs` | 97.6% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `graphics/src/image.rs` | `graphics/src/image.rs` | 97.1% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `src/application/timed.rs` | `src/application/timed.rs` | 97.0% | Taim top-level umbrella API & facades | MEDIUM | P1 | `idle_shell` | **REARCHITECT** |
| `graphics/src/image/storage.rs` | `graphics/src/image/storage.rs` | 96.8% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `wgpu/src/triangle/msaa.rs` | `wgpu/src/triangle/msaa.rs` | 96.5% | wgpu GPU primitive pipelines, batching & layer passes | CRITICAL | P1 | `wgpu, ten_thousand_rows, effect_ladder` | **REARCHITECT** |
| `core/src/overlay/element.rs` | `core/src/overlay/element.rs` | 96.5% | Core data types, input, geometry, and SceneArena | CRITICAL | P1 | `ten_thousand_rows, damage_churn` | **REARCHITECT** |
| `tiny_skia/src/raster.rs` | `tiny_skia/src/raster.rs` | 96.5% | Software CPU rasterizer fallback | CRITICAL | P1 | `cpu` | **REARCHITECT** |
| `tiny_skia/src/text.rs` | `tiny_skia/src/text.rs` | 96.5% | Software CPU rasterizer fallback | CRITICAL | P1 | `cpu` | **REARCHITECT** |
| `tiny_skia/src/layer.rs` | `tiny_skia/src/layer.rs` | 96.3% | Software CPU rasterizer fallback | CRITICAL | P1 | `cpu` | **REARCHITECT** |
| `futures/src/lib.rs` | `futures/src/lib.rs` | 96.1% | Task scheduling, channels, and async utilities | HIGH | P1 | `idle_shell` | **REARCHITECT** |
| `graphics/src/gradient.rs` | `graphics/src/gradient.rs` | 96.0% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `winit/src/conversion.rs` | `winit/src/conversion.rs` | 95.9% | Platform event loop & winit shell normalization | CRITICAL | P1 | `idle_shell, text_edit, multitouch` | **REARCHITECT** |
| `graphics/src/antialiasing.rs` | `graphics/src/antialiasing.rs` | 95.8% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `src/time.rs` | `src/time.rs` | 95.7% | Taim top-level umbrella API & facades | MEDIUM | P1 | `idle_shell` | **REARCHITECT** |
| `futures/src/subscription.rs` | `futures/src/subscription.rs` | 95.5% | Task scheduling, channels, and async utilities | HIGH | P1 | `idle_shell` | **REARCHITECT** |
| `src/daemon.rs` | `src/daemon.rs` | 95.2% | Taim top-level umbrella API & facades | MEDIUM | P1 | `idle_shell` | **REARCHITECT** |
| `core/src/widget/text.rs` | `core/src/widget/text.rs` | 95.2% | Core data types, input, geometry, and SceneArena | CRITICAL | P1 | `ten_thousand_rows, damage_churn` | **REARCHITECT** |
| `graphics/src/text.rs` | `graphics/src/text.rs` | 95.0% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `core/src/settings.rs` | `core/src/settings.rs` | 94.7% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `wgpu/src/color.rs` | `wgpu/src/color.rs` | 94.2% | wgpu backend compositing & pipeline management | HIGH | P1 | `wgpu` | **OPTIMIZE** |
| `winit/src/lib.rs` | `winit/src/lib.rs` | 94.0% | Platform event loop & winit shell normalization | CRITICAL | P1 | `idle_shell, text_edit, multitouch` | **REARCHITECT** |
| `tiny_skia/src/vector.rs` | `tiny_skia/src/vector.rs` | 94.0% | Software CPU rasterizer fallback | CRITICAL | P1 | `cpu` | **REARCHITECT** |
| `benches/wgpu.rs` | `benches/wgpu.rs` | 93.7% | Legacy microbenchmarks | LOW | P1 | `wgpu, cpu` | **REPLACE** |
| `benches/cpu.rs` | `benches/cpu.rs` | 93.7% | Legacy microbenchmarks | LOW | P1 | `wgpu, cpu` | **REPLACE** |
| `winit/src/window.rs` | `winit/src/window.rs` | 93.6% | Platform event loop & winit shell normalization | CRITICAL | P1 | `idle_shell, text_edit, multitouch` | **REARCHITECT** |
| `wgpu/src/quad/solid.rs` | `wgpu/src/quad/solid.rs` | 93.5% | wgpu GPU primitive pipelines, batching & layer passes | CRITICAL | P1 | `wgpu, ten_thousand_rows, effect_ladder` | **REARCHITECT** |
| `graphics/src/mesh.rs` | `graphics/src/mesh.rs` | 93.2% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `futures/Cargo.toml` | `futures/Cargo.toml` | 92.3% | Task scheduling, channels, and async utilities | HIGH | P1 | `idle_shell` | **REARCHITECT** |
| `core/src/length.rs` | `core/src/length.rs` | 92.1% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/lib.rs` | `core/src/lib.rs` | 92.1% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `wgpu/src/quad/gradient.rs` | `wgpu/src/quad/gradient.rs` | 91.0% | wgpu GPU primitive pipelines, batching & layer passes | CRITICAL | P1 | `wgpu, ten_thousand_rows, effect_ladder` | **REARCHITECT** |
| `renderer/src/fallback.rs` | `renderer/src/fallback.rs` | 90.9% | Renderer contract & backend dispatcher | CRITICAL | P1 | `all` | **REARCHITECT** |
| `wgpu/src/window.rs` | `wgpu/src/window.rs` | 90.9% | wgpu backend compositing & pipeline management | HIGH | P1 | `wgpu` | **OPTIMIZE** |
| `tiny_skia/src/engine.rs` | `tiny_skia/src/engine.rs` | 90.6% | Software CPU rasterizer fallback | CRITICAL | P1 | `cpu` | **REARCHITECT** |
| `core/src/overlay/nested.rs` | `core/src/overlay/nested.rs` | 90.3% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/Cargo.toml` | `core/Cargo.toml` | 90.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `futures/src/backend.rs` | `futures/src/backend.rs` | 90.0% | Task scheduling, channels, and async utilities | HIGH | P1 | `idle_shell` | **REARCHITECT** |
| `src/application.rs` | `src/application.rs` | 89.3% | Taim top-level umbrella API & facades | MEDIUM | P1 | `idle_shell` | **REARCHITECT** |
| `runtime/src/lib.rs` | `runtime/src/lib.rs` | 89.2% | UI reconciliation, scheduling, hit testing | CRITICAL | P1 | `idle_shell, virtual_list, damage_churn` | **REARCHITECT** |
| `graphics/Cargo.toml` | `graphics/Cargo.toml` | 88.9% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `core/src/input_method.rs` | `core/src/input_method.rs` | 88.7% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `wgpu/src/layer.rs` | `wgpu/src/layer.rs` | 88.1% | wgpu GPU primitive pipelines, batching & layer passes | CRITICAL | P1 | `wgpu, ten_thousand_rows, effect_ladder` | **REARCHITECT** |
| `core/src/overlay.rs` | `core/src/overlay.rs` | 88.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `tiny_skia/src/primitive.rs` | `tiny_skia/src/primitive.rs` | 87.9% | Software CPU rasterizer fallback | CRITICAL | P1 | `cpu` | **REARCHITECT** |
| `wgpu/src/shader/quad/gradient.wgsl` | `wgpu/src/shader/quad/gradient.wgsl` | 87.4% | WGSL shaders for GPU raster pass | CRITICAL | P1 | `wgpu, blur_layers` | **OPTIMIZE** |
| `core/src/widget/operation/scrollable.rs` | `core/src/widget/operation/scrollable.rs` | 87.3% | Core data types, input, geometry, and SceneArena | CRITICAL | P1 | `ten_thousand_rows, damage_churn` | **REARCHITECT** |
| `src/lib.rs` | `src/lib.rs` | 86.7% | Taim top-level umbrella API & facades | MEDIUM | P1 | `idle_shell` | **REARCHITECT** |
| `graphics/src/compositor.rs` | `graphics/src/compositor.rs` | 84.6% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `core/src/layout/flex.rs` | `core/src/layout/flex.rs` | 82.5% | Layout solver & node tree constraints | CRITICAL | P1 | `ten_thousand_rows, virtual_list, text_heavy` | **REARCHITECT** |
| `tiny_skia/Cargo.toml` | `tiny_skia/Cargo.toml` | 82.1% | Software CPU rasterizer fallback | CRITICAL | P1 | `cpu` | **REARCHITECT** |
| `graphics/src/color.rs` | `graphics/src/color.rs` | 80.7% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `winit/Cargo.toml` | `winit/Cargo.toml` | 80.7% | Platform event loop & winit shell normalization | CRITICAL | P1 | `idle_shell, text_edit, multitouch` | **REARCHITECT** |
| `wgpu/Cargo.toml` | `wgpu/Cargo.toml` | 79.6% | wgpu backend compositing & pipeline management | HIGH | P1 | `wgpu` | **OPTIMIZE** |
| `core/src/text/paragraph.rs` | `core/src/text/paragraph.rs` | 79.5% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/layout/limits.rs` | `core/src/layout/limits.rs` | 78.2% | Layout solver & node tree constraints | CRITICAL | P1 | `ten_thousand_rows, virtual_list, text_heavy` | **REARCHITECT** |
| `graphics/src/lib.rs` | `graphics/src/lib.rs` | 78.2% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `graphics/src/layer.rs` | `graphics/src/layer.rs` | 77.8% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `wgpu/src/shader/quad/solid.wgsl` | `wgpu/src/shader/quad/solid.wgsl` | 76.9% | WGSL shaders for GPU raster pass | CRITICAL | P1 | `wgpu, blur_layers` | **OPTIMIZE** |
| `core/src/animation.rs` | `core/src/animation.rs` | 76.7% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `tiny_skia/src/window/compositor.rs` | `tiny_skia/src/window/compositor.rs` | 76.6% | Software CPU rasterizer fallback | CRITICAL | P1 | `cpu` | **REARCHITECT** |
| `core/src/theme.rs` | `core/src/theme.rs` | 75.2% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `core/src/widget.rs` | `core/src/widget.rs` | 72.2% | Core data types, input, geometry, and SceneArena | CRITICAL | P1 | `ten_thousand_rows, damage_churn` | **REARCHITECT** |
| `renderer/src/lib.rs` | `renderer/src/lib.rs` | 70.7% | Renderer contract & backend dispatcher | CRITICAL | P1 | `all` | **REARCHITECT** |
| `core/src/renderer.rs` | `core/src/renderer.rs` | 70.5% | Core data types, input, geometry, and SceneArena | CRITICAL | P1 | `ten_thousand_rows, damage_churn` | **REARCHITECT** |
| `core/src/shell.rs` | `core/src/shell.rs` | 69.0% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `runtime/src/user_interface.rs` | `runtime/src/user_interface.rs` | 68.6% | UI reconciliation, scheduling, hit testing | CRITICAL | P1 | `idle_shell, virtual_list, damage_churn` | **REARCHITECT** |
| `tiny_skia/src/lib.rs` | `tiny_skia/src/lib.rs` | 67.8% | Software CPU rasterizer fallback | CRITICAL | P1 | `cpu` | **REARCHITECT** |
| `wgpu/src/window/compositor.rs` | `wgpu/src/window/compositor.rs` | 65.4% | wgpu backend compositing & pipeline management | HIGH | P1 | `wgpu` | **OPTIMIZE** |
| `runtime/Cargo.toml` | `runtime/Cargo.toml` | 63.8% | UI reconciliation, scheduling, hit testing | CRITICAL | P1 | `idle_shell, virtual_list, damage_churn` | **REARCHITECT** |
| `core/src/layout/node.rs` | `core/src/layout/node.rs` | 63.6% | Layout solver & node tree constraints | CRITICAL | P1 | `ten_thousand_rows, virtual_list, text_heavy` | **REARCHITECT** |
| `wgpu/src/lib.rs` | `wgpu/src/lib.rs` | 62.4% | wgpu backend compositing & pipeline management | HIGH | P1 | `wgpu` | **OPTIMIZE** |
| `wgpu/src/quad.rs` | `wgpu/src/quad.rs` | 61.5% | wgpu GPU primitive pipelines, batching & layer passes | CRITICAL | P1 | `wgpu, ten_thousand_rows, effect_ladder` | **REARCHITECT** |
| `runtime/src/widget.rs` | `runtime/src/widget.rs` | 57.1% | UI reconciliation, scheduling, hit testing | CRITICAL | P1 | `idle_shell, virtual_list, damage_churn` | **REARCHITECT** |
| `graphics/src/cache.rs` | `graphics/src/cache.rs` | 51.9% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `runtime/src/keyboard.rs` | `runtime/src/keyboard.rs` | 50.0% | UI reconciliation, scheduling, hit testing | CRITICAL | P1 | `idle_shell, virtual_list, damage_churn` | **REARCHITECT** |
| `renderer/Cargo.toml` | `renderer/Cargo.toml` | 49.4% | Renderer contract & backend dispatcher | CRITICAL | P1 | `all` | **REARCHITECT** |
| `wgpu/src/shader/quad.wgsl` | `wgpu/src/shader/quad.wgsl` | 45.6% | WGSL shaders for GPU raster pass | CRITICAL | P1 | `wgpu, blur_layers` | **OPTIMIZE** |
| `core/src/widget/tree.rs` | `core/src/widget/tree.rs` | 42.6% | Core data types, input, geometry, and SceneArena | CRITICAL | P1 | `ten_thousand_rows, damage_churn` | **REARCHITECT** |
| `core/src/widget/id.rs` | `core/src/widget/id.rs` | 42.5% | Core data types, input, geometry, and SceneArena | CRITICAL | P1 | `ten_thousand_rows, damage_churn` | **REARCHITECT** |
| `core/src/event.rs` | `core/src/event.rs` | 32.7% | Core data types, input, geometry, and SceneArena | CRITICAL | P1 | `ten_thousand_rows, damage_churn` | **REARCHITECT** |
| `wgpu/src/engine.rs` | `wgpu/src/engine.rs` | 28.4% | wgpu GPU primitive pipelines, batching & layer passes | CRITICAL | P1 | `wgpu, ten_thousand_rows, effect_ladder` | **REARCHITECT** |
| `graphics/src/text/cache.rs` | `graphics/src/text/cache.rs` | 28.2% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `graphics/src/damage.rs` | `graphics/src/damage.rs` | 16.6% | Retained display records, damage caching, typography | CRITICAL | P1 | `damage_churn, text_heavy, image_grid` | **REARCHITECT** |
| `core/src/touch.rs` | `core/src/touch.rs` | 13.3% | Core data types, input, geometry, and SceneArena | MEDIUM | P1 | `ten_thousand_rows, damage_churn` | **OPTIMIZE** |
| `tests/layout.rs` | `tests/layout.rs` | 0.9% | Layout solver & node tree constraints | CRITICAL | P1 | `ten_thousand_rows, virtual_list, text_heavy` | **REARCHITECT** |
| `widget/src/action.rs` | `widget/src/action.rs` | 100.0% | Widget control: action | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/lazy/cache.rs` | `widget/src/lazy/cache.rs` | 100.0% | Widget control: cache | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/overlay.rs` | `widget/src/overlay.rs` | 100.0% | Widget control: overlay | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/pane_grid/axis.rs` | `widget/src/pane_grid/axis.rs` | 100.0% | Widget control: axis | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/pane_grid/configuration.rs` | `widget/src/pane_grid/configuration.rs` | 100.0% | Widget control: configuration | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/pane_grid/controls.rs` | `widget/src/pane_grid/controls.rs` | 100.0% | Widget control: controls | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/pane_grid/direction.rs` | `widget/src/pane_grid/direction.rs` | 100.0% | Widget control: direction | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/pane_grid/draggable.rs` | `widget/src/pane_grid/draggable.rs` | 100.0% | Widget control: draggable | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/pane_grid/node.rs` | `widget/src/pane_grid/node.rs` | 100.0% | Widget control: node | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/pane_grid/pane.rs` | `widget/src/pane_grid/pane.rs` | 100.0% | Widget control: pane | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/pane_grid/split.rs` | `widget/src/pane_grid/split.rs` | 100.0% | Widget control: split | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/pane_grid/state.rs` | `widget/src/pane_grid/state.rs` | 100.0% | Widget control: state | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/space.rs` | `widget/src/space.rs` | 100.0% | Widget control: space | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/text_input/cursor.rs` | `widget/src/text_input/cursor.rs` | 100.0% | Widget control: cursor | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/text_input/editor.rs` | `widget/src/text_input/editor.rs` | 100.0% | Widget control: editor | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/shader.rs` | `widget/src/shader.rs` | 99.5% | Widget control: shader | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/combo_box.rs` | `widget/src/combo_box.rs` | 99.4% | Widget control: combo_box | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/pick_list.rs` | `widget/src/pick_list.rs` | 99.4% | Widget control: pick_list | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/markdown.rs` | `widget/src/markdown.rs` | 99.1% | Widget control: markdown | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/vertical_slider.rs` | `widget/src/vertical_slider.rs` | 98.9% | Widget control: vertical_slider | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/qr_code.rs` | `widget/src/qr_code.rs` | 98.6% | Widget control: qr_code | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/pane_grid.rs` | `widget/src/pane_grid.rs` | 98.5% | Widget control: pane_grid | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/table.rs` | `widget/src/table.rs` | 98.5% | Widget control: table | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/overlay/menu.rs` | `widget/src/overlay/menu.rs` | 98.5% | Widget control: menu | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/sensor.rs` | `widget/src/sensor.rs` | 98.3% | Widget control: sensor | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/mouse_area.rs` | `widget/src/mouse_area.rs` | 98.3% | Widget control: mouse_area | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/image.rs` | `widget/src/image.rs` | 98.3% | Widget control: image | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/svg.rs` | `widget/src/svg.rs` | 98.2% | Widget control: svg | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/rule.rs` | `widget/src/rule.rs` | 98.2% | Widget control: rule | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/progress_bar.rs` | `widget/src/progress_bar.rs` | 98.2% | Widget control: progress_bar | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/text_input.rs` | `widget/src/text_input.rs` | 98.1% | Widget control: text_input | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/tooltip.rs` | `widget/src/tooltip.rs` | 97.6% | Widget control: tooltip | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/text_editor.rs` | `widget/src/text_editor.rs` | 97.4% | Widget control: text_editor | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/slider.rs` | `widget/src/slider.rs` | 97.4% | Widget control: slider | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/scrollable.rs` | `widget/src/scrollable.rs` | 97.0% | Widget control: scrollable | HIGH | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/lazy/component.rs` | `widget/src/lazy/component.rs` | 96.9% | Widget control: component | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/radio.rs` | `widget/src/radio.rs` | 96.9% | Widget control: radio | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/checkbox.rs` | `widget/src/checkbox.rs` | 96.9% | Widget control: checkbox | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/canvas.rs` | `widget/src/canvas.rs` | 96.8% | Widget control: canvas | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/responsive.rs` | `widget/src/responsive.rs` | 96.6% | Widget control: responsive | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/toggler.rs` | `widget/src/toggler.rs` | 96.5% | Widget control: toggler | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/pane_grid/content.rs` | `widget/src/pane_grid/content.rs` | 96.5% | Widget control: content | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/button.rs` | `widget/src/button.rs` | 95.7% | Widget control: button | HIGH | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/float.rs` | `widget/src/float.rs` | 95.3% | Widget control: float | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/pane_grid/title_bar.rs` | `widget/src/pane_grid/title_bar.rs` | 94.9% | Widget control: title_bar | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/transition.rs` | `widget/src/transition.rs` | 94.6% | Widget control: transition | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/container.rs` | `widget/src/container.rs` | 94.2% | Widget control: container | HIGH | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/themer.rs` | `widget/src/themer.rs` | 93.5% | Widget control: themer | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/column.rs` | `widget/src/column.rs` | 93.1% | Widget control: column | HIGH | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/keyed.rs` | `widget/src/keyed.rs` | 92.0% | Widget control: keyed | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/row.rs` | `widget/src/row.rs` | 90.9% | Widget control: row | HIGH | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/keyed/column.rs` | `widget/src/keyed/column.rs` | 90.0% | Widget control: column | HIGH | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/pin.rs` | `widget/src/pin.rs` | 89.8% | Widget control: pin | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/stack.rs` | `widget/src/stack.rs` | 88.6% | Widget control: stack | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/text_input/value.rs` | `widget/src/text_input/value.rs` | 88.1% | Widget control: value | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/helpers.rs` | `widget/src/helpers.rs` | 86.3% | Widget control: helpers | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/lazy.rs` | `widget/src/lazy.rs` | 83.7% | Widget control: lazy | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/text.rs` | `widget/src/text.rs` | 83.3% | Widget control: text | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/lib.rs` | `widget/src/lib.rs` | 82.0% | Widget control: lib | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/grid.rs` | `widget/src/grid.rs` | 75.4% | Widget control: grid | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/Cargo.toml` | `widget/Cargo.toml` | 70.3% | Widget control: Cargo.toml | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `widget/src/lazy/helpers.rs` | `widget/src/lazy/helpers.rs` | 62.8% | Widget control: helpers | MEDIUM | P2 | `ten_thousand_rows, virtual_list, text_edit` | **REARCHITECT** |
| `.cargo/audit.toml` | `.cargo/audit.toml` | 100.0% | Configuration or manifest | LOW | P3 | `None` | **OPTIMIZE** |
| `.cargo/config.toml` | `.cargo/config.toml` | 100.0% | Configuration or manifest | LOW | P3 | `None` | **OPTIMIZE** |
| `clippy.toml` | `clippy.toml` | 100.0% | Configuration or manifest | LOW | P3 | `None` | **OPTIMIZE** |
| `docs/logo-no-shadow.svg` | `docs/logo-no-shadow.svg` | 100.0% | Configuration or manifest | LOW | P3 | `None` | **OPTIMIZE** |
| `docs/logo.svg` | `docs/logo.svg` | 100.0% | Configuration or manifest | LOW | P3 | `None` | **OPTIMIZE** |
| `docs/release_summary.py` | `docs/release_summary.py` | 97.3% | Configuration or manifest | LOW | P3 | `None` | **OPTIMIZE** |
| `debug/src/lib.rs` | `debug/src/lib.rs` | 72.4% | Debug visualizer & overlay | LOW | P3 | `None` | **OPTIMIZE** |
| `Cross.toml` | `Cross.toml` | 71.4% | Configuration or manifest | LOW | P3 | `None` | **OPTIMIZE** |
| `debug/Cargo.toml` | `debug/Cargo.toml` | 69.4% | Debug visualizer & overlay | LOW | P3 | `None` | **OPTIMIZE** |
| `Cargo.toml` | `Cargo.toml` | 53.2% | Configuration or manifest | LOW | P3 | `None` | **OPTIMIZE** |
| `assets/fonts/test/README.md` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/add.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/arrow_down.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/arrow_left.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/arrow_right.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/arrow_up.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/calendar.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/check.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/chevron_down.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/chevron_left.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/chevron_right.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/chevron_up.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/clock.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/close.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/delete.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/download.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/edit.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/filter.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/folder.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/heart.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/home.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/info.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/lock.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/mail.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/manifest.toml` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/menu.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/more_horiz.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/more_vert.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/pause.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/person.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/play.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/remove.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/search.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/settings.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/star.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/upload.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/visibility.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `assets/icons/warning.svg` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `benchmarks/devices.toml` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `core/src/cache.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `core/src/color/display.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `core/src/color/mod.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `core/src/color/radiance.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `core/src/color/space.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `core/src/damage.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `core/src/frame.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `core/src/layout/cache.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `core/src/layout_solver.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `core/src/material.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `core/src/pointer.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `core/src/scene.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `core/src/security.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `core/src/semantics.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `core/src/semantics_driver.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `core/src/spatial.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `core/src/text/fallback.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `core/src/text/line_break.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `core/src/text/selection.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `core/src/widget/visit.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `deny.toml` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `examples/ide/Cargo.toml` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `examples/ide/src/activity.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `examples/ide/src/debug.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `examples/ide/src/document.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `examples/ide/src/extensions.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `examples/ide/src/git.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `examples/ide/src/lib.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `examples/ide/src/lsp.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `examples/ide/src/main.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `examples/ide/src/notifications.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `examples/ide/src/palette.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `examples/ide/src/search.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `examples/ide/src/terminal.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `examples/ide/src/tree.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `examples/ide/tests/ide_semantics.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `examples/mobile_shell/Cargo.toml` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `examples/mobile_shell/src/main.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `examples/trivial/Cargo.toml` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `examples/trivial/src/main.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `fuzz/Cargo.toml` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `fuzz/fuzz_targets/damage_region.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `fuzz/fuzz_targets/layout_tree.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `fuzz/fuzz_targets/pointer_stream.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `fuzz/fuzz_targets/text_edit.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `graphics/src/display_list.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `graphics/src/quality.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `graphics/src/retained_layer.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `runtime/src/diagnostics.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `runtime/src/frame_scheduler.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `runtime/src/hit_test.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `runtime/src/invalidation.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `runtime/src/memory_pressure.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `runtime/src/pointer_router.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `runtime/src/power_policy.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `runtime/src/quality_controller.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `runtime/src/reactive_ui.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `runtime/src/reconcile.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `runtime/src/recovery.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `runtime/src/semantics.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `rust-toolchain.toml` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `scripts/assets/build-icon-font.py` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `scripts/assets/icon_geometry.py` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `scripts/ci/check-budgets.sh` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `scripts/ci/check-workspace.sh` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `scripts/clean-target.sh` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `scripts/design/check-icons.sh` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `scripts/install-hooks.sh` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `scripts/loc.sh` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `scripts/perf/compare-baseline.sh` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `scripts/perf/compare-frameworks.sh` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `scripts/perf/measure-app.py` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `scripts/perf/run-baseline.sh` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `scripts/perf/test_measure_app.py` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `taim-accessibility/Cargo.toml` | `N/A (Taim original)` | 0.0% | Semantic accessibility tree & AT-SPI | MEDIUM | NONE | `None` | **KEEP** |
| `taim-accessibility/src/actions.rs` | `N/A (Taim original)` | 0.0% | Semantic accessibility tree & AT-SPI | MEDIUM | NONE | `None` | **KEEP** |
| `taim-accessibility/src/adapter.rs` | `N/A (Taim original)` | 0.0% | Semantic accessibility tree & AT-SPI | MEDIUM | NONE | `None` | **KEEP** |
| `taim-accessibility/src/atspi.rs` | `N/A (Taim original)` | 0.0% | Semantic accessibility tree & AT-SPI | MEDIUM | NONE | `None` | **KEEP** |
| `taim-accessibility/src/atspi/adapter.rs` | `N/A (Taim original)` | 0.0% | Semantic accessibility tree & AT-SPI | MEDIUM | NONE | `None` | **KEEP** |
| `taim-accessibility/src/atspi/bridge.rs` | `N/A (Taim original)` | 0.0% | Semantic accessibility tree & AT-SPI | MEDIUM | NONE | `None` | **KEEP** |
| `taim-accessibility/src/atspi/server.rs` | `N/A (Taim original)` | 0.0% | Semantic accessibility tree & AT-SPI | MEDIUM | NONE | `None` | **KEEP** |
| `taim-accessibility/src/atspi/store.rs` | `N/A (Taim original)` | 0.0% | Semantic accessibility tree & AT-SPI | MEDIUM | NONE | `None` | **KEEP** |
| `taim-accessibility/src/atspi/vocabulary.rs` | `N/A (Taim original)` | 0.0% | Semantic accessibility tree & AT-SPI | MEDIUM | NONE | `None` | **KEEP** |
| `taim-accessibility/src/focus.rs` | `N/A (Taim original)` | 0.0% | Semantic accessibility tree & AT-SPI | MEDIUM | NONE | `None` | **KEEP** |
| `taim-accessibility/src/lib.rs` | `N/A (Taim original)` | 0.0% | Semantic accessibility tree & AT-SPI | MEDIUM | NONE | `None` | **KEEP** |
| `taim-accessibility/src/live_region.rs` | `N/A (Taim original)` | 0.0% | Semantic accessibility tree & AT-SPI | MEDIUM | NONE | `None` | **KEEP** |
| `taim-accessibility/src/node.rs` | `N/A (Taim original)` | 0.0% | Semantic accessibility tree & AT-SPI | MEDIUM | NONE | `None` | **KEEP** |
| `taim-accessibility/src/tree.rs` | `N/A (Taim original)` | 0.0% | Semantic accessibility tree & AT-SPI | MEDIUM | NONE | `None` | **KEEP** |
| `taim-accessibility/tests/atspi_bridge.rs` | `N/A (Taim original)` | 0.0% | Semantic accessibility tree & AT-SPI | MEDIUM | NONE | `None` | **KEEP** |
| `taim-accessibility/tests/atspi_bus.rs` | `N/A (Taim original)` | 0.0% | Semantic accessibility tree & AT-SPI | MEDIUM | NONE | `None` | **KEEP** |
| `taim-design/Cargo.toml` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/compiler.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/app_bar.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/bottom_sheet.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/button.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/card.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/dialog.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/gauge.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/glass_card.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/icon_button.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/list_tile.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/menu.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/mod.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/navigation.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/notification.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/progress.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/segmented_control.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/selection.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/shimmer.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/slider.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/text_field.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/toast.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/tooltip.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/components/window_chrome.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/icon.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/lib.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/platform.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/state.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/surface.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/theme.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/tokens/color.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/tokens/elevation.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/tokens/luminance.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/tokens/material.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/tokens/mod.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/tokens/motion.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/tokens/radius.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/tokens/spacing.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-design/src/tokens/typography.rs` | `N/A (Taim original)` | 0.0% | Semantic design tokens & physical lighting widgets | MEDIUM | NONE | `animated_cards, effect_ladder` | **KEEP** |
| `taim-gesture/Cargo.toml` | `N/A (Taim original)` | 0.0% | Multi-touch gesture competition arena & velocity | HIGH | NONE | `multitouch, virtual_list` | **KEEP** |
| `taim-gesture/src/arena.rs` | `N/A (Taim original)` | 0.0% | Multi-touch gesture competition arena & velocity | HIGH | NONE | `multitouch, virtual_list` | **KEEP** |
| `taim-gesture/src/edge_swipe.rs` | `N/A (Taim original)` | 0.0% | Multi-touch gesture competition arena & velocity | HIGH | NONE | `multitouch, virtual_list` | **KEEP** |
| `taim-gesture/src/lib.rs` | `N/A (Taim original)` | 0.0% | Multi-touch gesture competition arena & velocity | HIGH | NONE | `multitouch, virtual_list` | **KEEP** |
| `taim-gesture/src/long_press.rs` | `N/A (Taim original)` | 0.0% | Multi-touch gesture competition arena & velocity | HIGH | NONE | `multitouch, virtual_list` | **KEEP** |
| `taim-gesture/src/nested_scroll.rs` | `N/A (Taim original)` | 0.0% | Multi-touch gesture competition arena & velocity | HIGH | NONE | `multitouch, virtual_list` | **KEEP** |
| `taim-gesture/src/pan.rs` | `N/A (Taim original)` | 0.0% | Multi-touch gesture competition arena & velocity | HIGH | NONE | `multitouch, virtual_list` | **KEEP** |
| `taim-gesture/src/pinch.rs` | `N/A (Taim original)` | 0.0% | Multi-touch gesture competition arena & velocity | HIGH | NONE | `multitouch, virtual_list` | **KEEP** |
| `taim-gesture/src/recognizer.rs` | `N/A (Taim original)` | 0.0% | Multi-touch gesture competition arena & velocity | HIGH | NONE | `multitouch, virtual_list` | **KEEP** |
| `taim-gesture/src/rotate.rs` | `N/A (Taim original)` | 0.0% | Multi-touch gesture competition arena & velocity | HIGH | NONE | `multitouch, virtual_list` | **KEEP** |
| `taim-gesture/src/swipe.rs` | `N/A (Taim original)` | 0.0% | Multi-touch gesture competition arena & velocity | HIGH | NONE | `multitouch, virtual_list` | **KEEP** |
| `taim-gesture/src/tap.rs` | `N/A (Taim original)` | 0.0% | Multi-touch gesture competition arena & velocity | HIGH | NONE | `multitouch, virtual_list` | **KEEP** |
| `taim-gesture/src/velocity.rs` | `N/A (Taim original)` | 0.0% | Multi-touch gesture competition arena & velocity | HIGH | NONE | `multitouch, virtual_list` | **KEEP** |
| `taim-highlighter/Cargo.toml` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `taim-highlighter/src/lib.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `taim-highlighter/src/syntax.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `taim-highlighter/src/theme.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `taim-motion/Cargo.toml` | `N/A (Taim original)` | 0.0% | Physical springs & motion physics | HIGH | NONE | `animated_cards, motion_stress` | **KEEP** |
| `taim-motion/src/clock.rs` | `N/A (Taim original)` | 0.0% | Physical springs & motion physics | HIGH | NONE | `animated_cards, motion_stress` | **KEEP** |
| `taim-motion/src/decay.rs` | `N/A (Taim original)` | 0.0% | Physical springs & motion physics | HIGH | NONE | `animated_cards, motion_stress` | **KEEP** |
| `taim-motion/src/keyframes.rs` | `N/A (Taim original)` | 0.0% | Physical springs & motion physics | HIGH | NONE | `animated_cards, motion_stress` | **KEEP** |
| `taim-motion/src/lib.rs` | `N/A (Taim original)` | 0.0% | Physical springs & motion physics | HIGH | NONE | `animated_cards, motion_stress` | **KEEP** |
| `taim-motion/src/page_transition.rs` | `N/A (Taim original)` | 0.0% | Physical springs & motion physics | HIGH | NONE | `animated_cards, motion_stress` | **KEEP** |
| `taim-motion/src/reduced_motion.rs` | `N/A (Taim original)` | 0.0% | Physical springs & motion physics | HIGH | NONE | `animated_cards, motion_stress` | **KEEP** |
| `taim-motion/src/shared_element.rs` | `N/A (Taim original)` | 0.0% | Physical springs & motion physics | HIGH | NONE | `animated_cards, motion_stress` | **KEEP** |
| `taim-motion/src/spring.rs` | `N/A (Taim original)` | 0.0% | Physical springs & motion physics | HIGH | NONE | `animated_cards, motion_stress` | **KEEP** |
| `taim-motion/src/timeline.rs` | `N/A (Taim original)` | 0.0% | Physical springs & motion physics | HIGH | NONE | `animated_cards, motion_stress` | **KEEP** |
| `taim-motion/src/transition.rs` | `N/A (Taim original)` | 0.0% | Physical springs & motion physics | HIGH | NONE | `animated_cards, motion_stress` | **KEEP** |
| `taim-motion/src/tween.rs` | `N/A (Taim original)` | 0.0% | Physical springs & motion physics | HIGH | NONE | `animated_cards, motion_stress` | **KEEP** |
| `taim-platform-vyom/Cargo.toml` | `N/A (Taim original)` | 0.0% | VYOM platform & Wayland compositor protocol | HIGH | NONE | `idle_shell` | **KEEP** |
| `taim-platform-vyom/src/accessibility.rs` | `N/A (Taim original)` | 0.0% | VYOM platform & Wayland compositor protocol | HIGH | NONE | `idle_shell` | **KEEP** |
| `taim-platform-vyom/src/buffer.rs` | `N/A (Taim original)` | 0.0% | VYOM platform & Wayland compositor protocol | HIGH | NONE | `idle_shell` | **KEEP** |
| `taim-platform-vyom/src/capabilities.rs` | `N/A (Taim original)` | 0.0% | VYOM platform & Wayland compositor protocol | HIGH | NONE | `idle_shell` | **KEEP** |
| `taim-platform-vyom/src/clipboard.rs` | `N/A (Taim original)` | 0.0% | VYOM platform & Wayland compositor protocol | HIGH | NONE | `idle_shell` | **KEEP** |
| `taim-platform-vyom/src/color_management.rs` | `N/A (Taim original)` | 0.0% | VYOM platform & Wayland compositor protocol | HIGH | NONE | `idle_shell` | **KEEP** |
| `taim-platform-vyom/src/drag_drop.rs` | `N/A (Taim original)` | 0.0% | VYOM platform & Wayland compositor protocol | HIGH | NONE | `idle_shell` | **KEEP** |
| `taim-platform-vyom/src/event_loop.rs` | `N/A (Taim original)` | 0.0% | VYOM platform & Wayland compositor protocol | HIGH | NONE | `idle_shell` | **KEEP** |
| `taim-platform-vyom/src/frame_clock.rs` | `N/A (Taim original)` | 0.0% | VYOM platform & Wayland compositor protocol | HIGH | NONE | `idle_shell` | **KEEP** |
| `taim-platform-vyom/src/gesture_bridge.rs` | `N/A (Taim original)` | 0.0% | VYOM platform & Wayland compositor protocol | HIGH | NONE | `idle_shell` | **KEEP** |
| `taim-platform-vyom/src/ime.rs` | `N/A (Taim original)` | 0.0% | VYOM platform & Wayland compositor protocol | HIGH | NONE | `idle_shell` | **KEEP** |
| `taim-platform-vyom/src/input.rs` | `N/A (Taim original)` | 0.0% | VYOM platform & Wayland compositor protocol | HIGH | NONE | `idle_shell` | **KEEP** |
| `taim-platform-vyom/src/lib.rs` | `N/A (Taim original)` | 0.0% | VYOM platform & Wayland compositor protocol | HIGH | NONE | `idle_shell` | **KEEP** |
| `taim-platform-vyom/src/power.rs` | `N/A (Taim original)` | 0.0% | VYOM platform & Wayland compositor protocol | HIGH | NONE | `idle_shell` | **KEEP** |
| `taim-platform-vyom/src/shm.rs` | `N/A (Taim original)` | 0.0% | VYOM platform & Wayland compositor protocol | HIGH | NONE | `idle_shell` | **KEEP** |
| `taim-platform-vyom/src/surface.rs` | `N/A (Taim original)` | 0.0% | VYOM platform & Wayland compositor protocol | HIGH | NONE | `idle_shell` | **KEEP** |
| `taim-platform-vyom/src/window_protocol.rs` | `N/A (Taim original)` | 0.0% | VYOM platform & Wayland compositor protocol | HIGH | NONE | `idle_shell` | **KEEP** |
| `taim-profiler/Cargo.toml` | `N/A (Taim original)` | 0.0% | Microsecond telemetry & Chrome trace exporter | LOW | NONE | `all` | **KEEP** |
| `taim-profiler/src/collector.rs` | `N/A (Taim original)` | 0.0% | Microsecond telemetry & Chrome trace exporter | LOW | NONE | `all` | **KEEP** |
| `taim-profiler/src/events.rs` | `N/A (Taim original)` | 0.0% | Microsecond telemetry & Chrome trace exporter | LOW | NONE | `all` | **KEEP** |
| `taim-profiler/src/export.rs` | `N/A (Taim original)` | 0.0% | Microsecond telemetry & Chrome trace exporter | LOW | NONE | `all` | **KEEP** |
| `taim-profiler/src/lib.rs` | `N/A (Taim original)` | 0.0% | Microsecond telemetry & Chrome trace exporter | LOW | NONE | `all` | **KEEP** |
| `taim-raster/Cargo.toml` | `N/A (Taim original)` | 0.0% | Vector compute rasterizer & Bezier subdivision | CRITICAL | NONE | `effect_ladder, blur_layers, wgpu` | **KEEP** |
| `taim-raster/shader/coarse.wgsl` | `N/A (Taim original)` | 0.0% | Vector compute rasterizer & Bezier subdivision | CRITICAL | NONE | `effect_ladder, blur_layers, wgpu` | **KEEP** |
| `taim-raster/shader/fine.wgsl` | `N/A (Taim original)` | 0.0% | Vector compute rasterizer & Bezier subdivision | CRITICAL | NONE | `effect_ladder, blur_layers, wgpu` | **KEEP** |
| `taim-raster/shader/path_coarse.wgsl` | `N/A (Taim original)` | 0.0% | Vector compute rasterizer & Bezier subdivision | CRITICAL | NONE | `effect_ladder, blur_layers, wgpu` | **KEEP** |
| `taim-raster/src/allocator.rs` | `N/A (Taim original)` | 0.0% | Vector compute rasterizer & Bezier subdivision | CRITICAL | NONE | `effect_ladder, blur_layers, wgpu` | **KEEP** |
| `taim-raster/src/lib.rs` | `N/A (Taim original)` | 0.0% | Vector compute rasterizer & Bezier subdivision | CRITICAL | NONE | `effect_ladder, blur_layers, wgpu` | **KEEP** |
| `taim-raster/tests/proof_gate.rs` | `N/A (Taim original)` | 0.0% | Vector compute rasterizer & Bezier subdivision | CRITICAL | NONE | `effect_ladder, blur_layers, wgpu` | **KEEP** |
| `taim-showcase/Cargo.toml` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `taim-showcase/src/lib.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `taim-showcase/src/main.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `taim-showcase/tests/headless_semantics.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `taim-signal/Cargo.toml` | `N/A (Taim original)` | 0.0% | Fine-grained reactive signals & dependency graph | CRITICAL | NONE | `idle_shell, animated_cards, virtual_list` | **KEEP** |
| `taim-signal/src/lib.rs` | `N/A (Taim original)` | 0.0% | Fine-grained reactive signals & dependency graph | CRITICAL | NONE | `idle_shell, animated_cards, virtual_list` | **KEEP** |
| `tests/accessibility.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/accessibility/components.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/accessibility/focus.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/accessibility/overlay.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/accessibility/policies.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/accessibility/tree.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/accessibility/user_interface.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/accessibility/widgets.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/api.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/api/core_contracts.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/bench.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/bench/report_schema.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/color.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/color/contract.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/color/output.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/design.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/design/app_bar.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/design/catalog_coverage.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/design/components.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/design/dialog.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/design/icon_button.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/design/icon_font.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/design/list_tile.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/design/tokens.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/design/widgets.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/events.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/events/hit_path.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/events/spatial_hit_test.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/events/user_interface.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/gestures.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/gestures/arena.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/gestures/nested_scroll.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/gestures/recorded_streams.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/golden.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/golden/components.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/golden/damage_motion.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/golden/damage_pixels.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/golden/design_system.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/golden/effects.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/golden/effects_gpu.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/golden/gpu_primitives.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/golden/harness.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/golden/manifest.toml` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/golden/multilingual_text.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/golden/transitions.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/golden/widget_gallery.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/gpu.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/gpu/batching.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/gpu/cache_pressure.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/gpu/color_output.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/gpu/compute_rasterizer.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/gpu/device_loss.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/gpu/display_transform.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/gpu/effect_bounds.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/gpu/effect_budget.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/gpu/material_backdrop.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/gpu/material_live.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/gpu/material_shading.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/gpu/quality_budget.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/gpu/target_formats.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/graphics.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/graphics/damage_region.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/graphics/retained_layers.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/layout/incremental.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/layout/property.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/motion.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/motion/interruption.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/motion/primitives.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/motion/spring.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/motion/stress.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/platform.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/platform/fake_compositor.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/platform/input_ime.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/platform/lifecycle.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/platform/security.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/profiler.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/profiler/overhead.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/profiler/pipeline.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/profiler/privacy.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/runtime.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/runtime/frame_scheduler.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/runtime/idle.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/runtime/occlusion.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/runtime/retained_replay_probe.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/security.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/security/malformed_inputs.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/security/protected_content.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/semantics.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/semantics/incremental.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/smoke.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/smoke/workspace.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/stress.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/stress/navigation.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/stress/resource_pressure.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/stress/window_lifecycle.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/text.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/text/corpus.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/text/editing.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/text/ime.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/text/security.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/widgets.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/widgets/dirty_flags.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/widgets/identity.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/widgets/invalidation.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/widgets/keyed_reconcile.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tests/widgets/virtual_list.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `tools/taim_bench/Cargo.toml` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/build.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/config.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/main.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/metrics/allocations.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/metrics/frame_time.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/metrics/latency.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/metrics/memory.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/metrics/mod.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/metrics/pacing.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/report.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/scenes/animated_cards.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/scenes/blur_layers.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/scenes/damage_churn.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/scenes/effect_ladder.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/scenes/idle_shell.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/scenes/image_grid.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/scenes/mod.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/scenes/motion_stress.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/scenes/multitouch.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/scenes/signal_list.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/scenes/ten_thousand_rows.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/scenes/text_edit.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/scenes/text_heavy.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_bench/src/scenes/virtual_list.rs` | `N/A (Taim original)` | 0.0% | Deterministic 13-scene benchmark harness | LOW | NONE | `all` | **KEEP** |
| `tools/taim_lab/Cargo.toml` | `N/A (Taim original)` | 0.0% | Interactive widget inspector & developer lab | LOW | NONE | `None` | **KEEP** |
| `tools/taim_lab/src/animation_inspector.rs` | `N/A (Taim original)` | 0.0% | Interactive widget inspector & developer lab | LOW | NONE | `None` | **KEEP** |
| `tools/taim_lab/src/damage_overlay.rs` | `N/A (Taim original)` | 0.0% | Interactive widget inspector & developer lab | LOW | NONE | `None` | **KEEP** |
| `tools/taim_lab/src/device_profiles.rs` | `N/A (Taim original)` | 0.0% | Interactive widget inspector & developer lab | LOW | NONE | `None` | **KEEP** |
| `tools/taim_lab/src/frame_graph.rs` | `N/A (Taim original)` | 0.0% | Interactive widget inspector & developer lab | LOW | NONE | `None` | **KEEP** |
| `tools/taim_lab/src/gpu_resources.rs` | `N/A (Taim original)` | 0.0% | Interactive widget inspector & developer lab | LOW | NONE | `None` | **KEEP** |
| `tools/taim_lab/src/layout_overlay.rs` | `N/A (Taim original)` | 0.0% | Interactive widget inspector & developer lab | LOW | NONE | `None` | **KEEP** |
| `tools/taim_lab/src/main.rs` | `N/A (Taim original)` | 0.0% | Interactive widget inspector & developer lab | LOW | NONE | `None` | **KEEP** |
| `tools/taim_lab/src/overdraw_overlay.rs` | `N/A (Taim original)` | 0.0% | Interactive widget inspector & developer lab | LOW | NONE | `None` | **KEEP** |
| `tools/taim_lab/src/semantics_inspector.rs` | `N/A (Taim original)` | 0.0% | Interactive widget inspector & developer lab | LOW | NONE | `None` | **KEEP** |
| `tools/taim_lab/src/target.rs` | `N/A (Taim original)` | 0.0% | Interactive widget inspector & developer lab | LOW | NONE | `None` | **KEEP** |
| `tools/taim_lab/src/widget_inspector.rs` | `N/A (Taim original)` | 0.0% | Interactive widget inspector & developer lab | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/batch.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/display.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/effects/backdrop_blur.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/effects/blur.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/effects/color_matrix.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/effects/mask.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/effects/mod.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/effects/noise.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/effects/pipeline.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/effects/pool.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/effects/saturation.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/effects/shadow.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/gbuffer.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/metrics.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/pipeline_cache.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/rasterizer.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/resource_budget.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/shader/display.wgsl` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/shader/effect.wgsl` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/shader/light.wgsl` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/shader/material.wgsl` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/shader/quad/backdrop.wgsl` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `wgpu/src/window/frame_feedback.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/accordion.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/adaptive.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/app_bar.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/avatar.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/badge.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/bottom_nav_bar.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/bottom_sheet.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/carousel.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/chip.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/component.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/constraint.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/date.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/date_picker.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/dialog.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/drawer.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/icon.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/icon_button.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/list_tile.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/navigator.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/pull_to_refresh.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/reactive.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/safe_area.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/scaffold.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/search_bar.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/segmented_control.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/skeleton.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/sliver.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/snackbar.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/stepper.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/tabs.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/taim.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/time_picker.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `widget/src/virtual_list.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |
| `winit/src/measurement.rs` | `N/A (Taim original)` | 0.0% | Taim-authored component or test | LOW | NONE | `None` | **KEEP** |

# TAIM runtime/renderer independence checkpoint — 20 October 2026

Candidate code commit: `3e9b4294fa381f69850b93bed38a2cc4064b60e6`
Fork-point commit: `6db551046abd02eadee9f6bbcfa103bc4a9fadd5`
Status: **NO-GO — source-independence and production acceptance incomplete**

## Observable runtime changes

1. `taim_graphics::geometry::Style::is_fully_transparent` handles both
   solid color and gradient pigment. A known all-zero-alpha,
   nonempty stop set is eligible for culling; unknown/NaN alpha,
   empty gradients and any partially opaque stop are not culled.
2. `Fill::is_fully_transparent` propagates conservative culling
   independently of winding rule.
3. The live `Frame::fill` and `Frame::fill_rectangle` entry points
   consume this policy and **avoid backend command/tessellation dispatch**
   for provably invisible paint. No GPU throughput claim is made.
4. `taim_futures::backend::null::Executor::new` now rejects a missing
   async executor with `Unsupported` instead of starting a GUI event
   loop whose commands/subscriptions are silently discarded.
   An explicitly constructed null unit executor can still drop work,
   so embedded users must not select it for actual async applications.
5. Default font size configuration normalizes negative and nonfinite
   logical pixel values before passing them into renderer metrics.
6. Tokio, smol and Wasm subscription interval builders clamp
   zero-duration periods to 1 ms to avoid timer panics and immediate
   event-loop spins. This is a minimum safety bound, NOT a promise
   of predictable 1 ms timer scheduling or low-power behavior.

## Source-lineage evidence

Two GitHub recursive Git-tree inventories were complete (`truncated=false`):
baseline `6db55104`, candidate tree
`22c83c20b6194d900f4dc4a59e6766e2140bbac0`.

| Stage | Exact same-path source/config Git blob matches |
| --- | ---: |
| Earlier source-identity checkpoint | 60 |
| First native stream/overlay integration | 58 |
| Layout/window integration | 55 |
| Input and clock integration | 46 |
| **Current runtime/render pass** | **39** |

Changed inherited exact blobs:
- `graphics/src/geometry/style.rs`
- `graphics/src/geometry/fill.rs`
- `futures/src/backend/null.rs`
- `runtime/src/font.rs`
- `futures/src/backend/native/smol.rs`
- `futures/src/backend/native/tokio.rs`
- `futures/src/backend/wasm/wasm_bindgen.rs`

The edited sources still retain historical Iced-derived expressions and
APIs. A lower exact-blob number **does not certify clean-room authorship,
licensing independence, semantic conformance, source provenance, or safety**.
Manual comparison of token-identical and renamed/moved implementations
remains mandatory. Always retain inherited MIT LICENSE, CREDITS.md and
the entire historical Git ancestry.

## Exact inherited paths that still match (39)

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
- `futures/src/backend/native/thread_pool.rs`
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

## Required tests and release gates

Eight new unit tests have been added but were **not executed**:
2 pigment, 1 fill-rule, 1 null-backend, 1 font-size, and 3 interval
tests. Real GPU-shader validation, visual golden baselines,
Wayland/VYOM input/presentation, IME/AT-SPI, device-loss recovery,
multi-platform API compatibility and real performance measurements
remain open. GitHub Actions issue #2 currently blocks hosted runners.

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
python3 scripts/audit/source-lineage-gate.py --strict --output /tmp/taim-lineage.json
```

Strict source-lineage promotion is expected to fail. Do not push to main,
publish stable/v1.0, or claim 100% source independence/benchmark
superiority before reviewed evidence supports it.

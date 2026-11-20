# TAIM widget/frame and pane-drag contract checkpoint

- Date: 20 October 2026
- Code candidate: `c7b516613d34b7a53a1e4682da07e55063fb8994`
- Iced fork baseline: `81afda72f374386036a5b96a55f57a3dcfb0b653`
- Release gate: **NO-GO**
- Branch: `feature/taim-widget-actions-hit-testing-20261020`

## Behavioral implementation

1. `widget/src/action.rs`: `Action::publish(message)` now carries
   `RedrawRequest::NextFrame`. Earlier it returned `Wait` while
   promising a redraw. Canvas and Shader widget update paths already
   consume the action's redraw request into `Shell`, so the change
   affects real event-to-render behavior.
2. `Action::and_request_redraw_at(at)` combines deadlines using the
   TAIM window scheduler's `RedrawRequest::merge` policy; an
   immediate frame cannot be downgraded into a late timer.
   Captured events retain their capture status.
3. `widget/src/pane_grid/draggable.rs`: shared drag-hit validation
   rejects empty, nonfinite or out-of-bounds layout/pointer geometry
   before dispatching to user-defined `Draggable` implementations.
4. `widget/src/pane_grid/interaction.rs`: the actual pointer interaction
   path now calls the checked `can_start_drag` entry point.

Six unexecuted unit cases check published message redraw intent,
capture preservation, deadline ordering and invalid drag pointers.
No runnable Cargo/Rust toolchain was available in this environment.

## Exact byte SHA source-lineage evidence (not originality percentage)

Recursive Git trees for Iced fork `81afda72` and new candidate
tree `d81663d69709005a3b18dda276e765a13780179d`
were complete (neither truncated). Counting exact matching Git blob
SHA1 values at the same path with `.rs`, `.wgsl`, `.toml` extensions:

| Candidate milestone | Same-path identical files |
| --- | ---: |
| Earlier identity baseline | 60 |
| Stream, overlay and layout | 55 |
| Input, scheduling and Linux | 46 |
| Geometry, executor and timer | 39 |
| **Widget actions and drag** | **37** |

The new code introduces additional TAIM behavior but remains
based on a historically Iced-derived codebase. Changed blob SHA
values do **not** prove independent source authorship. Copied or
substantially similar code elsewhere in the tree requires a
token/semantic and manual provenance audit. Keep the original
Iced MIT license, CREDITS.md and full Git ancestry.

## 37 exact identical same-path files still present

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
- `widget/src/lazy/cache.rs`
- `widget/src/overlay.rs`
- `winit/src/error.rs`

## Release blockers

- Rust `cargo check`, Clippy, fmt, tests and shader validation not run.
- GitHub Actions runner allocation issue #2 remains open.
- Real VYOM/Wayland input, multiple monitors, IME and AT-SPI
  accessibility not externally verified.
- Hardware GPU reset/recovery, signed packaging, security,
  same-device benchmark and manual source provenance are unverified.
- Additional modified/moved Iced-derived implementation remains.

Expected owner-run commands (strict source-lineage gate
intentionally refuses this candidate):

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
python3 scripts/audit/source-lineage-gate.py --strict --output /tmp/taim-audit.json
```

Do not merge to main, declare 100% original source, label production
ready, or publish v1.0 without independent passing evidence.

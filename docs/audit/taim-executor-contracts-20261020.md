# TAIM executor contract and source-lineage checkpoint

Date: 20 October 2026
Branch: `feature/taim-executor-contracts-20261020`
Candidate implementation: `8b59fd9c13f9f7782f35fe0a7c26af0638d0a499`
Baseline original Iced fork: `81afda72f374386036a5b96a55f57a3dcfb0b653`
Verdict: **NO-GO** — source independence and production not proven.

## Correctness regression found and fixed

An earlier hardening patch changed the **explicit**
`futures::backend::null::Executor::new()` constructor to return
`Unsupported`. However, `futures/src/runtime.rs` contains existing
scheduler tests that construct this null executor via `.unwrap()`,
because they exercise the independently polled `TaskScheduler`
without background executor work. The earlier patch therefore broke
those tests.

**Correction:** explicit `null::Executor` is again constructible for
headless/scheduler-only use. In `backend::default`, when no supported
native async backend feature (`tokio`, `smol`, `thread-pool`) is
enabled, the default `Executor::new()` returns
`std::io::ErrorKind::Unsupported`. This preserves fail-fast
production startup without removing the explicit no-op test backend.
Other executor feature combinations preserve the existing precedence.

## Native TAIM worker pool with task lifecycle telemetry

`futures/src/backend/native/thread_pool.rs` is now a TAIM adapter
around the upstream futures worker pool rather than a type alias.
Its `Telemetry` exposes three *approximate* atomic counters:

- `submitted`: total dispatched work since executor construction
- `active`: queued/running/pending tasks not yet dropped
- `terminated`: completed or cancelled tasks

An RAII `InFlight` guard is **created before spawning**, then captured
by the task future. This matters: constructing the guard on first poll
would leak `active` if a queued future were cancelled before it was
ever polled. The drop guard accounts for both completion and
cancellation, including never-polled future drop. The wrapper delegates
execution to futures ThreadPool; this is **not yet an independent
thread scheduler** or evidence that it is faster than Iced.

Seven added/revised tests cover explicit null constructor
compatibility, conditional default fail-fast selection, zero counter
initialization, guard drop accounting, never-polled cancellation and
end-to-end dispatched task messaging.

**These tests are committed but NOT EXECUTED** in the current environment.

## Exact path+Git SHA inventory

Both historical Iced and current candidate Git recursive trees are
complete (`truncated:false`). Counting only byte-identical blobs at
the *same path* ending in `.rs`, `.wgsl`, `.toml`:

| Candidate | Identical paths |
| --- | ---: |
| Original early TAIM identity checkpoint | 60 |
| Stream/layout native checkpoint | 55 |
| Clock/input and Linux native checkpoint | 46 |
| Rendering and executor safety checkpoint | 39 |
| Widget action and drag hit-testing checkpoint | 37 |
| **Current executor contract checkpoint** | **35** |

Changed former exact matches:
- `futures/src/backend/default.rs`
- `futures/src/backend/native/thread_pool.rs`

The explicit null backend was already modified in an earlier commit,
so its corrective edit does not change this narrow exact match count.

### All remaining exact same-path original-Iced blobs (35)

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

An exact blob count is a **lower bound on inherited source**: it omits
relocations, token-level copies, altered inherited expressions and
other derivative implementations. Merely changing identifiers/comments
is not evidence of independently authored source. Preserve full MIT
LICENSE and CREDITS.md attribution and Git ancestry.

## Outstanding verification and acceptance

1. Rust compiler / Cargo were not available in this runner; no compile,
   Clippy, fmt, target matrix or 7-test execution is certified.
2. GitHub Actions runner-allocation blocker issue #2 is still open.
3. Strict manual/token lineage checks, security/ASan-style campaigns,
   and remaining original-source rewrites are incomplete.
4. Real VYOM/Wayland frame presentation, keyboard, IME, accessibility,
   GPU recovery and same-device input-to-photon benchmarks not verified.
5. No stable API/release review, signed provenance or main-branch merge.

Required commands in a clean checkout:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
python3 scripts/audit/source-lineage-gate.py --strict --output /tmp/taim-lineage.json
```

Strict lineage is expected to reject. **Do not merge to main,
claim 100% TAIM-native independent source, or release production.**

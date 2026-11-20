# TAIM input delivery, gesture cleanup and native Windows shell

**Date:** 2026-10-20
**Code candidate commit:** `692dc7b10ac51238fd3a8d07aa21f013547a12d5`
**Historical original Iced fork:** `81afda72f374386036a5b96a55f57a3dcfb0b653`
**Release decision:** NO-GO — not independently sourced end to end and not production-certified

## Functional implementation in current candidate

1. **Scroll-delta loss prevention.** `core::mouse::Event::can_replace_previous_without_loss`
   identifies only finite cursor position samples as replaceable queue entries.
   WheelScrolled carries an additive scroll delta, so its public core
   `Event::coalescing()` classification is now `Critical`. Previously,
   classifying it `Mergeable` allowed a general drop/replace queue to
   discard scroll distance. A more sophisticated future queue may sum
   compatible scroll deltas without losing their sign or units.
2. **Pointer coordinate validation.** `Pointer::sanitize_coordinates`
   rejects nonfinite active `Down`/`Move` positions and converts
   nonfinite terminal `Up`/`Cancel` to a finite `Cancel` at the inert
   origin. This ensures malformed terminal coordinates cannot produce
   an accidental successful tap, and an existing capture still ends.
3. **Two actual input boundaries protected.** Core `Event::to_pointer`
   uses sanitization for mouse and touch, and `runtime::pointer_router::Router::route`
   also sanitizes direct `Pointer` callers. If an invalid initial Down
   arrives after an arena was prepared, the Router cancels that arena
   so it does not remain stuck; invalid Move never abandons an existing
   valid capture.
4. **Windows platform shadow consistency.** Windows-specific settings
   expose `effective_undecorated_shadow(decorations)` and the actual
   Winit window-attribute conversion consumes it, preventing a custom
   undecorated shadow from being requested for an ordinarily decorated
   OS-managed window.

## Regression coverage committed, NOT executed

- 2 cases for finite cursor replacement, wheel preservation and invalid
  cursor samples.
- 2 cases in core Event for invalid mouse/touch active and terminal
  samples, plus the existing scroll-classification test was corrected.
- 1 Pointer sanitization contract case.
- 1 native Windows undecorated-shadow contract case.
- 2 Router cases for prepared invalid Down and captured invalid Move/Up.

**Eight new test functions** and one corrected existing test were added
in source. No passing Rust test results are claimed.

## Exact same-path original-fork Git blob comparison

Both baseline and candidate recursive Git trees were complete
(`truncated:false`).

| Candidate stage | Byte-identical same-path source/config files |
| --- | ---: |
| Early TAIM identity | 60 |
| Stream/window/layout continuation | 55 |
| Input/clock/native platform continuation | 46 |
| Renderer and async timers | 39 |
| Widget event/drag | 37 |
| Native executor contracts | 35 |
| **Input delivery and gesture cleanup** | **33** |

Two inherited exact-matching paths were changed:
`core/src/mouse.rs` and
`core/src/window/settings/windows.rs`.
Other already-modified source files also received substantive changes.

This count is **not a completion percentage**. It does not detect
Iced-derived code in modified/relocated files, API and structural
similarity, copied algorithms or source lineage still preserved
elsewhere. Cosmetic edits cannot prove independent authorship.
Retain original Iced MIT LICENSE, CREDITS.md and Git history.

### Remaining exact same-path files (33)

- `.cargo/audit.toml`
- `.cargo/config.toml`
- `clippy.toml`
- `core/src/keyboard.rs`
- `core/src/window/level.rs`
- `core/src/window/settings/macos.rs`
- `core/src/window/settings/other.rs`
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

## Blockers to a production merge

- No verified real Rust/Cargo/rustfmt/Clippy/compiler run in assistant environment.
- GitHub Actions issue #2: jobs fail before runner allocation.
- Strict source-lineage manual/token audit not passed.
- Verified Wayland/VYOM actual device presentation, keyboard/IME,
  accessibility, GPU reset/recovery, multi-target compilation and
  repeatable p50/p95/p99 benchmarks absent.
- New input cancellation and scroll contracts require platform regression
  before enabling in a stable release.

Run on a working Rust runner:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
python3 scripts/audit/source-lineage-gate.py --strict --output /tmp/taim-lineage.json
```

Strict lineage must reject current inherited source.
Do not merge to `main`, declare 100% original, or tag production/v1.0.
